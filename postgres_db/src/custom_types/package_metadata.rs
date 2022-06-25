use chrono::{DateTime, Utc};
use diesel::pg::Pg;
use diesel::types::{ToSql, FromSql};
use diesel::deserialize;
use diesel::serialize::{self, Output, WriteTuple};
use diesel::sql_types::{Record, Nullable, Int8, Bool, Timestamptz, Jsonb};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::io::Write;
use super::sql_types::*;
use super::PackageMetadata;



// ---------- PackageMetadataStructSql <----> PackageMetadata


type PackageMetadataStructRecordSql = (
    Bool,
    Nullable<Int8>,
    Nullable<Timestamptz>,
    Nullable<Timestamptz>,
    Jsonb
);

type PackageMetadataStructRecordRust = (
    bool,
    Option<i64>,
    Option<DateTime<Utc>>,
    Option<DateTime<Utc>>,
    Value
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

impl<'a> ToSql<PackageMetadataStructSql, Pg> for PackageMetadata {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        let record: PackageMetadataStructRecordRust = match self {
            PackageMetadata::NotDeleted { 
                dist_tag_latest_version: lv, 
                created: c, 
                modified: m, 
                other_dist_tags: odts } => (
                    false, 
                    Some(*lv), 
                    Some(*c), 
                    Some(*m), 
                    dist_tag_dict_to_sql(odts.clone())
                ),
            PackageMetadata::Deleted { 
                dist_tag_latest_version: lv, 
                created: c, 
                modified: m, 
                other_dist_tags: odts } => (
                    true,
                    *lv,
                    *c,
                    *m,
                    dist_tag_dict_to_sql(odts.clone())
                )
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
            (false, Some(lv), Some(c), Some(m), odts) =>
                Ok(PackageMetadata::NotDeleted { dist_tag_latest_version: lv, created: c, modified: m, other_dist_tags: dist_tag_dict_from_sql(odts)? }),
            (true, lv, c, m, odts) => 
                Ok(PackageMetadata::Deleted { dist_tag_latest_version: lv, created: c, modified: m, other_dist_tags: dist_tag_dict_from_sql(odts)? }),
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
        let now = today.and_time(NaiveTime::from_hms(1, 2, 28)).unwrap();
        
        let empty_odts = HashMap::new();
        let some_odts = HashMap::from([("cats".into(), "1.3.5".into()), ("old".into(), "0.3.1".into())]);

        let data = vec![
            TestPackageMetadataToSql {
                id: 1,
                m: PackageMetadata::NotDeleted { dist_tag_latest_version: 5, created: now, modified: now, other_dist_tags: empty_odts.clone() }
            },
            TestPackageMetadataToSql {
                id: 2,
                m: PackageMetadata::NotDeleted { dist_tag_latest_version: 5, created: now, modified: now, other_dist_tags: some_odts.clone() }
            },
            TestPackageMetadataToSql {
                id: 3,
                m: PackageMetadata::Deleted { dist_tag_latest_version: Some(5), created: Some(now), modified: Some(now), other_dist_tags: some_odts.clone() }
            },
            TestPackageMetadataToSql {
                id: 4,
                m: PackageMetadata::Deleted { dist_tag_latest_version: None, created: None, modified: None, other_dist_tags: some_odts }
            },
            TestPackageMetadataToSql {
                id: 5,
                m: PackageMetadata::Deleted { dist_tag_latest_version: None, created: None, modified: None, other_dist_tags: empty_odts }
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
