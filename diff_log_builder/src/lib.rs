pub mod deserialize;

use postgres_db::change_log::Change;
use postgres_db::connection::DbConnectionInTransaction;
use postgres_db::custom_types::Semver;
use postgres_db::diff_log;
use postgres_db::diff_log::internal_diff_log_state::manager::DiffStateManager;
use postgres_db::diff_log::DiffLogInstruction;
use postgres_db::diff_log::NewDiffLogEntry;
use postgres_db::diff_log::NewDiffLogEntryWithHash;
use postgres_db::packument::AllVersionPackuments;
use postgres_db::packument::PackageOnlyPackument;
use postgres_db::packument::VersionOnlyPackument;
use std::collections::BTreeSet;
use std::iter;
use std::panic;

use serde_json::{Map, Value};

use utils::RemoveInto;

pub fn process_changes(
    conn: &mut DbConnectionInTransaction,
    changes: Vec<Change>,
) -> (usize, usize) {
    let mut state_manager = DiffStateManager::new();
    let mut new_diff_entries: Vec<NewDiffLogEntryWithHash> = Vec::new();

    let mut read_bytes = 0;
    let mut write_bytes = 0;

    for c in changes {
        let (rb, wb) = process_change(conn, c, &mut state_manager, &mut new_diff_entries);
        read_bytes += rb;
        write_bytes += wb;
    }

    state_manager.flush_to_db(conn);
    diff_log::insert_diff_log_entries(
        new_diff_entries.into_iter().map(|x| x.entry).collect(),
        conn,
    );

    (read_bytes, write_bytes)
}

