use std::collections::BTreeMap;
use std::collections::HashMap;

use crate::diff_log::DiffLogInstruction;
use crate::diff_log::NewDiffLogEntryWithHash;
use crate::DbConnection;

use super::sql;
use super::sql::InternalDiffLogStateRow;
use super::sql::InternalDiffLogVersionStateElem;
use super::InternalDiffLogPackageState;
use super::InternalDiffLogVersionState;

#[derive(PartialEq, Eq, Debug)]
enum FlushOp {
    Nothing,
    Create,
    Set,
}

pub struct DiffStateManager {
    local_state: HashMap<String, (Option<InternalDiffLogPackageState>, FlushOp)>,
}

impl DiffStateManager {
    pub fn new() -> DiffStateManager {
        DiffStateManager {
            local_state: HashMap::new(),
        }
    }

    pub fn lookup_package(
        &mut self,
        package_name_str: String,
        conn: &DbConnection,
    ) -> Option<&InternalDiffLogPackageState> {
        let x = self
            .local_state
            .entry(package_name_str)
            .or_insert_with_key(|k| {
                let sql_row = sql::lookup_package(k, conn);
                let loaded_state = sql_row.map(|r| InternalDiffLogPackageState {
                    package_pack_hash: Some(r.package_only_packument_hash),
                    deleted: r.deleted,
                    versions: r
                        .versions
                        .into_iter()
                        .map(|elem| {
                            (
                                elem.v,
                                InternalDiffLogVersionState {
                                    version_pack_hash: elem.pack_hash,
                                    deleted: elem.deleted,
                                },
                            )
                        })
                        .collect(),
                });
                (loaded_state, FlushOp::Nothing)
            });
        x.0.as_ref()
    }

    pub fn apply_diff_entry(&mut self, entry: &NewDiffLogEntryWithHash) {
        let hash = entry.hash.as_ref();
        let package = &entry.entry.package_name;

        // We are *required* to have already called lookup_package
        let state = self.local_state.get_mut(package).unwrap();

        match &entry.entry.instr {
            DiffLogInstruction::CreatePackage(_) => {
                // Check that the package doesn't already exist
                assert!(state.0.is_none());
                assert_eq!(state.1, FlushOp::Nothing);
                assert!(hash.is_none());

                state.0 = Some(InternalDiffLogPackageState {
                    package_pack_hash: None, // This will be changed later with a SetPackageLatestTag instruction
                    deleted: false,
                    versions: BTreeMap::new(),
                });
                state.1 = FlushOp::Create;
            }
            DiffLogInstruction::UpdatePackage(_) => {
                // And the package must already exist for us to be doing an update
                let package_state = state.0.as_mut().unwrap();

                package_state.package_pack_hash = Some(hash.unwrap().clone());
                assert_eq!(package_state.deleted, false);

                state.1 = if state.1 == FlushOp::Create {
                    FlushOp::Create
                } else {
                    FlushOp::Set
                };
            }
            // DiffLogInstruction::SetPackageLatestTag(_) => {
            //     // And the package must already exist for us to be doing a latest tag set
            //     let package_state = state.0.as_mut().unwrap();

            //     package_state.package_pack_hash = Some(hash.unwrap().clone());

            //     state.1 = if state.1 == FlushOp::Create {
            //         FlushOp::Create
            //     } else {
            //         FlushOp::Set
            //     };
            // }
            DiffLogInstruction::DeletePackage => {
                assert!(hash.is_none());

                // And the package must already exist for us to be doing a delete
                let package_state = state.0.as_mut().unwrap();
                assert_eq!(package_state.deleted, false);

                package_state.deleted = true;

                state.1 = if state.1 == FlushOp::Create {
                    FlushOp::Create
                } else {
                    FlushOp::Set
                };
            }
            DiffLogInstruction::CreateVersion(v, _) => {
                // The package must already exist for us to be doing a version creation
                let package_state = state.0.as_mut().unwrap();
                assert!(package_state
                    .versions
                    .insert(
                        v.clone(),
                        InternalDiffLogVersionState {
                            version_pack_hash: hash.unwrap().clone(),
                            deleted: false
                        }
                    )
                    .is_none());

                state.1 = if state.1 == FlushOp::Create {
                    FlushOp::Create
                } else {
                    FlushOp::Set
                };
            }
            DiffLogInstruction::UpdateVersion(v, _) => {
                // The package and version must already exist for us to be doing a version update
                let package_state = state.0.as_mut().unwrap();
                let ver_state = package_state.versions.get_mut(v).unwrap();
                ver_state.version_pack_hash = hash.unwrap().clone()
            }
            DiffLogInstruction::DeleteVersion(v) => {
                assert!(hash.is_none());

                // The package and version must already exist for us to be doing a version delete
                let package_state = state.0.as_mut().unwrap();
                let ver_state = package_state.versions.get_mut(v).unwrap();
                ver_state.deleted = true;
            }
        }
    }

