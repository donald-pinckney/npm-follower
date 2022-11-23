use postgres_db::{
    connection::{DbConnectionInTransaction, QueryRunner},
    custom_types::{PackageStateTimePoint, PackageStateType, Semver},
    diff_log::DiffLogInstruction,
    packages::{NewPackage, Package, PackageUpdate},
    packument::{PackageOnlyPackument, VersionOnlyPackument},
};
use serde_json::Value;

pub struct EntryProcessor {}

impl EntryProcessor {
    pub fn new() -> Self {
        Self {}
    }
}

impl EntryProcessor {
    pub fn process_entry(
        &mut self,
        conn: &mut DbConnectionInTransaction,
        package: String,
        instr: DiffLogInstruction,
        seq: i64,
        diff_entry_id: i64,
    ) {
        match instr {
            DiffLogInstruction::CreatePackage(data) => {
                self.create_package(conn, package, data, seq, diff_entry_id)
            }
            DiffLogInstruction::UpdatePackage(data) => {
                self.update_package(conn, package, data, seq, diff_entry_id)
            }
            DiffLogInstruction::PatchPackageReferences => {
                self.patch_package_refs(conn, package, seq, diff_entry_id)
            }
            DiffLogInstruction::CreateVersion(v, data) => {
                self.create_version(conn, package, v, data, seq, diff_entry_id)
            }
            DiffLogInstruction::UpdateVersion(v, data) => {
                self.update_version(conn, package, v, data, seq, diff_entry_id)
            }
            DiffLogInstruction::DeleteVersion(v) => {
                self.delete_version(conn, package, v, seq, diff_entry_id)
            }
        }
    }

    fn create_package(
        &mut self,
        conn: &mut DbConnectionInTransaction,
        package: String,
        data: PackageOnlyPackument,
        seq: i64,
        diff_entry_id: i64,
    ) {
        let new_package = match data {
            PackageOnlyPackument::Normal {
                latest,
                created,
                modified,
                other_dist_tags,
                extra_version_times: _,
            } => {
                assert_eq!(latest, None);
                NewPackage {
                    name: package,
                    current_package_state_type: PackageStateType::Normal,
                    package_state_history: vec![PackageStateTimePoint {
                        state: PackageStateType::Normal,
                        seq,
                        diff_entry_id,
                        estimated_time: Some(created),
                    }],
                    dist_tag_latest_version: None,
                    created: Some(created),
                    modified: Some(modified),
                    other_dist_tags: Some(Value::Object(other_dist_tags)),
                    other_time_data: None,
                    unpublished_data: None,
                }
            }
            PackageOnlyPackument::Unpublished {
                created,
                modified,
                unpublished_blob,
                extra_version_times,
            } => NewPackage {
                name: package,
                current_package_state_type: PackageStateType::Unpublished,
                package_state_history: vec![PackageStateTimePoint {
                    state: PackageStateType::Unpublished,
                    seq,
                    diff_entry_id,
                    estimated_time: Some(created),
                }],
                dist_tag_latest_version: None,
                created: Some(created),
                modified: Some(modified),
                other_dist_tags: None,
                other_time_data: Some(serde_json::to_value(extra_version_times).unwrap()),
                unpublished_data: Some(unpublished_blob),
            },
            // Maybe we want to treat these separately?
            PackageOnlyPackument::Deleted | PackageOnlyPackument::MissingData => NewPackage {
                name: package,
                current_package_state_type: PackageStateType::Deleted,
                package_state_history: vec![PackageStateTimePoint {
                    state: PackageStateType::Deleted,
                    seq,
                    diff_entry_id,
                    estimated_time: None, // TODO: try to estimate a seq time based on other nearby seqs?
                }],
                dist_tag_latest_version: None,
                created: None,
                modified: None,
                other_dist_tags: None,
                other_time_data: None,
                unpublished_data: None,
            },
        };

        postgres_db::packages::insert_new_package(conn, new_package);
    }

