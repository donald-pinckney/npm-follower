mod relational_db_accessor;

use std::collections::HashSet;

use postgres_db::{
    connection::QueryRunner,
    custom_types::{
        PackageStateTimePoint, PackageStateType, Semver, VersionStateTimePoint, VersionStateType,
    },
    dependencies::{DependencyType, NewDependency},
    diff_log::DiffLogInstruction,
    packages::NewPackage,
    packument::{PackageOnlyPackument, Spec, VersionOnlyPackument},
    versions::NewVersion,
};
use relational_db_accessor::RelationalDbAccessor;
use serde_json::Value;

pub struct EntryProcessor {
    pub db: RelationalDbAccessor,
}

impl EntryProcessor {
    pub fn new() -> Self {
        Self {
            db: RelationalDbAccessor::new(),
        }
    }
}

impl EntryProcessor {
    pub fn process_entry<R>(
        &mut self,
        conn: &mut R,
        package: String,
        instr: DiffLogInstruction,
        seq: i64,
        diff_entry_id: i64,
    ) where
        R: QueryRunner,
    {
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

    pub fn flush_caches<R>(&mut self, conn: &mut R)
    where
        R: QueryRunner,
    {
        self.db.flush_caches(conn);
    }

    fn create_package<R>(
        &mut self,
        conn: &mut R,
        package: String,
        data: PackageOnlyPackument,
        seq: i64,
        diff_entry_id: i64,
    ) where
        R: QueryRunner,
    {
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
                other_time_data: Some(
                    serde_json::to_value(
                        postgres_db::serde_non_string_key_serialization::BTreeMapSerializedAsString::new(extra_version_times),
                    )
                    .unwrap(),
                ),
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

        self.db.insert_new_package(conn, new_package);
    }

    fn update_package<R>(
        &mut self,
        conn: &mut R,
        package_name: String,
        data: PackageOnlyPackument,
        seq: i64,
        diff_entry_id: i64,
    ) where
        R: QueryRunner,
    {
        // We have to put this in a block so that we drop
        // `old_package` before calling `update_package`.
        let (diff, package_id) = {
            let old_package = self.db.get_package_by_name(conn, &package_name);
            let old_history = old_package.package_state_history.clone();
            let package_id = old_package.id;

            let new_package = match data {
                PackageOnlyPackument::Normal {
                    latest,
                    created,
                    modified,
                    other_dist_tags,
                    extra_version_times: _,
                } => {
                    let latest_id = latest.map(|latest_semver| {
                        self.db
                            .get_version_id_by_semver(conn, package_id, latest_semver)
                    });
                    NewPackage {
                        name: package_name.clone(),
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
                    name: package_name.clone(),
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
                    name: package_name.clone(),
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

            (old_package.diff(new_package), package_id)
        };
        self.db
            .update_package(conn, package_id, &package_name, diff);
    }

    fn patch_package_refs<R>(
        &mut self,
        conn: &mut R,
        package: String,
        _seq: i64,
        _diff_entry_id: i64,
    ) where
        R: QueryRunner,
    {
        let package_id = self.db.get_package_id_by_name(conn, &package);
        self.db.update_deps_missing_pack(conn, &package, package_id);
    }

    fn create_version<R>(
        &mut self,
        conn: &mut R,
        package: String,
        version: Semver,
        mut data: VersionOnlyPackument,
        seq: i64,
        diff_entry_id: i64,
    ) where
        R: QueryRunner,
    {
        assert_unique(data.prod_dependencies.iter().map(|(dst, _)| dst));
        assert_unique(data.dev_dependencies.iter().map(|(dst, _)| dst));
        assert_unique(data.peer_dependencies.iter().map(|(dst, _)| dst));
        assert_unique(data.optional_dependencies.iter().map(|(dst, _)| dst));

        let package_id = self.db.get_package_id_by_name(conn, &package);

        let (repo_raw, repo_info) = data
            .repository
            .take()
            .map_or((None, None), |x| (Some(x.raw), Some(x.info)));

        let mut prod_to_insert: Vec<_> = data
            .prod_dependencies
            .into_iter()
            .map(|(dst_name, spec)| {
                let dst_id = self.db.maybe_get_package_id_by_name(conn, &dst_name);
                (
                    NewDependency::create(
                        dst_name,
                        dst_id,
                        spec.raw,
                        spec.parsed,
                        DependencyType::Prod,
                    ),
                    DependencyType::Prod,
                )
            })
            .collect();

        let mut dev_to_insert: Vec<_> = data
            .dev_dependencies
            .into_iter()
            .map(|(dst_name, spec)| {
                let dst_id = self.db.maybe_get_package_id_by_name(conn, &dst_name);
                (
                    NewDependency::create(
                        dst_name,
                        dst_id,
                        spec.raw,
                        spec.parsed,
                        DependencyType::Dev,
                    ),
                    DependencyType::Dev,
                )
            })
            .collect();

        let mut peer_to_insert: Vec<_> = data
            .peer_dependencies
            .into_iter()
            .map(|(dst_name, spec)| {
                let dst_id = self.db.maybe_get_package_id_by_name(conn, &dst_name);
                (
                    NewDependency::create(
                        dst_name,
                        dst_id,
                        spec.raw,
                        spec.parsed,
                        DependencyType::Peer,
                    ),
                    DependencyType::Peer,
                )
            })
            .collect();

        let mut optional_to_insert: Vec<_> = data
            .optional_dependencies
            .into_iter()
            .map(|(dst_name, spec)| {
                let dst_id = self.db.maybe_get_package_id_by_name(conn, &dst_name);
                (
                    NewDependency::create(
                        dst_name,
                        dst_id,
                        spec.raw,
                        spec.parsed,
                        DependencyType::Optional,
                    ),
                    DependencyType::Optional,
                )
            })
            .collect();

        let mut to_insert = Vec::new();
        to_insert.append(&mut prod_to_insert);
        to_insert.append(&mut dev_to_insert);
        to_insert.append(&mut peer_to_insert);
        to_insert.append(&mut optional_to_insert);

        let (deps_to_insert, to_insert_types): (Vec<_>, Vec<_>) = to_insert.into_iter().unzip();

        let inserted_ids = self.insert_or_inc_dependencies(conn, deps_to_insert);

        let mut prod_inserted_ids = Vec::new();
        let mut dev_inserted_ids = Vec::new();
        let mut peer_inserted_ids = Vec::new();
        let mut optional_inserted_ids = Vec::new();

        for (dep_id, dep_t) in inserted_ids.into_iter().zip(to_insert_types) {
            let dep_type_vec = match dep_t {
                DependencyType::Prod => &mut prod_inserted_ids,
                DependencyType::Dev => &mut dev_inserted_ids,
                DependencyType::Peer => &mut peer_inserted_ids,
                DependencyType::Optional => &mut optional_inserted_ids,
            };
            dep_type_vec.push(dep_id);
        }

        let new_version_row = NewVersion {
            package_id,
            semver: version,
            current_version_state_type: VersionStateType::Normal,
            version_state_history: vec![VersionStateTimePoint {
                state: VersionStateType::Normal,
                seq,
                diff_entry_id,
                estimated_time: Some(data.time),
            }],
            tarball_url: data.dist.tarball_url,
            repository_raw: repo_raw,
            repository_parsed: repo_info,
            created: data.time,
            extra_metadata: Value::Object(data.extra_metadata.into_iter().collect()),
            prod_dependencies: prod_inserted_ids,
            dev_dependencies: dev_inserted_ids,
            peer_dependencies: peer_inserted_ids,
            optional_dependencies: optional_inserted_ids,
        };

        self.db.insert_new_version(conn, new_version_row);
    }

    fn update_version<R>(
        &mut self,
        conn: &mut R,
        package: String,
        version: Semver,
        data: VersionOnlyPackument,
        seq: i64,
        diff_entry_id: i64,
    ) where
        R: QueryRunner,
    {
        todo!()
    }

    fn delete_version<R>(
        &mut self,
        conn: &mut R,
        package: String,
        version: Semver,
        seq: i64,
        diff_entry_id: i64,
    ) where
        R: QueryRunner,
    {
        todo!()
    }

    fn insert_or_inc_dependencies<R>(&mut self, conn: &mut R, deps: Vec<NewDependency>) -> Vec<i64>
    where
        R: QueryRunner,
    {
        self.db.insert_or_inc_dependencies(conn, deps)
    }
}

fn snoc<T>(mut vec: Vec<T>, item: T) -> Vec<T> {
    vec.push(item);
    vec
}

fn assert_unique<T, X>(xs: T)
where
    T: Iterator<Item = X>,
    X: Eq + std::hash::Hash,
{
    let mut set = HashSet::new();
    for item in xs {
        assert!(set.insert(item));
    }
}