    pub fn flush_to_db(self, conn: &DbConnection) {
        let (create_rows, update_rows): (Vec<_>, Vec<_>) = self
            .local_state
            .into_iter()
            .filter(|(_, (_, flush))| *flush != FlushOp::Nothing)
            .map(|(package_name, (maybe_state, flush))| {
                let state = maybe_state.unwrap();
                let row = InternalDiffLogStateRow {
                    package_name,
                    package_only_packument_hash: state.package_pack_hash.unwrap(),
                    deleted: state.deleted,
                    versions: state
                        .versions
                        .into_iter()
                        .map(|(v, v_state)| InternalDiffLogVersionStateElem {
                            v: v,
                            pack_hash: v_state.version_pack_hash,
                            deleted: v_state.deleted,
                        })
                        .collect(),
                };
                (row, flush)
            })
            .partition(|(_, flush)| *flush == FlushOp::Create);

        sql::create_packages(create_rows.into_iter().map(|(r, _)| r).collect(), conn);
        sql::update_packages(update_rows.into_iter().map(|(r, _)| r).collect(), conn);
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, rc::Rc};

    use chrono::Utc;
    use serde_json::Map;

    // use crate::custom_types::PrereleaseTag;
    // use crate::custom_types::Semver;
    use crate::{
        custom_types::Semver,
        diff_log::{
            internal_diff_log_state::sql::{self, InternalDiffLogStateRow},
            NewDiffLogEntry,
        },
        packument::{Dist, PackageOnlyPackument, VersionOnlyPackument},
        testing,
    };

    use super::*;

    #[test]
    fn test_diff_log_internal_state_manager_empty_db_create_package() {
        let pack1 = "react".to_string();
        let pack2 = "lodash".to_string();

        let pack1_pack = PackageOnlyPackument::Normal {
            latest: None,
            created: Utc::now(),
            modified: Utc::now(),
            other_dist_tags: Map::new(),
        };
        let (_, hash1) = pack1_pack.serialize_and_hash();
        let diff1 = DiffLogInstruction::CreatePackage(pack1_pack.clone());
        let diff1_set_latest = DiffLogInstruction::UpdatePackage(pack1_pack);

        testing::using_test_db(|conn| {
            let mut manager = DiffStateManager::new();

            // Check DB state
            assert_eq!(sql::lookup_package(&pack1, conn), None);
            assert_eq!(sql::lookup_package(&pack2, conn), None);

            // Check memory state
            // These won't hit cache
            assert_eq!(manager.lookup_package(pack1.clone(), conn), None);
            assert_eq!(manager.lookup_package(pack2.clone(), conn), None);

            // These should hit cache, don't know how to test that though
            assert_eq!(manager.lookup_package(pack1.clone(), conn), None);
            assert_eq!(manager.lookup_package(pack2.clone(), conn), None);

            manager.apply_diff_entry(&NewDiffLogEntryWithHash {
                entry: NewDiffLogEntry {
                    seq: 0,
                    package_name: pack1.clone(),
                    instr: diff1,
                },
                hash: None,
            });

            // Check DB state
            assert_eq!(sql::lookup_package(&pack1, conn), None);
            assert_eq!(sql::lookup_package(&pack2, conn), None);

            // Check memory state
            let pack1_state = manager.lookup_package(pack1.clone(), conn).unwrap();
            assert_eq!(pack1_state.package_pack_hash.as_ref(), None);
            assert_eq!(pack1_state.deleted, false);
            assert_eq!(pack1_state.versions.len(), 0);
            assert_eq!(manager.lookup_package(pack2.clone(), conn), None);

            manager.apply_diff_entry(&NewDiffLogEntryWithHash {
                entry: NewDiffLogEntry {
                    seq: 0,
                    package_name: pack1.clone(),
                    instr: diff1_set_latest,
                },
                hash: Some(hash1.clone()),
            });

            // Check DB state
            assert_eq!(sql::lookup_package(&pack1, conn), None);
            assert_eq!(sql::lookup_package(&pack2, conn), None);

            // Check memory state
            let pack1_state = manager.lookup_package(pack1.clone(), conn).unwrap();
            assert_eq!(pack1_state.package_pack_hash.as_ref(), Some(&hash1));
            assert_eq!(pack1_state.deleted, false);
            assert_eq!(pack1_state.versions.len(), 0);
            assert_eq!(manager.lookup_package(pack2.clone(), conn), None);

            manager.flush_to_db(conn);

            // Check DB state
            let pack1_state = sql::lookup_package(&pack1, conn).unwrap();
            assert_eq!(pack1_state.package_only_packument_hash, hash1);
            assert_eq!(pack1_state.deleted, false);
            assert_eq!(pack1_state.versions.len(), 0);
            assert_eq!(sql::lookup_package(&pack2, conn), None);
        });
    }

