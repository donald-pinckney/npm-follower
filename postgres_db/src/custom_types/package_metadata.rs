use chrono::{DateTime, Utc};
use deepsize::DeepSizeOf;
use diesel::deserialize;
use diesel::deserialize::FromSql;
use diesel::pg::Pg;
use diesel::pg::PgValue;
use diesel::serialize::ToSql;
use diesel::serialize::{self, IsNull, Output, WriteTuple};
use diesel::sql_types::{Int8, Nullable, Record, Timestamptz};
use std::io::Write;

use crate::schema::sql_types::PackageState;
use crate::schema::sql_types::PackageStateEnum;

#[derive(Debug, PartialEq, Eq, FromSqlRow, AsExpression, Clone, DeepSizeOf)]
#[diesel(sql_type = PackageStateEnum)]
pub enum PackageStateType {
    Normal,
    Unpublished,
    Deleted,
}

impl ToSql<PackageStateEnum, Pg> for PackageStateType {
    fn to_sql(&self, out: &mut Output<Pg>) -> serialize::Result {
        match *self {
            PackageStateType::Normal => out.write_all(b"normal")?,
            PackageStateType::Unpublished => out.write_all(b"unpublished")?,
            PackageStateType::Deleted => out.write_all(b"deleted")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<PackageStateEnum, Pg> for PackageStateType {
    fn from_sql(bytes: PgValue) -> deserialize::Result<Self> {
        let bytes = bytes.as_bytes();

        match bytes {
            b"normal" => Ok(PackageStateType::Normal),
            b"unpublished" => Ok(PackageStateType::Unpublished),
            b"deleted" => Ok(PackageStateType::Deleted),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}

#[derive(Debug, PartialEq, Eq, FromSqlRow, AsExpression, Clone, DeepSizeOf)]
#[diesel(sql_type = PackageState)]
pub struct PackageStateTimePoint {
    pub state: PackageStateType,
    pub seq: i64,
    pub diff_entry_id: i64,
    pub estimated_time: Option<DateTime<Utc>>,
}

impl ToSql<PackageState, Pg> for PackageStateTimePoint {
    fn to_sql(&self, out: &mut Output<Pg>) -> serialize::Result {
        WriteTuple::<(PackageStateEnum, Int8, Int8, Nullable<Timestamptz>)>::write_tuple(
            &(
                &self.state,
                self.seq,
                self.diff_entry_id,
                self.estimated_time,
            ),
            out,
        )
    }
}

impl FromSql<PackageState, Pg> for PackageStateTimePoint {
    fn from_sql(bytes: PgValue) -> deserialize::Result<Self> {
        let (state, seq, diff_entry_id, estimated_time): (
            PackageStateType,
            i64,
            i64,
            Option<DateTime<Utc>>,
        ) = FromSql::<Record<(PackageStateEnum, Int8, Int8, Nullable<Timestamptz>)>, Pg>::from_sql(
            bytes,
        )?;
        Ok(PackageStateTimePoint {
            state,
            seq,
            diff_entry_id,
            estimated_time,
        })
    }
}

use crate::schema::sql_types::VersionState;
use crate::schema::sql_types::VersionStateEnum;

#[derive(Debug, PartialEq, FromSqlRow, AsExpression)]
#[diesel(sql_type = VersionStateEnum)]
pub enum VersionStateType {
    Normal,
    Unpublished,
    Deleted,
}

impl ToSql<VersionStateEnum, Pg> for VersionStateType {
    fn to_sql(&self, out: &mut Output<Pg>) -> serialize::Result {
        match *self {
            VersionStateType::Normal => out.write_all(b"normal")?,
            VersionStateType::Unpublished => out.write_all(b"unpublished")?,
            VersionStateType::Deleted => out.write_all(b"deleted")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<VersionStateEnum, Pg> for VersionStateType {
    fn from_sql(bytes: PgValue) -> deserialize::Result<Self> {
        let bytes = bytes.as_bytes();

        match bytes {
            b"normal" => Ok(VersionStateType::Normal),
            b"unpublished" => Ok(VersionStateType::Unpublished),
            b"deleted" => Ok(VersionStateType::Deleted),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}

#[derive(Debug, PartialEq, FromSqlRow, AsExpression)]
#[diesel(sql_type = VersionState)]
pub struct VersionStateTimePoint {
    pub state: VersionStateType,
    pub seq: i64,
    pub diff_entry_id: i64,
    pub estimated_time: Option<DateTime<Utc>>,
}

impl ToSql<VersionState, Pg> for VersionStateTimePoint {
    fn to_sql(&self, out: &mut Output<Pg>) -> serialize::Result {
        WriteTuple::<(VersionStateEnum, Int8, Int8, Nullable<Timestamptz>)>::write_tuple(
            &(
                &self.state,
                self.seq,
                self.diff_entry_id,
                self.estimated_time,
            ),
            out,
        )
    }
}

impl FromSql<VersionState, Pg> for VersionStateTimePoint {
    fn from_sql(bytes: PgValue) -> deserialize::Result<Self> {
        let (state, seq, diff_entry_id, estimated_time): (
            VersionStateType,
            i64,
            i64,
            Option<DateTime<Utc>>,
        ) = FromSql::<Record<(VersionStateEnum, Int8, Int8, Nullable<Timestamptz>)>, Pg>::from_sql(
            bytes,
        )?;
        Ok(VersionStateTimePoint {
            state,
            seq,
            diff_entry_id,
            estimated_time,
        })
    }
}

// ---------- PackageMetadataStruct <----> PackageMetadata

// type PackageMetadataStructRecordSql = (
//     PackageStateSql,
//     Nullable<Int8>,
//     Nullable<Timestamptz>,
//     Nullable<Timestamptz>,
//     Nullable<Jsonb>,
//     Nullable<Jsonb>,
//     Nullable<Jsonb>,
// );

// type PackageMetadataStructRecordRust = (
//     PackageState,
//     Option<i64>,
//     Option<DateTime<Utc>>,
//     Option<DateTime<Utc>>,
//     Option<Value>,
//     Option<Value>,
//     Option<Value>,
// );

// fn dist_tag_dict_from_sql(v: Value) -> Result<BTreeMap<String, String>, serde_json::Error> {
//     let mv = serde_json::from_value::<Map<String, Value>>(v)?;
//     let mut result = BTreeMap::new();
//     for (k, kv) in mv.into_iter() {
//         result.insert(k, serde_json::from_value::<String>(kv)?);
//     }
//     Ok(result)
// }

// // TODO[perf]: optimize this out?
// fn dist_tag_dict_to_sql(m: BTreeMap<String, String>) -> Value {
//     Value::Object(m.into_iter().map(|(k, v)| (k, Value::String(v))).collect())
// }

// // TODO[perf]: optimize this out?
// fn other_times_dict_from_sql(
//     v: Value,
// ) -> Result<BTreeMap<Semver, DateTime<Utc>>, serde_json::Error> {
//     let mv = serde_json::from_value::<Map<String, Value>>(v)?;
//     let mut result = BTreeMap::new();
//     for (k, kv) in mv.into_iter() {
//         result.insert(
//             serde_json::from_str::<Semver>(&k)?,
//             serde_json::from_value::<DateTime<Utc>>(kv)?,
//         );
//     }
//     Ok(result)
// }

// // TODO[perf]: optimize this out?
// fn other_times_dict_to_sql(m: BTreeMap<Semver, DateTime<Utc>>) -> Value {
//     Value::Object(
//         m.into_iter()
//             .map(|(k, v)| {
//                 (
//                     serde_json::to_string(&k).unwrap(),
//                     serde_json::to_value(v).unwrap(),
//                 )
//             })
//             .collect(),
//     )
// }

// impl<'a> ToSql<PackageMetadataStruct, Pg> for PackageMetadata {
//     fn to_sql(&self, out: &mut Output<Pg>) -> serialize::Result {
//         let record: PackageMetadataStructRecordRust = match self {
//             PackageMetadata::Normal {
//                 dist_tag_latest_version: lv,
//                 created: c,
//                 modified: m,
//                 other_dist_tags: odts,
//             } => (
//                 PackageState::Normal,
//                 *lv,
//                 Some(*c),
//                 Some(*m),
//                 Some(dist_tag_dict_to_sql(odts.clone())),
//                 None,
//                 None,
//             ),
//             PackageMetadata::Unpublished {
//                 created: c,
//                 modified: m,
//                 other_time_data: otd,
//                 unpublished_data: ud,
//             } => (
//                 PackageState::Unpublished,
//                 None,
//                 Some(*c),
//                 Some(*m),
//                 None,
//                 Some(other_times_dict_to_sql(otd.clone())),
//                 Some(ud.clone()),
//             ),

//             PackageMetadata::Deleted => (PackageState::Deleted, None, None, None, None, None, None),
//         };

//         WriteTuple::<PackageMetadataStructRecordSql>::write_tuple(&record, out)
//     }
// }

// impl<'a> FromSql<PackageMetadataStruct, Pg> for PackageMetadata {
//     fn from_sql(bytes: PgValue) -> deserialize::Result<Self> {
//         let tup: PackageMetadataStructRecordRust =
//             FromSql::<Record<PackageMetadataStructRecordSql>, Pg>::from_sql(bytes)?;
//         match tup {
//             (PackageState::Normal, lv, Some(c), Some(m), Some(odts), None, None) => {
//                 Ok(PackageMetadata::Normal {
//                     dist_tag_latest_version: lv,
//                     created: c,
//                     modified: m,
//                     other_dist_tags: dist_tag_dict_from_sql(odts)?,
//                 })
//             }
//             (PackageState::Unpublished, None, Some(c), Some(m), None, Some(otd), Some(ud)) => {
//                 Ok(PackageMetadata::Unpublished {
//                     created: c,
//                     modified: m,
//                     other_time_data: other_times_dict_from_sql(otd)?,
//                     unpublished_data: ud,
//                 })
//             }
//             (PackageState::Deleted, None, None, None, None, None, None) => {
//                 Ok(PackageMetadata::Deleted)
//             }
//             _ => Err("Unrecognized enum variant".into()),
//         }
//     }
// }

// // Unit tests
// #[cfg(test)]
// mod tests {
//     use std::collections::BTreeMap;

//     use crate::connection::QueryRunner;
//     use crate::custom_types::PackageMetadata;
//     use crate::custom_types::Semver;
//     use crate::testing;
//     use chrono::NaiveTime;
//     use chrono::Utc;
//     use diesel::prelude::*;

//     table! {
//         use diesel::sql_types::*;
//         use crate::schema::sql_types::PackageMetadataStruct;

//         test_package_metadata_to_sql {
//             id -> Integer,
//             m -> PackageMetadataStruct,
//         }
//     }

//     #[derive(Insertable, Queryable, Identifiable, Debug, PartialEq)]
//     #[diesel(table_name = test_package_metadata_to_sql)]
//     struct TestPackageMetadataToSql {
//         id: i32,
//         m: PackageMetadata,
//     }

//     #[test]
//     fn test_package_metadata_to_sql_fn() {
//         use self::test_package_metadata_to_sql::dsl::*;

//         let today = Utc::today();
//         let date1 = today.and_time(NaiveTime::from_hms(1, 2, 28)).unwrap();
//         let date2 = today.and_time(NaiveTime::from_hms(4, 3, 12)).unwrap();

//         let ud = serde_json::Value::Object(serde_json::Map::from_iter(
//             vec![("dogs".to_string(), serde_json::Value::String("mice".into()))].into_iter(),
//         ));

//         let empty_odts = BTreeMap::new();
//         let some_odts = BTreeMap::from([
//             ("cats".into(), "1.3.5".into()),
//             ("old".into(), "0.3.1".into()),
//         ]);

//         let empty_otd = BTreeMap::new();
//         let some_otd = BTreeMap::from([(
//             Semver {
//                 major: 1,
//                 minor: 2,
//                 bug: 3,
//                 prerelease: vec![],
//                 build: vec![],
//             },
//             date2,
//         )]);

//         let data = vec![
//             TestPackageMetadataToSql {
//                 id: 1,
//                 m: PackageMetadata::Normal {
//                     dist_tag_latest_version: Some(5),
//                     created: date1,
//                     modified: date2,
//                     other_dist_tags: empty_odts,
//                 },
//             },
//             TestPackageMetadataToSql {
//                 id: 2,
//                 m: PackageMetadata::Normal {
//                     dist_tag_latest_version: None,
//                     created: date2,
//                     modified: date1,
//                     other_dist_tags: some_odts,
//                 },
//             },
//             TestPackageMetadataToSql {
//                 id: 3,
//                 m: PackageMetadata::Unpublished {
//                     created: date1,
//                     modified: date2,
//                     other_time_data: empty_otd,
//                     unpublished_data: ud.clone(),
//                 },
//             },
//             TestPackageMetadataToSql {
//                 id: 4,
//                 m: PackageMetadata::Unpublished {
//                     created: date2,
//                     modified: date1,
//                     other_time_data: some_otd,
//                     unpublished_data: ud,
//                 },
//             },
//             TestPackageMetadataToSql {
//                 id: 5,
//                 m: PackageMetadata::Deleted,
//             },
//         ];

//         testing::using_test_db(|conn| {
//             testing::using_temp_table(
//                 conn,
//                 "test_package_metadata_to_sql",
//                 "id SERIAL PRIMARY KEY, m package_metadata NOT NULL",
//                 |conn| {
//                     let inserted = conn
//                         .get_results(
//                             diesel::insert_into(test_package_metadata_to_sql).values(&data),
//                         )
//                         .unwrap();
//                     assert_eq!(data, inserted);

//                     let filter_all = conn
//                         .load(test_package_metadata_to_sql.filter(id.ge(1)))
//                         .unwrap();
//                     assert_eq!(data, filter_all);
//                 },
//             );
//         });
//     }
// }
