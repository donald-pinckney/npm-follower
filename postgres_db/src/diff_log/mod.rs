pub mod internal_diff_log_state;

use std::rc::Rc;

use crate::custom_types::DiffTypeEnum;
use crate::custom_types::Semver;
use crate::packument::PackageOnlyPackument;
use crate::packument::VersionOnlyPackument;

use super::schema;
use super::schema::diff_log;
use super::DbConnection;
use diesel::prelude::*;
use diesel::Insertable;
use diesel::Queryable;
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

#[derive(Insertable)]
#[table_name = "diff_log"]
struct NewDiffLogRow {
    seq: i64,
    package_name: String,
    dt: DiffTypeEnum,
    package_only_packument: Option<Value>,
    v: Option<Semver>,
    version_packument: Option<Value>,
}

#[derive(PartialEq, Eq, Debug)]
pub struct DiffLogEntry {
    id: i64,
    seq: i64,
    package_name: String,
    instr: DiffLogInstruction,
}

pub struct NewDiffLogEntry {
    pub seq: i64,
    pub package_name: String,
    pub instr: DiffLogInstruction,
}

pub struct NewDiffLogEntryWithHash {
    pub entry: NewDiffLogEntry,
    // Contains the hash the set for either the package only packument or the version only packument.
    // Is Some for: UpdatePackage, SetPackageLatestTag, CreateVersion, UpdateVersion
    pub hash: Option<String>,
}

#[derive(PartialEq, Eq, Debug)]
pub enum DiffLogInstruction {
    CreatePackage(PackageOnlyPackument),
    UpdatePackage(PackageOnlyPackument),
    // SetPackageLatestTag(Option<Semver>),
    DeletePackage,
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
            DiffTypeEnum::DeletePackage => DiffLogInstruction::DeletePackage,
            DiffTypeEnum::CreateVersion => DiffLogInstruction::CreateVersion(
                r.v.unwrap(),
                serde_json::from_value(r.version_packument.unwrap()).unwrap(),
            ),
            DiffTypeEnum::UpdateVersion => DiffLogInstruction::UpdateVersion(
                r.v.unwrap(),
                serde_json::from_value(r.version_packument.unwrap()).unwrap(),
            ),
            DiffTypeEnum::DeleteVersion => DiffLogInstruction::DeleteVersion(r.v.unwrap()),
        };

        DiffLogEntry {
            id: r.id,
            seq: r.seq,
            package_name: r.package_name,
            instr: instr,
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
            DiffLogInstruction::DeletePackage => (DiffTypeEnum::DeletePackage, None, None, None),
            DiffLogInstruction::CreateVersion(v, vp) => {
                (DiffTypeEnum::CreateVersion, None, Some(v), Some(vp))
            }
            DiffLogInstruction::UpdateVersion(v, vp) => {
                (DiffTypeEnum::UpdateVersion, None, Some(v), Some(vp))
            }
            DiffLogInstruction::DeleteVersion(v) => {
                (DiffTypeEnum::DeleteVersion, None, Some(v), None)
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

pub fn insert_diff_log_entries(entries: Vec<NewDiffLogEntry>, conn: &DbConnection) -> usize {
    use schema::diff_log::dsl::*;

    let rows: Vec<NewDiffLogRow> = entries.into_iter().map(|e| e.into()).collect();

    // TODO[bug]: batch this
    diesel::insert_into(diff_log)
        .values(rows)
        .execute(&conn.conn)
        .expect("Failed to insert diff log entries into DB")
}

#[cfg(test)]
mod tests {

    use std::collections::BTreeMap;
    use std::collections::HashMap;

    use crate::change_log;
    use crate::custom_types::PrereleaseTag;
    use crate::custom_types::Semver;
    use crate::diff_log::insert_diff_log_entries;
    use crate::diff_log::DiffLogRow;
    use crate::packument::Dist;
    use crate::packument::PackageOnlyPackument;
    use crate::packument::VersionOnlyPackument;
    use crate::testing;
    use crate::DbConnection;
    use chrono::Utc;
    use diesel::RunQueryDsl;
    use serde_json::Map;
    use serde_json::Value;

    use super::DiffLogEntry;
    use super::DiffLogInstruction;
    use super::NewDiffLogEntry;

    fn get_all_diff_logs(conn: &DbConnection) -> Vec<DiffLogEntry> {
        use super::schema::diff_log::dsl::*;

        // TODO[bug]: batch this
        let rows: Vec<DiffLogRow> = diff_log.load(&conn.conn).unwrap();

        rows.into_iter().map(|r| r.into()).collect()
    }

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
                seq: 102,
                package_name: "react".into(),
                instr: DiffLogInstruction::DeletePackage,
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
                seq: 102,
                package_name: "react".into(),
                instr: DiffLogInstruction::DeletePackage,
            },
            DiffLogEntry {
                id: 4,
                seq: 103,
                package_name: "react".into(),
                instr: DiffLogInstruction::CreateVersion(
                    v.clone(),
                    garbage_version_pack_data.clone(),
                ),
            },
            DiffLogEntry {
                id: 5,
                seq: 104,
                package_name: "react".into(),
                instr: DiffLogInstruction::UpdateVersion(v.clone(), garbage_version_pack_data),
            },
            DiffLogEntry {
                id: 6,
                seq: 105,
                package_name: "react".into(),
                instr: DiffLogInstruction::DeleteVersion(v),
            },
        ];

        testing::using_test_db(|conn| {
            change_log::insert_change(conn, 100, Value::Null);
            change_log::insert_change(conn, 101, Value::Null);
            change_log::insert_change(conn, 102, Value::Null);
            change_log::insert_change(conn, 103, Value::Null);
            change_log::insert_change(conn, 104, Value::Null);
            change_log::insert_change(conn, 105, Value::Null);

            let insert_count = insert_diff_log_entries(new_data, conn);
            let retrieved_data = get_all_diff_logs(conn);

            assert_eq!(retrieved_data, expected_data);
            assert_eq!(retrieved_data.len(), insert_count);
        });
    }
}