    #[test]
    fn test_diff_log_internal_state_manager_empty_db_create_package_create_version() {
        let pack1 = "react".to_string();
        let pack2 = "lodash".to_string();

        let v0 = Semver::new_testing_semver(0);
        let created_time = Utc::now();
        let modified_time = Utc::now();
        let pack1_pack = PackageOnlyPackument::Normal {
            latest: None,
            created: created_time,
            modified: modified_time,
            other_dist_tags: Map::new(),
        };
        let pack1_pack_with_latest = PackageOnlyPackument::Normal {
            latest: Some(v0.clone()),
            created: created_time,
            modified: modified_time,
            other_dist_tags: Map::new(),
        };
        let (_, hash1_with_latest) = pack1_pack_with_latest.serialize_and_hash();
        let diff1 = DiffLogInstruction::CreatePackage(pack1_pack);
        let diff1_set_latest = DiffLogInstruction::UpdatePackage(pack1_pack_with_latest);

        let vpack = VersionOnlyPackument {
            prod_dependencies: vec![],
            dev_dependencies: vec![],
            peer_dependencies: vec![],
            optional_dependencies: vec![],
            dist: Dist {
                tarball_url: "stuff".into(),
                shasum: None,
                unpacked_size: None,
                file_count: None,
                integrity: None,
                signature0_sig: None,
                signature0_keyid: None,
                npm_signature: None,
            },
            repository: None,
            time: Utc::now(),
            extra_metadata: BTreeMap::new(),
        };
        let (_, vhash) = vpack.serialize_and_hash();
        let diff1_create_version = DiffLogInstruction::CreateVersion(v0.clone(), vpack);

        testing::using_test_db(|conn| {
            let mut manager = DiffStateManager::new();

            // Check DB state
            assert_eq!(sql::lookup_package(&pack1, conn), None);
            assert_eq!(sql::lookup_package(&pack2, conn), None);

            // Check memory state
            // These won't hit cache
            assert_eq!(manager.lookup_package(pack1.clone(), conn), None);
            assert_eq!(manager.lookup_package(pack2.clone(), conn), None);

            // These should hit cache, don't know how to test that though
            assert_eq!(manager.lookup_package(pack1.clone(), conn), None);
            assert_eq!(manager.lookup_package(pack2.clone(), conn), None);

            // WRITE
            manager.apply_diff_entry(&NewDiffLogEntryWithHash {
                entry: NewDiffLogEntry {
                    seq: 0,
                    package_name: pack1.clone(),
                    instr: diff1,
                },
                hash: None,
            });

            // Check DB state
            assert_eq!(sql::lookup_package(&pack1, conn), None);
            assert_eq!(sql::lookup_package(&pack2, conn), None);

            // Check memory state
            let pack1_state = manager.lookup_package(pack1.clone(), conn).unwrap();
            assert_eq!(pack1_state.package_pack_hash.as_ref(), None);
            assert_eq!(pack1_state.deleted, false);
            assert_eq!(pack1_state.versions.len(), 0);
            assert_eq!(manager.lookup_package(pack2.clone(), conn), None);

            // WRITE
            manager.apply_diff_entry(&NewDiffLogEntryWithHash {
                entry: NewDiffLogEntry {
                    seq: 0,
                    package_name: pack1.clone(),
                    instr: diff1_create_version,
                },
                hash: Some(vhash.clone()),
            });

            // Check DB state
            assert_eq!(sql::lookup_package(&pack1, conn), None);
            assert_eq!(sql::lookup_package(&pack2, conn), None);

            // Check memory state
            let pack1_state = manager.lookup_package(pack1.clone(), conn).unwrap();
            assert_eq!(pack1_state.package_pack_hash.as_ref(), None);
            assert_eq!(pack1_state.deleted, false);
            assert_eq!(pack1_state.versions.len(), 1);
            assert_eq!(
                pack1_state.versions[&v0],
                InternalDiffLogVersionState {
                    version_pack_hash: vhash.clone(),
                    deleted: false
                }
            );
            assert_eq!(manager.lookup_package(pack2.clone(), conn), None);

            // WRITE
            manager.apply_diff_entry(&NewDiffLogEntryWithHash {
                entry: NewDiffLogEntry {
                    seq: 0,
                    package_name: pack1.clone(),
                    instr: diff1_set_latest,
                },
                hash: Some(hash1_with_latest.clone()),
            });

            // Check DB state
            assert_eq!(sql::lookup_package(&pack1, conn), None);
            assert_eq!(sql::lookup_package(&pack2, conn), None);

            // Check memory state
            let pack1_state = manager.lookup_package(pack1.clone(), conn).unwrap();
            assert_eq!(
                pack1_state.package_pack_hash.as_ref().unwrap(),
                &hash1_with_latest
            );
            assert_eq!(pack1_state.deleted, false);
            assert_eq!(pack1_state.versions.len(), 1);
            assert_eq!(
                pack1_state.versions[&v0],
                InternalDiffLogVersionState {
                    version_pack_hash: vhash.clone(),
                    deleted: false
                }
            );
            assert_eq!(manager.lookup_package(pack2.clone(), conn), None);

            manager.flush_to_db(conn);

            // Check DB state
            let pack1_state = sql::lookup_package(&pack1, conn).unwrap();
            assert_eq!(pack1_state.package_only_packument_hash, hash1_with_latest);
            assert_eq!(pack1_state.deleted, false);
            assert_eq!(pack1_state.versions.len(), 1);
            assert_eq!(
                pack1_state.versions[0],
                InternalDiffLogVersionStateElem {
                    v: v0,
                    pack_hash: vhash,
                    deleted: false
                }
            );

            assert_eq!(sql::lookup_package(&pack2, conn), None);
        });
    }

