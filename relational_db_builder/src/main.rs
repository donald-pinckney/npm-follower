use std::panic;

use postgres_db::change_log;
use postgres_db::change_log::Change;

use postgres_db::dependencies::Dependencie;
use postgres_db::internal_state;
use postgres_db::packages::insert_package;
use postgres_db::packages::Package;
use postgres_db::versions::Version;
use postgres_db::DbConnection;
use relational_db_builder::packument::Packument;
use relational_db_builder::packument::Spec;
use utils::check_no_concurrent_processes;

use relational_db_builder::*;

const PAGE_SIZE: i64 = 1024;

fn main() {
    check_no_concurrent_processes("relational_db_builder");

    let conn = postgres_db::connect();

    internal_state::set_relational_processed_seq(0, &conn); //TODO: delete this when done with
                                                            //local dev

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

fn apply_packument_change(conn: &DbConnection, package_name: String, pack: packument::Packument) {
    // let pack_str = format!("{:?}", pack);
    // println!(
    // "parsed change: name = {}, packument = {}...",
    // package_name,
    // // &pack_str[..std::cmp::min(100, pack_str.len())]
    // pack_str
    // );

    let metadata = pack.clone().into();

    let secret = false;
    let package = Package::create(package_name.clone(), metadata, secret);

    // TODO: somehow, wrap this in a transaction, maybe a huge FnMut and we pass it to the db?

    let package_id = insert_package(conn, package);
    postgres_db::dependencies::update_deps_missing_pack(conn, &package_name, package_id);

    match pack {
        Packument::Normal {
            latest,
            created,
            modified: _,
            other_dist_tags: _,
            version_times: _,
            versions,
        } => {
            let insert_deps = |deps: &Vec<(String, Spec)>| -> Vec<i64> {
                let mut ser_deps: Vec<Dependencie> = Vec::new();
                for (pack_name, spec) in deps {
                    // get the id of the package of this dep

                    let dep_pkg_id = postgres_db::packages::query_pkg_id(conn, pack_name);

                    let dep = Dependencie::create(
                        pack_name.clone(),
                        dep_pkg_id,
                        spec.raw.clone(),
                        spec.parsed.clone(),
                        secret,
                    );
                    ser_deps.push(dep);
                }
                postgres_db::dependencies::insert_dependencies(conn, ser_deps)
            };

            println!("Normal: {}", package_name);

            let mut dep_ids_to_patch: Vec<i64> = vec![];
            for (sv, vpack) in versions {
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
                    vpack.dist.tarball_url,
                    vpack.repository.as_ref().map(|r| r.raw.clone()),
                    vpack.repository.map(|r| r.info),
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

                // TODO: what do we do if latest is none?
                if latest.is_some() && latest.as_ref().unwrap() == &sv {
                    postgres_db::packages::patch_latest_version_id(conn, package_id, ver_id);
                }
            }
        }
        // TODO: what do we do with these?
        Packument::Unpublished {
            created: _,
            modified: _,
            unpublished_blob: _,
            extra_version_times: _,
        } => println!("Unpublished: {}", package_name),
        Packument::MissingData | Packument::Deleted => println!("Deleted: {}", package_name),
    }
}
