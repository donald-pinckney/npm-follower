use chrono::{DateTime, Utc};
use diesel::pg::Pg;
use diesel::types::{ToSql, FromSql};
use diesel::deserialize;
use diesel::serialize::{self, Output, WriteTuple, IsNull};
use diesel::sql_types::{Record, Nullable, Int8, Timestamptz, Jsonb};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::io::Write;
use super::sql_types::*;
use super::PackageMetadata;



// ---------- PackageMetadataStructSql <----> PackageMetadata


type PackageMetadataStructRecordSql = (
    PackageStateSql,
    Nullable<Int8>,
    Nullable<Timestamptz>,
    Nullable<Timestamptz>,
    Nullable<Jsonb>,
    Nullable<Jsonb>,
    Nullable<Jsonb>
);

type PackageMetadataStructRecordRust = (
    PackageState,
    Option<i64>,
    Option<DateTime<Utc>>,
    Option<DateTime<Utc>>,
    Option<Value>,
    Option<Value>,
    Option<Value>
);

fn dist_tag_dict_from_sql(v: Value) -> Result<HashMap<String, String>, serde_json::Error> {
    let mv = serde_json::from_value::<Map<String, Value>>(v)?;
    let mut result = HashMap::new();
    for (k, kv) in mv.into_iter() {
        result.insert(k, serde_json::from_value::<String>(kv)?);
    }
    Ok(result)
}

fn dist_tag_dict_to_sql(m: HashMap<String, String>) -> Value {
    Value::Object(m.into_iter().map(|(k, v)| (k, Value::String(v))).collect())
}

fn other_times_dict_from_sql(v: Value) -> Result<HashMap<String, DateTime<Utc>>, serde_json::Error> {
    let mv = serde_json::from_value::<Map<String, Value>>(v)?;
    let mut result = HashMap::new();
    for (k, kv) in mv.into_iter() {
        result.insert(k, serde_json::from_value::<DateTime<Utc>>(kv)?);
    }
    Ok(result)
}

fn other_times_dict_to_sql(m: HashMap<String, DateTime<Utc>>) -> Value {
    Value::Object(m.into_iter().map(|(k, v)| (k, serde_json::to_value(v).unwrap())).collect())
}



impl<'a> ToSql<PackageMetadataStructSql, Pg> for PackageMetadata {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        let record: PackageMetadataStructRecordRust = match self {
            PackageMetadata::Normal { 
                dist_tag_latest_version: lv, 
                created: c, 
                modified: m, 
                other_dist_tags: odts } => (
                    PackageState::Normal, 
                    *lv, 
                    Some(*c), 
                    Some(*m), 
                    Some(dist_tag_dict_to_sql(odts.clone())),
                    None,
                    None
                ),
            PackageMetadata::Unpublished { 
                created: c, 
                modified: m, 
                other_time_data: otd,
                unpublished_data: ud } => (
                    PackageState::Unpublished,
                    None,
                    Some(*c),
                    Some(*m),
                    None,
                    Some(other_times_dict_to_sql(otd.clone())),
                    Some(ud.clone())
                ),

            PackageMetadata::Deleted => (PackageState::Deleted, None, None, None, None, None, None)
        };

        WriteTuple::<PackageMetadataStructRecordSql>::write_tuple(
            &record,
            out
        )
    }
}