    #[test]
    fn test_diff_log_internal_state_manager_non_empty_db_query_package() {
        let pack1 = "react".to_string();
        let pack2 = "lodash".to_string();

        let pack1_pack = PackageOnlyPackument::Normal {
            latest: None,
            created: Utc::now(),
            modified: Utc::now(),
            other_dist_tags: Map::new(),
        };
        let (_, hash1) = pack1_pack.serialize_and_hash();
        let diff1 = DiffLogInstruction::CreatePackage(pack1_pack.clone());
        let diff1_set_latest = DiffLogInstruction::UpdatePackage(pack1_pack);

        testing::using_test_db(|conn| {
            let mut manager = DiffStateManager::new();

            // Check DB state
            assert_eq!(sql::lookup_package(&pack1, conn), None);
            assert_eq!(sql::lookup_package(&pack2, conn), None);

            // Insert into DB
            sql::create_packages(
                vec![InternalDiffLogStateRow {
                    package_name: pack1.clone(),
                    package_only_packument_hash: hash1.clone(),
                    deleted: false,
                    versions: vec![],
                }],
                conn,
            );

            // Check memory state
            // These won't hit cache
            assert_eq!(
                manager.lookup_package(pack1.clone(), conn),
                Some(&InternalDiffLogPackageState {
                    package_pack_hash: Some(hash1.clone()),
                    deleted: false,
                    versions: BTreeMap::new()
                })
            );
            assert_eq!(manager.lookup_package(pack2.clone(), conn), None);

            // These should hit cache, don't know how to test that though
            assert_eq!(
                manager.lookup_package(pack1.clone(), conn),
                Some(&InternalDiffLogPackageState {
                    package_pack_hash: Some(hash1),
                    deleted: false,
                    versions: BTreeMap::new()
                })
            );
            assert_eq!(manager.lookup_package(pack2.clone(), conn), None);
        });
    }
}
