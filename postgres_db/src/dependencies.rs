use crate::custom_types::PackageMetadata;
use crate::custom_types::ParsedSpec;
use crate::custom_types::RepoInfo;
use crate::custom_types::Semver;

use super::schema::dependencies;
use chrono::{DateTime, Utc};
use diesel::pg::upsert::excluded;
use diesel::sql_types::BigInt;
use diesel::Queryable;
use serde_json::Value;

use super::schema;
use super::DbConnection;
use diesel::prelude::*;

#[derive(Queryable, Insertable, Debug)]
#[diesel(table_name = dependencies)]
// TODO: please rename this to Dependency, it throws me an error
// and idk how to fix it
pub struct Dependencie {
    pub dst_package_name: String,
    pub dst_package_id_if_exists: Option<i64>,
    pub raw_spec: Value,
    pub spec: ParsedSpec,
    pub secret: bool,
    pub freq_count: i64,
}

impl Dependencie {
    pub fn create(
        dst_package_name: String,
        dst_package_id_if_exists: Option<i64>,
        raw_spec: Value,
        spec: ParsedSpec,
        secret: bool,
        freq_count: i64,
    ) -> Dependencie {
        // trim the package name to max 1000 chars
        let dst_package_name = {
            if dst_package_name.len() > 1000 {
                eprintln!("WARNING: package name is too long, trimming it");
                format!("{}...", &dst_package_name[..1000])
            } else {
                dst_package_name
            }
        };

        Dependencie {
            dst_package_name,
            dst_package_id_if_exists,
            raw_spec,
            spec,
            secret,
            freq_count,
        }
    }
}

pub fn update_deps_missing_pack(conn: &DbConnection, pack_name: &str, pack_id: i64) {
    use super::schema::dependencies::dsl::*;

    diesel::update(dependencies)
        .filter(dst_package_name.eq(pack_name))
        .set(dst_package_id_if_exists.eq(pack_id))
        .execute(&conn.conn)
        .expect("Error updating dependencies");
}

pub fn insert_dependencies(conn: &DbConnection, deps: Vec<Dependencie>) -> Vec<i64> {
    use super::schema::dependencies::dsl::*;

    // chunking the dependencies to avoid the 2000 limit
    let mut ids = Vec::new();
    for dep in deps {
        let inserted = diesel::insert_into(dependencies)
            .values(&dep)
            .on_conflict((dst_package_name, raw_spec))
            .do_update()
            .set(freq_count.eq(freq_count + excluded(freq_count)))
            .get_result::<(i64, String, Option<i64>, Value, ParsedSpec, bool, i64)>(&conn.conn)
            .unwrap_or_else(|e| {
                eprintln!("Got error: {}", e);
                eprintln!("on dep: {:?}", dep);
                panic!("Error inserting dependency");
            });
        ids.push(inserted.0);
    }

    ids
}