    fn update_package(
        &mut self,
        conn: &mut DbConnectionInTransaction,
        package: String,
        data: PackageOnlyPackument,
        seq: i64,
        diff_entry_id: i64,
    ) {
        let old_package = self.get_package_by_name(conn, &package); // TODO[perf]: replace with cached state
        let old_history = old_package.package_state_history.clone();

        let new_package = match data {
            PackageOnlyPackument::Normal {
                latest,
                created,
                modified,
                other_dist_tags,
                extra_version_times: _,
            } => {
                // TODO[perf]: replace with cached state
                let latest_id = latest.map(|latest_semver| {
                    self.get_version_id_by_semver(conn, old_package.id, latest_semver)
                });
                NewPackage {
                    name: package.clone(),
                    current_package_state_type: PackageStateType::Normal,
                    package_state_history: snoc(
                        old_history,
                        PackageStateTimePoint {
                            state: PackageStateType::Normal,
                            seq,
                            diff_entry_id,
                            estimated_time: Some(modified), // TODO ???
                        },
                    ),
                    dist_tag_latest_version: latest_id,
                    created: Some(created),
                    modified: Some(modified),
                    other_dist_tags: Some(Value::Object(other_dist_tags)),
                    other_time_data: None,
                    unpublished_data: None,
                }
            }
            PackageOnlyPackument::Unpublished {
                created,
                modified,
                unpublished_blob,
                extra_version_times,
            } => NewPackage {
                name: package.clone(),
                current_package_state_type: PackageStateType::Unpublished,
                package_state_history: snoc(
                    old_history,
                    PackageStateTimePoint {
                        state: PackageStateType::Unpublished,
                        seq,
                        diff_entry_id,
                        estimated_time: Some(modified), // TODO ???
                    },
                ),
                dist_tag_latest_version: old_package.dist_tag_latest_version,
                created: Some(created),
                modified: Some(modified),
                other_dist_tags: old_package.other_dist_tags.clone(),
                other_time_data: Some(serde_json::to_value(extra_version_times).unwrap()),
                unpublished_data: Some(unpublished_blob),
            },
            // Maybe we want to treat these separately?
            PackageOnlyPackument::Deleted | PackageOnlyPackument::MissingData => NewPackage {
                name: package.clone(),
                current_package_state_type: PackageStateType::Deleted,
                package_state_history: snoc(
                    old_history,
                    PackageStateTimePoint {
                        state: PackageStateType::Deleted,
                        seq,
                        diff_entry_id,
                        estimated_time: None, // TODO ???
                    },
                ),
                dist_tag_latest_version: old_package.dist_tag_latest_version,
                created: old_package.created,
                modified: old_package.modified,
                other_dist_tags: old_package.other_dist_tags.clone(),
                other_time_data: old_package.other_time_data.clone(),
                unpublished_data: old_package.unpublished_data.clone(),
            },
        };

        let diff = old_package.diff(new_package);

        postgres_db::packages::update_package(conn, &package, diff);
    }

    fn patch_package_refs(
        &mut self,
        conn: &mut DbConnectionInTransaction,
        package: String,
        _seq: i64,
        _diff_entry_id: i64,
    ) {
        let package_id = self.get_package_id_by_name(conn, &package);
        postgres_db::dependencies::update_deps_missing_pack(conn, &package, package_id);
    }

    fn create_version(
        &mut self,
        conn: &mut DbConnectionInTransaction,
        package: String,
        version: Semver,
        data: VersionOnlyPackument,
        seq: i64,
        diff_entry_id: i64,
    ) {
    }

    fn update_version(
        &mut self,
        conn: &mut DbConnectionInTransaction,
        package: String,
        version: Semver,
        data: VersionOnlyPackument,
        seq: i64,
        diff_entry_id: i64,
    ) {
    }

    fn delete_version(
        &mut self,
        conn: &mut DbConnectionInTransaction,
        package: String,
        version: Semver,
        seq: i64,
        diff_entry_id: i64,
    ) {
    }
}

impl EntryProcessor {
    fn get_package_by_name<R: QueryRunner>(&mut self, conn: &mut R, package: &str) -> Package {
        todo!()
    }

    fn get_package_id_by_name<R: QueryRunner>(&mut self, conn: &mut R, package: &str) -> i64 {
        todo!()
    }

    fn get_version_id_by_semver<R: QueryRunner>(
        &mut self,
        conn: &mut R,
        package_id: i64,
        v: Semver,
    ) -> i64 {
        todo!()
    }
}

fn snoc<T>(mut vec: Vec<T>, item: T) -> Vec<T> {
    vec.push(item);
    vec
}