fn process_change(
    conn: &mut DbConnectionInTransaction,
    c: Change,
    state_manager: &mut DiffStateManager,
    new_diff_entries: &mut Vec<NewDiffLogEntryWithHash>,
) -> (usize, usize) {
    let seq = c.seq;
    // TODO[perf]: DELETE THIS LATER!
    let change_bytes = serde_json::to_vec(&c.raw_json).unwrap().len();

    // Parse the Change
    let result = panic::catch_unwind(|| deserialize_change(c));
    let (package_name, package_data, all_versions_data) = match result {
        Err(err) => {
            println!("Failed on seq: {}", seq);
            panic::resume_unwind(err);
        }
        Ok(Some((name, package_data, all_versions_data))) => {
            (name, package_data, all_versions_data)
        }
        Ok(None) => return (change_bytes, 0),
    };

    let (_, package_data_hash, package_data_num_bytes) = package_data.serialize_and_hash();

    // 1. Lookup current state
    // 2. Decide what type of change it is (look at hashes, etc.) and compute diffs
    let diff_instrs: Vec<(NewDiffLogEntryWithHash, usize)> =
        match state_manager.lookup_package(package_name.clone(), conn) {
            Some(hash_state) => {
                // The package already exists in the DB.
                // First, we must update all the versions
                // Let OV be the versions already present in DB.
                // Let NV be the new versions in this change.
                // For every v in (OV - NV):
                //  DeleteVersion(v)
                // For every v in (NV - OV):
                //  CreateVersion(v, ...)
                // For every v in (OV ∩ NV) if OV[v] != NV[v]:
                //  UpdateVersion(v, ...)
                //
                // Now we can update the package
                // If new package data != old package data:
                //  UpdatePackage(...)
                // If package is deleted:
                //  DeletePackage(...)

                let old_and_new_versions =
                    BTreeSet::from_iter(hash_state.versions.keys().chain(all_versions_data.keys()));

                let mut instrs_and_write_sizes = vec![];

                for v in old_and_new_versions {
                    match (hash_state.versions.get(v), all_versions_data.get(v)) {
                        (None, None) => unreachable!(),
                        (Some(old_version_state), None) => {
                            if !old_version_state.deleted {
                                instrs_and_write_sizes.push((
                                    NewDiffLogEntryWithHash {
                                        entry: NewDiffLogEntry {
                                            seq,
                                            package_name: package_name.clone(),
                                            instr: DiffLogInstruction::DeleteVersion(v.clone()),
                                        },
                                        hash: None,
                                    },
                                    0,
                                ));
                            }
                        }
                        (None, Some(v_data)) => {
                            let (_, v_hash, v_data_num_bytes) = v_data.serialize_and_hash();
                            instrs_and_write_sizes.push((
                                NewDiffLogEntryWithHash {
                                    entry: NewDiffLogEntry {
                                        seq,
                                        package_name: package_name.clone(),
                                        instr: DiffLogInstruction::CreateVersion(
                                            v.clone(),
                                            v_data.clone(),
                                        ),
                                    },
                                    hash: Some(v_hash),
                                },
                                v_data_num_bytes,
                            ));
                        }
                        (Some(old_version_state), Some(v_data)) => {
                            let (_, v_hash, v_data_num_bytes) = v_data.serialize_and_hash();
                            if old_version_state.version_pack_hash != v_hash {
                                instrs_and_write_sizes.push((
                                    NewDiffLogEntryWithHash {
                                        entry: NewDiffLogEntry {
                                            seq,
                                            package_name: package_name.clone(),
                                            instr: DiffLogInstruction::UpdateVersion(
                                                v.clone(),
                                                v_data.clone(),
                                            ),
                                        },
                                        hash: Some(v_hash),
                                    },
                                    v_data_num_bytes,
                                ));
                            }
                        }
                    }
                }

                if hash_state.package_pack_hash.as_ref().unwrap() != &package_data_hash {
                    instrs_and_write_sizes.push((
                        NewDiffLogEntryWithHash {
                            entry: NewDiffLogEntry {
                                seq,
                                package_name: package_name.clone(),
                                instr: DiffLogInstruction::UpdatePackage(package_data.clone()),
                            },
                            hash: Some(package_data_hash),
                        },
                        package_data_num_bytes,
                    ));
                }

                if !package_data.is_normal() {
                    instrs_and_write_sizes.push((
                        NewDiffLogEntryWithHash {
                            entry: NewDiffLogEntry {
                                seq,
                                package_name,
                                instr: DiffLogInstruction::DeletePackage,
                            },
                            hash: None,
                        },
                        0,
                    ));
                }

                instrs_and_write_sizes
            }
            None => {
                // The package does not yet exist in the DB.
                // So we must do:
                // If package_data is `Normal`:
                //  CreatePackage(package_data but with `latest` set to None)
                //  CreateVersion(v1)
                //  ...
                //  CreateVersion(vn)
                //  UpdatePackage(package_data with `latest` set back to its value)
                //  PatchPackageReferences
                // Else:
                //  CreatePackage(package_data)
                //  CreateVersion(v1)
                //  ...
                //  CreateVersion(vn)
                //  DeletePackage if we are Unpublished, Deleted, or MissingData
                //  PatchPackageReferences

                let version_creation_instrs = all_versions_data.into_iter().map(|(v, v_data)| {
                    generate_create_version_instr(seq, &package_name, v, v_data)
                });

                match &package_data {
                    PackageOnlyPackument::Normal {
                        latest: _,
                        created,
                        modified,
                        other_dist_tags,
                    } => {
                        let package_data_without_latest = PackageOnlyPackument::Normal {
                            latest: None,
                            created: *created,
                            modified: *modified,
                            other_dist_tags: other_dist_tags.clone(),
                        };

                        let instrs_and_write_sizes = iter::once((
                            NewDiffLogEntryWithHash {
                                entry: NewDiffLogEntry {
                                    seq,
                                    package_name: package_name.clone(),
                                    instr: DiffLogInstruction::CreatePackage(
                                        package_data_without_latest,
                                    ),
                                },
                                hash: None,
                            },
                            package_data_num_bytes,
                        ))
                        .chain(version_creation_instrs)
                        .chain(iter::once((
                            NewDiffLogEntryWithHash {
                                entry: NewDiffLogEntry {
                                    seq,
                                    package_name: package_name.clone(),
                                    instr: DiffLogInstruction::UpdatePackage(package_data),
                                },
                                hash: Some(package_data_hash),
                            },
                            package_data_num_bytes,
                        )))
                        .chain(iter::once((
                            NewDiffLogEntryWithHash {
                                entry: NewDiffLogEntry {
                                    seq,
                                    package_name: package_name.clone(),
                                    instr: DiffLogInstruction::PatchPackageReferences,
                                },
                                hash: None,
                            },
                            0,
                        )));

                        Vec::from_iter(instrs_and_write_sizes)
                    }
                    _ => {
                        let instrs_and_write_sizes = iter::once((
                            NewDiffLogEntryWithHash {
                                entry: NewDiffLogEntry {
                                    seq,
                                    package_name: package_name.clone(),
                                    instr: DiffLogInstruction::CreatePackage(package_data),
                                },
                                hash: Some(package_data_hash),
                            },
                            package_data_num_bytes,
                        ))
                        .chain(version_creation_instrs)
                        .chain(iter::once((
                            NewDiffLogEntryWithHash {
                                entry: NewDiffLogEntry {
                                    seq,
                                    package_name: package_name.clone(),
                                    instr: DiffLogInstruction::DeletePackage,
                                },
                                hash: None,
                            },
                            0,
                        )))
                        .chain(iter::once((
                            NewDiffLogEntryWithHash {
                                entry: NewDiffLogEntry {
                                    seq,
                                    package_name: package_name.clone(),
                                    instr: DiffLogInstruction::PatchPackageReferences,
                                },
                                hash: None,
                            },
                            0,
                        )));

                        Vec::from_iter(instrs_and_write_sizes)
                    }
                }
            }
        };

    let mut write_bytes = 0;
    // 3. Update state
    for d in &diff_instrs {
        state_manager.apply_diff_entry(&d.0);
        write_bytes += d.1;
    }
    // 4. Add diff entries
    new_diff_entries.extend(diff_instrs.into_iter().map(|x| x.0));

    (change_bytes, write_bytes)
}

