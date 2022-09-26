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
    for chunk in deps.chunks(2000) {
        let inserted = diesel::insert_into(dependencies)
            .values(chunk)
            .on_conflict((dst_package_name, raw_spec))
            .do_update()
            .set(freq_count.eq(freq_count + excluded(freq_count)))
            .get_results::<(i64, String, Option<i64>, Value, ParsedSpec, bool, i64)>(&conn.conn)
            .expect("Error saving new dependencies");
        ids.extend(inserted.into_iter().map(|(_id, _, _, _, _, _, _)| _id));
    }

    ids
}
