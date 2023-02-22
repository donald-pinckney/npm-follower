pub mod internal_diff_log_state;

use crate::connection::DbConnection;
use crate::connection::QueryRunner;
use crate::custom_types::DiffTypeEnum;
use crate::custom_types::Semver;
use crate::packument::PackageOnlyPackument;
use crate::packument::VersionOnlyPackument;

use super::schema;
use super::schema::diff_log;
use diesel::prelude::*;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

// TODO[perf]: add some Rcs so things are passed by reference more.

#[derive(Queryable)]
struct DiffLogRow {
    id: i64,
    seq: i64,
    package_name: String,
    dt: DiffTypeEnum,
    package_only_packument: Option<Value>,
    v: Option<Semver>,
    version_packument: Option<Value>,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = diff_log)]
struct NewDiffLogRow {
    seq: i64,
    package_name: String,
    dt: DiffTypeEnum,
    package_only_packument: Option<Value>,
    v: Option<Semver>,
    version_packument: Option<Value>,
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct DiffLogEntry {
    pub id: i64,
    pub seq: i64,
    pub package_name: String,
    pub instr: DiffLogInstruction,
}

#[derive(Debug)]
pub struct NewDiffLogEntry {
    pub seq: i64,
    pub package_name: String,
    pub instr: DiffLogInstruction,
}

#[derive(Debug)]
pub struct NewDiffLogEntryWithHash {
    pub entry: NewDiffLogEntry,
    // Contains the hash the set for either the package only packument or the version only packument.
    // Is Some for: UpdatePackage, SetPackageLatestTag, CreateVersion, UpdateVersion
    pub hash: Option<String>,
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum DiffLogInstruction {
    CreatePackage(PackageOnlyPackument),
    UpdatePackage(PackageOnlyPackument),
    PatchPackageReferences,
    CreateVersion(Semver, VersionOnlyPackument),
    UpdateVersion(Semver, VersionOnlyPackument),
    DeleteVersion(Semver),
}

impl From<DiffLogRow> for DiffLogEntry {
    fn from(r: DiffLogRow) -> Self {
        let instr = match r.dt {
            DiffTypeEnum::CreatePackage => DiffLogInstruction::CreatePackage(
                serde_json::from_value(r.package_only_packument.unwrap()).unwrap(),
            ),
            DiffTypeEnum::UpdatePackage => DiffLogInstruction::UpdatePackage(
                serde_json::from_value(r.package_only_packument.unwrap()).unwrap(),
            ),
            // DiffTypeEnum::SetPackageLatestTag => DiffLogInstruction::SetPackageLatestTag(r.v),
            DiffTypeEnum::CreateVersion => DiffLogInstruction::CreateVersion(
                r.v.unwrap(),
                serde_json::from_value(r.version_packument.unwrap()).unwrap(),
            ),
            DiffTypeEnum::UpdateVersion => DiffLogInstruction::UpdateVersion(
                r.v.unwrap(),
                serde_json::from_value(r.version_packument.unwrap()).unwrap(),
            ),
            DiffTypeEnum::DeleteVersion => DiffLogInstruction::DeleteVersion(r.v.unwrap()),
            DiffTypeEnum::PatchPackageReferences => DiffLogInstruction::PatchPackageReferences,
        };

        DiffLogEntry {
            id: r.id,
            seq: r.seq,
            package_name: r.package_name,
            instr,
        }
    }
}

impl From<NewDiffLogEntry> for NewDiffLogRow {
    fn from(e: NewDiffLogEntry) -> Self {
        let (dt, pkg_pack, v, version_packument) = match e.instr {
            DiffLogInstruction::CreatePackage(pack) => {
                (DiffTypeEnum::CreatePackage, Some(pack), None, None)
            }
            DiffLogInstruction::UpdatePackage(pack) => {
                (DiffTypeEnum::UpdatePackage, Some(pack), None, None)
            }
            // DiffLogInstruction::SetPackageLatestTag(v) => {
            //     (DiffTypeEnum::SetPackageLatestTag, None, v, None)
            // }
            DiffLogInstruction::CreateVersion(v, vp) => {
                (DiffTypeEnum::CreateVersion, None, Some(v), Some(vp))
            }
            DiffLogInstruction::UpdateVersion(v, vp) => {
                (DiffTypeEnum::UpdateVersion, None, Some(v), Some(vp))
            }
            DiffLogInstruction::DeleteVersion(v) => {
                (DiffTypeEnum::DeleteVersion, None, Some(v), None)
            }
            DiffLogInstruction::PatchPackageReferences => {
                (DiffTypeEnum::PatchPackageReferences, None, None, None)
            }
        };
        NewDiffLogRow {
            seq: e.seq,
            package_name: e.package_name,
            dt,
            package_only_packument: pkg_pack.map(|x| serde_json::to_value(x).unwrap()),
            v,
            version_packument: version_packument.map(|x| serde_json::to_value(x).unwrap()),
        }
    }
}

pub fn query_num_diff_entries_after_seq(after_seq: i64, conn: &mut DbConnection) -> i64 {
    use diesel::dsl::*;
    use diesel::prelude::*;
    use schema::diff_log::dsl::*;

    conn.first(diff_log.filter(seq.gt(after_seq)).select(count(id)))
        .unwrap_or_else(|_| {
            panic!(
                "Error querying DB for number of changes after seq: {}",
                after_seq
            )
        })
}

pub fn query_diff_entries_after_seq(
    after_seq: i64,
    limit_size: i64,
    conn: &mut DbConnection,
) -> Vec<DiffLogEntry> {
    use diesel::prelude::*;
    use schema::diff_log::dsl::*;

    let diff_log_alias = alias!(schema::diff_log as diff_log_alias);

    let seq_subquery = diff_log_alias
        .filter(diff_log_alias.field(seq).gt(after_seq))
        .select(diff_log_alias.field(seq))
        .distinct()
        .order(diff_log_alias.field(seq))
        .limit(limit_size);

    let join_query = diff_log
        .filter(seq.eq_any(seq_subquery))
        .select((
            id,
            seq,
            package_name,
            dt,
            package_only_packument,
            v,
            version_packument,
        ))
        .order(id);

    let rows: Vec<DiffLogRow> = conn.load(join_query).unwrap_or_else(|err| {
        panic!(
            "Error querying DB for diff_log after seq {}:\n{}",
            after_seq, err
        )
    });

    rows.into_iter().map(|e| e.into()).collect()
}

pub fn query_num_changes_after_seq_in_diff_log(after_seq: i64, conn: &mut DbConnection) -> i64 {
    use schema::diff_log::dsl::*;

    let query = diff_log
        .filter(seq.gt(after_seq))
        .select(diesel::dsl::count_distinct(seq));

    conn.first(query).unwrap_or_else(|_| {
        panic!(
            "Error querying DB for number of changes after seq: {}",
            after_seq
        )
    })
}

const INSERT_CHUNK_SIZE: usize = 2048;

pub fn insert_diff_log_entries<R: QueryRunner>(
    entries: Vec<NewDiffLogEntry>,
    conn: &mut R,
) -> usize {
    let rows: Vec<NewDiffLogRow> = entries.into_iter().map(|e| e.into()).collect();

    let mut chunk_iter = rows.chunks_exact(INSERT_CHUNK_SIZE);
    let mut modify_count = 0;
    for chunk in &mut chunk_iter {
        modify_count += insert_diff_log_rows_chunk(chunk, conn);
    }

    modify_count += insert_diff_log_rows_chunk(chunk_iter.remainder(), conn);

    modify_count
}

fn insert_diff_log_rows_chunk<R: QueryRunner>(rows: &[NewDiffLogRow], conn: &mut R) -> usize {
    use schema::diff_log::dsl::*;

    if rows.len() > INSERT_CHUNK_SIZE {
        panic!("Programming error: insert_diff_log_rows_chunk must be called with a chunk of size <= INSERT_CHUNK_SIZE ({})", INSERT_CHUNK_SIZE);
    }

    conn.execute(diesel::insert_into(diff_log).values(rows))
        .unwrap_or_else(|err| panic!("Failed to insert diff log rows into DB.\n{}", err))
}

pub mod testing {
    use super::*;

