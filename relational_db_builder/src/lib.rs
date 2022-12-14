mod relational_db_accessor;

use std::collections::HashSet;

use postgres_db::{
    connection::QueryRunner,
    custom_types::{
        PackageStateTimePoint, PackageStateType, Semver, VersionStateTimePoint, VersionStateType,
    },
    dependencies::{Dependency, DependencyType, NewDependency},
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
                // self.create_package(conn, package, data, seq, diff_entry_id)
            }
            DiffLogInstruction::UpdatePackage(data) => {
                // self.update_package(conn, package, data, seq, diff_entry_id)
            }
            DiffLogInstruction::PatchPackageReferences => {
                // self.patch_package_refs(conn, package, seq, diff_entry_id)
            }
            DiffLogInstruction::CreateVersion(v, data) => {
                // self.create_version(conn, package, v, data, seq, diff_entry_id)
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
        package_name: String,
        version: Semver,
        new_pack_data: VersionOnlyPackument,
        seq: i64,
        diff_entry_id: i64,
    ) where
        R: QueryRunner,
    {
        let package_id = self.db.get_package_id_by_name(conn, &package_name);
        let version_id = self.db.get_version_id_by_semver(conn, package_id, version);
        let current_data = self.db.get_version_by_id(conn, version_id);
        let current_prod_deps: Vec<_> = current_data
            .prod_dependencies
            .iter()
            .map(|x| self.db.get_dependency_by_id(conn, *x))
            .collect();
        let current_dev_deps: Vec<_> = current_data
            .dev_dependencies
            .iter()
            .map(|x| self.db.get_dependency_by_id(conn, *x))
            .collect();
        let current_peer_deps: Vec<_> = current_data
            .peer_dependencies
            .iter()
            .map(|x| self.db.get_dependency_by_id(conn, *x))
            .collect();
        let current_optional_deps: Vec<_> = current_data
            .optional_dependencies
            .iter()
            .map(|x| self.db.get_dependency_by_id(conn, *x))
            .collect();

        // Assert that the version is in normal state
        assert_eq!(
            current_data.current_version_state_type,
            VersionStateType::Normal
        );

        // Assert that deps didn't change
        for (old, new) in current_prod_deps
            .iter()
            .zip(new_pack_data.prod_dependencies)
        {
            assert_dep_eq(old, &new);
        }

        for (old, new) in current_dev_deps.iter().zip(new_pack_data.dev_dependencies) {
            assert_dep_eq(old, &new);
        }

        for (old, new) in current_peer_deps
            .iter()
            .zip(new_pack_data.peer_dependencies)
        {
            assert_dep_eq(old, &new);
        }

        for (old, new) in current_optional_deps
            .iter()
            .zip(new_pack_data.optional_dependencies)
        {
            assert_dep_eq(old, &new);
        }

        // Assert that tarball url didn't change
        assert_eq!(
            current_data.tarball_url, new_pack_data.dist.tarball_url,
            "Tarball url changed"
        );

        // Assert that repo info didn't change
        if let Some(new_repo_info) = new_pack_data.repository {
            let current_repo_raw = current_data.repository_raw.expect("Repo changed (1)");
            let current_repo_parsed = current_data.repository_parsed.expect("Repo changed (2)");

            assert_eq!(current_repo_raw, new_repo_info.raw, "Repo changed (3)");

            assert_eq!(current_repo_parsed, new_repo_info.info, "Repo changed (4)");
        } else {
            assert!(current_data.repository_raw.is_none(), "Repo changed (5)");
            assert!(current_data.repository_parsed.is_none(), "Repo changed (6)");
        }

        // Assert that time didn't change
        assert_eq!(current_data.created, new_pack_data.time, "Time changed");

        // Assert that extra metadata didn't change
        let new_extra_metadata = Value::Object(new_pack_data.extra_metadata.into_iter().collect());
        if current_data.extra_metadata != new_extra_metadata {
            self.db
                .set_version_extra_metadata(conn, version_id, new_extra_metadata);
        }
    }

    fn delete_version<R>(
        &mut self,
        conn: &mut R,
        package_name: String,
        version: Semver,
        seq: i64,
        diff_entry_id: i64,
    ) where
        R: QueryRunner,
    {
        let package_id = self.db.get_package_id_by_name(conn, &package_name);
        let version_id = self.db.get_version_id_by_semver(conn, package_id, version);
        let current_data = self.db.get_version_by_id(conn, version_id);

        let delete_prod_deps: Vec<_> = current_data
            .prod_dependencies
            .iter()
            .map(|x| self.db.get_dependency_by_id(conn, *x))
            .map(|x| x.mark_as_delete(DependencyType::Prod).as_new())
            .collect();
        let delete_dev_deps: Vec<_> = current_data
            .dev_dependencies
            .iter()
            .map(|x| self.db.get_dependency_by_id(conn, *x))
            .map(|x| x.mark_as_delete(DependencyType::Dev).as_new())
            .collect();
        let delete_peer_deps: Vec<_> = current_data
            .peer_dependencies
            .iter()
            .map(|x| self.db.get_dependency_by_id(conn, *x))
            .map(|x| x.mark_as_delete(DependencyType::Peer).as_new())
            .collect();
        let delete_optional_deps: Vec<_> = current_data
            .optional_dependencies
            .iter()
            .map(|x| self.db.get_dependency_by_id(conn, *x))
            .map(|x| x.mark_as_delete(DependencyType::Optional).as_new())
            .collect();

        self.insert_or_inc_dependencies(conn, delete_prod_deps);
        self.insert_or_inc_dependencies(conn, delete_dev_deps);
        self.insert_or_inc_dependencies(conn, delete_peer_deps);
        self.insert_or_inc_dependencies(conn, delete_optional_deps);

        self.db
            .delete_version(conn, version_id, seq, diff_entry_id, None);
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

fn assert_dep_eq(old_dep: &Dependency, new_dep: &(String, Spec)) {
    assert_eq!(
        old_dep.dst_package_name, new_dep.0,
        "Dependency package name changed"
    );
    assert_eq!(
        old_dep.raw_spec, new_dep.1.raw,
        "Dependency raw spec changed"
    );
    assert_eq!(
        old_dep.spec, new_dep.1.parsed,
        "Dependency parsed spec changed"
    );
}
