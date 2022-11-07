pub mod packument;

use crate::packument::Packument;
use crate::packument::Spec;
use crate::packument::VersionPackument;
use postgres_db::change_log::Change;
use postgres_db::custom_types::Semver;
use postgres_db::dependencies::Dependencie;
use postgres_db::packages::insert_package;
use postgres_db::packages::Package;
use postgres_db::versions::Version;
use postgres_db::DbConnection;
use std::collections::HashMap;
use std::collections::HashSet;
use std::panic;

use serde_json::{Map, Value};

use utils::RemoveInto;

pub fn deserialize_change(c: Change) -> Option<(String, Packument)> {
    let mut change_json = serde_json::from_value::<Map<String, Value>>(c.raw_json).unwrap();
    let del = change_json
        .remove_key_unwrap_type::<bool>("deleted")
        .unwrap();

    let package_name = change_json.remove_key_unwrap_type::<String>("id").unwrap();

    if package_name == "_design/app" || package_name == "_design/scratch" {
        return None;
    }

    let mut doc = change_json
        .remove_key_unwrap_type::<Map<String, Value>>("doc")
        .unwrap();
    let doc_id = doc.remove_key_unwrap_type::<String>("_id").unwrap();
    let doc_deleted = doc
        .remove_key_unwrap_type::<bool>("_deleted")
        .unwrap_or(false);
    doc.remove_key_unwrap_type::<String>("_rev").unwrap();

    if del != doc_deleted {
        panic!("ERROR: mismatched del and del_deleted");
    }

    if package_name != doc_id {
        panic!("ERROR: mismatched package_name and doc_id");
    }

    if del {
        if !doc.is_empty() {
            panic!("ERROR: extra keys in deleted doc");
        }
        Some((package_name, Packument::Deleted))
    } else {
        let unpublished = doc
            .get("time")
            .map(|time_value| time_value.as_object().unwrap().contains_key("unpublished"))
            .unwrap_or(false);

        if unpublished {
            Some((
                package_name,
                packument::deserialize::deserialize_packument_blob_unpublished(doc),
            ))
        } else {
            let has_dist_tags = doc.contains_key("dist-tags");
            if has_dist_tags {
                Some((
                    package_name,
                    packument::deserialize::deserialize_packument_blob_normal(doc),
                ))
            } else {
                // If the packument says *not* deleted,
                // but has no fields, then we mark it as missing data.
                // See seq = 4413127.
                assert!(!doc.contains_key("time"));
                assert!(!doc.contains_key("versions"));
                Some((package_name, Packument::MissingData))
            }
        }
    }
}

pub fn process_change(conn: &mut DbConnection, c: Change) {
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

fn make_dep_countmap(
    versions: &HashMap<Semver, VersionPackument>,
) -> HashMap<(String, String), i64> {
    let mut dep_countmap = HashMap::new();
    for ver_pack in versions.values() {
        dep_countmap = update_dep_countmap(ver_pack, dep_countmap);
    }
    dep_countmap
}

fn apply_versions(
    conn: &mut DbConnection,
    pack: packument::Packument,
    pkg_already_existed: bool,
    package_id: i64,
    secret: bool,
) {
    match pack {
        Packument::Normal {
            latest,
            created,
            modified: _,
            other_dist_tags: _,
            version_times: _,
            versions,
        } => {
            println!("Normal pkg: {}", package_id);
            // these are made such that the postgres_db does the least amount of work possible
            let dep_countmap = make_dep_countmap(&versions);
            let mut deps_inserted: HashSet<(String, String)> = HashSet::new();
            // TODO [bug]: this isn't used?
            // we probably need to patch self-referential deps
            let mut dep_ids_to_patch: Vec<i64> = vec![];

            let mut insert_deps = |deps: &Vec<(String, Spec)>| -> Vec<i64> {
                let mut constructed_deps: Vec<Dependencie> = Vec::new();
                for (pack_name, spec) in deps {
                    let spec_pair = (pack_name.clone(), serde_json::to_string(&spec.raw).unwrap());
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
                            .get(&(pack_name.clone(), serde_json::to_string(&spec.raw).unwrap()))
                            .unwrap_or(&1),
                    );

                    deps_inserted.insert(spec_pair);
                    constructed_deps.push(dep);
                }
                let inserted_deps =
                    postgres_db::dependencies::insert_dependencies(conn, constructed_deps);
                dep_ids_to_patch.extend(&inserted_deps);

                inserted_deps
            };

            let mut versions_to_insert = vec![];

            for (sv, vpack) in &versions {
                let prod_dep_ids = insert_deps(&vpack.prod_dependencies);
                let dev_dep_ids = insert_deps(&vpack.dev_dependencies);
                let optional_dep_ids = insert_deps(&vpack.optional_dependencies);
                let peer_dep_ids = insert_deps(&vpack.peer_dependencies);

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

                versions_to_insert.push(ver);
            }

            let ver_ids_semvers = postgres_db::versions::insert_versions(conn, versions_to_insert);

            for (ver_id, sv) in ver_ids_semvers {
                let needs_patch = matches!(latest, Some(ref x) if x == &sv);
                if needs_patch {
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
        } => {
            println!("Unpublished pkg: {}", package_id)
        }
        Packument::MissingData | Packument::Deleted => {
            println!("Deleted pkg: {}", package_id)
        }
    }
}

pub fn apply_packument_change(
    conn: &mut DbConnection,
    package_name: String,
    pack: packument::Packument,
) {
    let metadata = pack.clone().into();

    let secret = false;
    let package = Package::create(package_name.clone(), metadata, secret);

    let (package_id, pkg_already_existed) = insert_package(conn, package);

    // we don't need to patch deps if we had it before
    if !pkg_already_existed {
        postgres_db::dependencies::update_deps_missing_pack(conn, &package_name, package_id);
    }

    let res = conn.run_psql_transaction(|| {
        apply_versions(conn, pack, pkg_already_existed, package_id, secret);
        Ok(())
    });
    match res {
        Ok(_) => (),
        Err(err) => {
            println!("Failed on package: {}, reason: {}", package_name, err);
        }
    }
}