    pub fn get_all_diff_logs<R: QueryRunner>(conn: &mut R) -> Vec<DiffLogEntry> {
        use super::schema::diff_log::dsl::*;

        // TODO[bug]: batch this
        let rows: Vec<DiffLogRow> = conn.load(diff_log).unwrap();

        rows.into_iter().map(|r| r.into()).collect()
    }
}

#[cfg(test)]
mod tests {

    use std::collections::BTreeMap;

    use super::testing::get_all_diff_logs;
    use crate::change_log;
    use crate::custom_types::PrereleaseTag;
    use crate::custom_types::Semver;
    use crate::diff_log::insert_diff_log_entries;
    use crate::packument::Dist;
    use crate::packument::PackageOnlyPackument;
    use crate::packument::VersionOnlyPackument;
    use crate::testing;

    use chrono::Utc;
    use serde_json::Map;
    use serde_json::Value;

    use super::DiffLogEntry;
    use super::DiffLogInstruction;
    use super::NewDiffLogEntry;

    #[test]
    fn test_diff_log_read_write() {
        let v = Semver {
            major: 1,
            minor: 2,
            bug: 3,
            prerelease: vec![PrereleaseTag::Int(5), PrereleaseTag::String("alpha".into())],
            build: vec!["b23423".into()],
        };
        let v2 = Semver {
            major: 8,
            minor: 2,
            bug: 3,
            prerelease: vec![PrereleaseTag::Int(5), PrereleaseTag::String("alpha".into())],
            build: vec!["b23423".into()],
        };
        let garbage_pack_data = PackageOnlyPackument::Normal {
            latest: Some(v2),
            created: Utc::now(),
            modified: Utc::now(),
            other_dist_tags: Map::new(),
            extra_version_times: BTreeMap::new(),
        };

        let garbage_version_pack_data = VersionOnlyPackument {
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

        let new_data = vec![
            NewDiffLogEntry {
                seq: 100,
                package_name: "react".into(),
                instr: DiffLogInstruction::CreatePackage(garbage_pack_data.clone()),
            },
            NewDiffLogEntry {
                seq: 101,
                package_name: "react".into(),
                instr: DiffLogInstruction::UpdatePackage(garbage_pack_data.clone()),
            },
            NewDiffLogEntry {
                seq: 103,
                package_name: "react".into(),
                instr: DiffLogInstruction::CreateVersion(
                    v.clone(),
                    garbage_version_pack_data.clone(),
                ),
            },
            NewDiffLogEntry {
                seq: 104,
                package_name: "react".into(),
                instr: DiffLogInstruction::UpdateVersion(
                    v.clone(),
                    garbage_version_pack_data.clone(),
                ),
            },
            NewDiffLogEntry {
                seq: 105,
                package_name: "react".into(),
                instr: DiffLogInstruction::DeleteVersion(v.clone()),
            },
        ];

        let expected_data = vec![
            DiffLogEntry {
                id: 1,
                seq: 100,
                package_name: "react".into(),
                instr: DiffLogInstruction::CreatePackage(garbage_pack_data.clone()),
            },
            DiffLogEntry {
                id: 2,
                seq: 101,
                package_name: "react".into(),
                instr: DiffLogInstruction::UpdatePackage(garbage_pack_data),
            },
            DiffLogEntry {
                id: 3,
                seq: 103,
                package_name: "react".into(),
                instr: DiffLogInstruction::CreateVersion(
                    v.clone(),
                    garbage_version_pack_data.clone(),
                ),
            },
            DiffLogEntry {
                id: 4,
                seq: 104,
                package_name: "react".into(),
                instr: DiffLogInstruction::UpdateVersion(v.clone(), garbage_version_pack_data),
            },
            DiffLogEntry {
                id: 5,
                seq: 105,
                package_name: "react".into(),
                instr: DiffLogInstruction::DeleteVersion(v),
            },
        ];

        let now = Utc::now();
        testing::using_test_db(|conn| {
            change_log::insert_change(conn, 100, Value::Null, now);
            change_log::insert_change(conn, 101, Value::Null, now);
            change_log::insert_change(conn, 102, Value::Null, now);
            change_log::insert_change(conn, 103, Value::Null, now);
            change_log::insert_change(conn, 104, Value::Null, now);
            change_log::insert_change(conn, 105, Value::Null, now);

            let insert_count = insert_diff_log_entries(new_data, conn);
            let retrieved_data = get_all_diff_logs(conn);

            assert_eq!(retrieved_data, expected_data);
            assert_eq!(retrieved_data.len(), insert_count);
        });
    }
}