fn generate_create_version_instr(
    seq: i64,
    package_name: &str,
    v: Semver,
    v_data: VersionOnlyPackument,
) -> (NewDiffLogEntryWithHash, usize) {
    let (_, v_hash, v_data_num_bytes) = v_data.serialize_and_hash();
    (
        NewDiffLogEntryWithHash {
            entry: NewDiffLogEntry {
                seq,
                package_name: package_name.to_owned(),
                instr: DiffLogInstruction::CreateVersion(v, v_data),
            },
            hash: Some(v_hash),
        },
        v_data_num_bytes,
    )
}

pub fn deserialize_change(
    c: Change,
) -> Option<(String, PackageOnlyPackument, AllVersionPackuments)> {
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
        Some((
            package_name,
            PackageOnlyPackument::Deleted,
            AllVersionPackuments::new(),
        ))
    } else {
        let unpublished = doc
            .get("time")
            .map(|time_value| time_value.as_object().unwrap().contains_key("unpublished"))
            .unwrap_or(false);

        if unpublished {
            let (package_data, versions_data) =
                deserialize::deserialize_packument_blob_unpublished(doc);
            Some((package_name, package_data, versions_data))
        } else {
            let has_dist_tags = doc.contains_key("dist-tags");
            if has_dist_tags {
                let (package_data, versions_data) =
                    deserialize::deserialize_packument_blob_normal(doc);
                Some((package_name, package_data, versions_data))
            } else {
                // If the packument says *not* deleted,
                // but has no fields, then we mark it as missing data.
                // See seq = 4413127.
                assert!(!doc.contains_key("time"));
                assert!(!doc.contains_key("versions"));
                Some((
                    package_name,
                    PackageOnlyPackument::MissingData,
                    AllVersionPackuments::new(),
                ))
            }
        }
    }
}
