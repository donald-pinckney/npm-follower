use std::collections::HashMap;
use std::collections::HashSet;
use std::panic;

use postgres_db::change_log;
use postgres_db::change_log::Change;

use postgres_db::custom_types::ParsedSpec;
use postgres_db::dependencies::Dependencie;
use postgres_db::internal_state;
use postgres_db::packages::insert_package;
use postgres_db::packages::Package;
use postgres_db::versions::Version;
use postgres_db::DbConnection;
use relational_db_builder::packument::Packument;
use relational_db_builder::packument::Spec;
use relational_db_builder::packument::VersionPackument;
use utils::check_no_concurrent_processes;

use relational_db_builder::*;

const PAGE_SIZE: i64 = 1024;

fn main() {
    check_no_concurrent_processes("relational_db_builder");

    let conn = postgres_db::connect();

    let mut processed_up_to = internal_state::query_relational_processed_seq(&conn).unwrap_or(0);

    let num_changes_total = change_log::query_num_changes_after_seq(processed_up_to, &conn);
    let mut num_changes_so_far = 0;

    // TODO: Extract this into function (duplicated in download_queuer/src/main.rs)
    loop {
        println!(
            "Fetching seq > {}, page size = {} ({:.1}%)",
            processed_up_to,
            PAGE_SIZE,
            100.0 * (num_changes_so_far as f64) / (num_changes_total as f64)
        );
        let changes = change_log::query_changes_after_seq(processed_up_to, PAGE_SIZE, &conn);
        let num_changes = changes.len() as i64;
        num_changes_so_far += num_changes;
        if num_changes == 0 {
            break;
        }

        let last_seq_in_page = changes.last().unwrap().seq;

        for c in changes {
            process_change(&conn, c);
        }

        internal_state::set_relational_processed_seq(last_seq_in_page, &conn);
        processed_up_to = last_seq_in_page;

        if num_changes < PAGE_SIZE {
            break;
        }
    }
}

fn process_change(conn: &DbConnection, c: Change) {
    let seq = c.seq;
    // println!("\nparsing seq: {}", seq);

    let result = panic::catch_unwind(|| deserialize_change(c));
    match result {
        Err(err) => {
            println!("Failed on seq: {}", seq);
            panic::resume_unwind(err);
        }
        Ok(Some((name, pack))) => apply_packument_change(conn, name, pack),
        Ok(None) => (),
    }
}

fn update_dep_countmap(
    ver: &VersionPackument,
    mut map: HashMap<(String, String), i64>,
) -> HashMap<(String, String), i64> {
    for (pack_name, spec) in ver
        .prod_dependencies
        .iter()
        .chain(ver.dev_dependencies.iter())
        .chain(ver.optional_dependencies.iter())
        .chain(ver.peer_dependencies.iter())
    {
        let count = map
            .entry((pack_name.clone(), serde_json::to_string(&spec.raw).unwrap()))
            .or_insert(0);
        *count += 1;
    }
    map
}

fn apply_packument_change(conn: &DbConnection, package_name: String, pack: packument::Packument) {
    let metadata = pack.clone().into();

    let secret = false;
    let package = Package::create(package_name.clone(), metadata, secret);

    let (package_id, pkg_already_existed) = insert_package(conn, package);

    // we don't need to patch deps if we had it before
    if !pkg_already_existed {
        postgres_db::dependencies::update_deps_missing_pack(conn, &package_name, package_id);
    }

    let res = conn.run_psql_transaction(|| {
        match pack {
            Packument::Normal {
                latest,
                created,
                modified: _,
                other_dist_tags: _,
                version_times: _,
                versions,
            } => {
                // these are made such that the postgres_db does the least amount of work possible
                let mut dep_countmap = HashMap::new();
                let mut deps_inserted: HashSet<(String, String)> = HashSet::new();

                for ver_pack in versions.values() {
                    dep_countmap = update_dep_countmap(ver_pack, dep_countmap);
                }

                let mut insert_deps = |deps: &Vec<(String, Spec)>| -> Vec<i64> {
                    let mut constructed_deps: Vec<Dependencie> = Vec::new();
                    for (pack_name, spec) in deps {
                        let spec_pair =
                            (pack_name.clone(), serde_json::to_string(&spec.raw).unwrap());
                        // skip dups
                        if deps_inserted.contains(&spec_pair) {
                            continue;
                        }

                        // get the id of the package of this dep, these could be none.

                        let dep_pkg_id = postgres_db::packages::query_pkg_id(conn, pack_name);

                        let dep = Dependencie::create(
                            pack_name.clone(),
                            dep_pkg_id,
                            spec.raw.clone(),
                            spec.parsed.clone(),
                            secret,
                            *dep_countmap
                                .get(&(
                                    pack_name.clone(),
                                    serde_json::to_string(&spec.raw).unwrap(),
                                ))
                                .unwrap_or(&1),
                        );

                        deps_inserted.insert(spec_pair);
                        constructed_deps.push(dep);
                    }
                    postgres_db::dependencies::insert_dependencies(conn, constructed_deps)
                };

                println!("Normal: {}", package_name);

                let mut dep_ids_to_patch: Vec<i64> = vec![];
                for (sv, vpack) in &versions {
                    let prod_dep_ids = insert_deps(&vpack.prod_dependencies);
                    let dev_dep_ids = insert_deps(&vpack.dev_dependencies);
                    let optional_dep_ids = insert_deps(&vpack.optional_dependencies);
                    let peer_dep_ids = insert_deps(&vpack.peer_dependencies);
                    dep_ids_to_patch.extend(&prod_dep_ids);
                    dep_ids_to_patch.extend(&dev_dep_ids);
                    dep_ids_to_patch.extend(&optional_dep_ids);
                    dep_ids_to_patch.extend(&peer_dep_ids);

                    let ver = Version::create(
                        package_id,
                        sv.clone(),
                        vpack.dist.tarball_url.clone(),
                        vpack.repository.as_ref().map(|r| r.raw.clone()),
                        vpack.repository.as_ref().map(|r| r.info.clone()),
                        created,
                        false,
                        serde_json::to_value(&vpack.extra_metadata).unwrap(),
                        prod_dep_ids,
                        dev_dep_ids,
                        peer_dep_ids,
                        optional_dep_ids,
                        secret,
                    );

                    let ver_id = postgres_db::versions::insert_version(conn, ver);

                    if latest.is_some() && latest.as_ref().unwrap() == sv {
                        postgres_db::packages::patch_latest_version_id(conn, package_id, ver_id);
                    }
                }

                // check versions that might have been deleted if this is a package change update
                if pkg_already_existed {
                    postgres_db::versions::delete_versions_not_in(
                        conn,
                        package_id,
                        versions.keys().collect(),
                    );
                }
            }
            // TODO: do we do something with these other than inserting the package?
            Packument::Unpublished {
                created: _,
                modified: _,
                unpublished_blob: _,
                extra_version_times: _,
            } => println!("Unpublished: {}", package_name),
            Packument::MissingData | Packument::Deleted => println!("Deleted: {}", package_name),
        }
        Ok(())
    });
    match res {
        Ok(_) => (),
        Err(err) => {
            println!("Failed on package: {}, reason: {}", package_name, err);
        }
    }
}
