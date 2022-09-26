use crate::custom_types::ParsedSpec;

use super::schema::dependencies;
use diesel::Queryable;
use serde_json::Value;

use super::DbConnection;
use diesel::prelude::*;

#[derive(Insertable, Debug)]
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
    pub md5digest: String,
}

#[derive(Debug)]
pub struct QueriedDependency {
    pub id: i64,
    pub dst_package_name: String,
    pub dst_package_id_if_exists: Option<i64>,
    pub raw_spec: Value,
    pub spec: ParsedSpec,
    pub secret: bool,
    pub freq_count: i64,
    pub md5digest: String,
}

impl<ST, SB: diesel::backend::Backend> Queryable<ST, SB> for QueriedDependency
where
    (
        i64,
        std::string::String,
        std::option::Option<i64>,
        serde_json::Value,
        crate::custom_types::ParsedSpec,
        bool,
        i64,
        std::string::String,
    ): diesel::deserialize::FromSqlRow<ST, SB>,
{
    type Row = (
        i64,
        String,
        Option<i64>,
        Value,
        ParsedSpec,
        bool,
        i64,
        String,
    );

    fn build(row: Self::Row) -> Self {
        QueriedDependency {
            id: row.0,
            dst_package_name: row.1,
            dst_package_id_if_exists: row.2,
            raw_spec: row.3,
            spec: row.4,
            secret: row.5,
            freq_count: row.6,
            md5digest: row.7,
        }
    }
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

        // md5 hash of both the package name and the spec
        let md5digest = format!(
            "{:x}",
            md5::compute(format!("{}{}", dst_package_name, raw_spec))
        );

        Dependencie {
            dst_package_name,
            dst_package_id_if_exists,
            raw_spec,
            spec,
            secret,
            freq_count,
            md5digest,
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

    let mut ids = Vec::new();
    for dep in deps {
        // find all deps with the same hash
        let deps_with_same_hash: Vec<QueriedDependency> = dependencies
            .filter(md5digest.eq(&dep.md5digest))
            .load(&conn.conn)
            .expect("Error loading dependencies");

        // if there are no deps with the same hash, just insert the dep
        if deps_with_same_hash.is_empty() {
            let inserted = diesel::insert_into(dependencies)
                .values(&dep)
                .get_result::<QueriedDependency>(&conn.conn)
                .unwrap_or_else(|e| {
                    eprintln!("Got error: {}", e);
                    eprintln!("on dep: {:?}", dep);
                    panic!("Error inserting dependency");
                });
            ids.push(inserted.id);
            continue;
        }

        // now, find the dep with the same name and spec
        for dep_with_same_hash in deps_with_same_hash {
            if dep_with_same_hash.dst_package_name == dep.dst_package_name
                && dep_with_same_hash.raw_spec == dep.raw_spec
            {
                // if the dep with the same name and spec is found, just update the freq_count
                diesel::update(dependencies)
                    .filter(id.eq(dep_with_same_hash.id))
                    .set(freq_count.eq(freq_count + dep.freq_count))
                    .execute(&conn.conn)
                    .expect("Error updating dependencies");
                ids.push(dep_with_same_hash.id);
                break;
            }
        }
    }

    ids
}