impl<'a> FromSql<PackageMetadataStructSql, Pg> for PackageMetadata {
    fn from_sql(bytes: Option<&[u8]>) -> deserialize::Result<Self> {
        let tup: PackageMetadataStructRecordRust = FromSql::<Record<PackageMetadataStructRecordSql>, Pg>::from_sql(bytes)?;
        match tup {
            (PackageState::Normal, lv, Some(c), Some(m), Some(odts), None, None) =>
                Ok(PackageMetadata::Normal { dist_tag_latest_version: lv, created: c, modified: m, other_dist_tags: dist_tag_dict_from_sql(odts)? }),
            (PackageState::Unpublished, None, Some(c), Some(m), None, Some(otd), Some(ud)) => 
                Ok(PackageMetadata::Unpublished { created: c, modified: m, other_time_data: other_times_dict_from_sql(otd)?, unpublished_data: ud }),
            (PackageState::Deleted, None, None, None, None, None, None) => Ok(PackageMetadata::Deleted),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}




#[derive(SqlType)]
#[postgres(type_name = "package_state_enum")]
struct PackageStateSql;

#[derive(Debug, PartialEq, FromSqlRow, AsExpression)]
#[sql_type = "PackageStateSql"]
enum PackageState {
    Normal,
    Unpublished,
    Deleted
}

impl ToSql<PackageStateSql, Pg> for PackageState {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        match *self {
            PackageState::Normal => out.write_all(b"normal")?,
            PackageState::Unpublished => out.write_all(b"unpublished")?,
            PackageState::Deleted => out.write_all(b"deleted")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<PackageStateSql, Pg> for PackageState {
    fn from_sql(bytes: Option<&[u8]>) -> deserialize::Result<Self> {
        match not_none!(bytes) {
            b"normal" => Ok(PackageState::Normal),
            b"unpublished" => Ok(PackageState::Unpublished),
            b"deleted" => Ok(PackageState::Deleted),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}





// Unit tests
#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chrono::NaiveTime;
    use chrono::Utc;
    use diesel::prelude::*;
    use diesel::RunQueryDsl;
    use crate::custom_types::PackageMetadata;
    use crate::testing;

    table! {
        use diesel::sql_types::*;
        use crate::custom_types::sql_type_names::Package_metadata_struct;

        test_package_metadata_to_sql {
            id -> Integer,
            m -> Package_metadata_struct,
        }
    }

    #[derive(Insertable, Queryable, Identifiable, Debug, PartialEq)]
    #[table_name = "test_package_metadata_to_sql"]
    struct TestPackageMetadataToSql {
        id: i32,
        m: PackageMetadata,
    }

    #[test]
    fn test_package_metadata_to_sql_fn() {
        use self::test_package_metadata_to_sql::dsl::*;

        let today = Utc::today();
        let date1 = today.and_time(NaiveTime::from_hms(1, 2, 28)).unwrap();
        let date2 = today.and_time(NaiveTime::from_hms(4, 3, 12)).unwrap();

        let ud = serde_json::Value::Object(serde_json::Map::from_iter(vec![("dogs".to_string(), serde_json::Value::String("mice".into()))].into_iter()));

        let empty_odts = HashMap::new();
        let some_odts = HashMap::from([("cats".into(), "1.3.5".into()), ("old".into(), "0.3.1".into())]);

        let empty_otd = HashMap::new();
        let some_otd = HashMap::from([("1.2.3".into(), date2)]);

        let data = vec![
            TestPackageMetadataToSql {
                id: 1,
                m: PackageMetadata::Normal { dist_tag_latest_version: Some(5), created: date1, modified: date2, other_dist_tags: empty_odts.clone() }
            },
            TestPackageMetadataToSql {
                id: 2,
                m: PackageMetadata::Normal { dist_tag_latest_version: None, created: date2, modified: date1, other_dist_tags: some_odts.clone() }
            },
            TestPackageMetadataToSql {
                id: 3,
                m: PackageMetadata::Unpublished { created: date1, modified: date2, other_time_data: empty_otd.clone(), unpublished_data: ud.clone() }
            },
            TestPackageMetadataToSql {
                id: 4,
                m: PackageMetadata::Unpublished { created: date2, modified: date1, other_time_data: some_otd.clone(), unpublished_data: ud.clone() }
            },
            TestPackageMetadataToSql {
                id: 5,
                m: PackageMetadata::Deleted
            }
        ];



        let conn = testing::test_connect();
        let _temp_table = testing::TempTable::new(&conn, "test_package_metadata_to_sql", "id SERIAL PRIMARY KEY, m package_metadata NOT NULL");

        let inserted = diesel::insert_into(test_package_metadata_to_sql).values(&data).get_results(&conn.conn).unwrap();
        assert_eq!(data, inserted);

        let filter_all = test_package_metadata_to_sql
            .filter(id.ge(1))
            .load(&conn.conn)
            .unwrap();
        assert_eq!(data, filter_all);
    }
}
