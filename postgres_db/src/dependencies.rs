use crate::{connection::QueryRunner, custom_types::ParsedSpec};

use super::schema::dependencies;
use diesel::Queryable;
use serde_json::Value;

use diesel::prelude::*;
use sha2::{Digest, Sha256};

#[derive(Insertable, Debug)]
#[diesel(table_name = dependencies)]
// TODO: please rename this to Dependency, it throws me an error
// and idk how to fix it
pub struct NewDependency {
    pub dst_package_name: String,
    pub dst_package_id_if_exists: Option<i64>,
    pub raw_spec: Value,
    pub spec: ParsedSpec,
    pub freq_count: i64,
    pub md5digest: String,
    pub md5digest_with_version: String,
}

#[derive(Debug, Queryable)]
#[diesel(table_name = dependencies)]
pub struct Dependency {
    pub id: i64,
    pub dst_package_name: String,
    pub dst_package_id_if_exists: Option<i64>,
    pub raw_spec: Value,
    pub spec: ParsedSpec,
    pub freq_count: i64,
    pub md5digest: String,
    pub md5digest_with_version: String,
}

impl NewDependency {
    pub fn create(
        dst_package_name: String,
        dst_package_id_if_exists: Option<i64>,
        raw_spec: Value,
        spec: ParsedSpec,
        freq_count: i64,
    ) -> NewDependency {
        // sha hash of only the package name
        let mut hasher = Sha256::new();
        hasher.update(&dst_package_name);
        let md5digest = format!("{:x}", hasher.finalize_reset());

        // sha hash of both the package name and the spec
        hasher.update(format!("{}{}", dst_package_name, raw_spec));
        let md5digest_with_version = format!("{:x}", hasher.finalize());

        NewDependency {
            dst_package_name,
            dst_package_id_if_exists,
            raw_spec,
            spec,
            freq_count,
            md5digest,
            md5digest_with_version,
        }
    }
}

pub fn update_deps_missing_pack<R: QueryRunner>(conn: &mut R, pack_name: &str, pack_id: i64) {
    use super::schema::dependencies::dsl::*;

    let mut hasher = Sha256::new();
    hasher.update(pack_name);
    let name_digest = format!("{:x}", hasher.finalize());

    let update_missing_pack_query = diesel::update(dependencies)
        .filter(md5digest.eq(name_digest))
        .filter(dst_package_id_if_exists.is_null())
        .filter(dst_package_name.eq(pack_name))
        .set(dst_package_id_if_exists.eq(pack_id));

    conn.execute(update_missing_pack_query)
        .expect("Error updating dependencies");

    // // find all dependencies that have the same name digest
    // let deps = conn
    //     .load::<_, Dependency>(
    //         dependencies
    //             .filter(md5digest.eq(name_digest))
    //             .filter(dst_package_id_if_exists.is_null()),
    //     )
    //     .expect("Error loading dependencies");

    // // find the package id of the package with the same name
    // // and update the dependency with the package id
    // for dep in deps {
    //     if dep.dst_package_name == pack_name {
    //         conn.execute(
    //             diesel::update(dependencies.find(dep.id)).set(dst_package_id_if_exists.eq(pack_id)),
    //         )
    //         .expect("Error updating dependencies");
    //         break;
    //     }
    // }
}

// pub fn insert_dependencies(conn: &mut DbConnection, deps: Vec<Dependencie>) -> Vec<i64> {
//     use super::schema::dependencies::dsl::*;

//     // TODO [perf]: batch these inserts. Tried that, seemed to make it worse :(
//     let mut ids = Vec::new();
//     for dep in deps {
//         // find all deps with the same hash
//         // TODO [perf]: consider memoizing this?
//         let deps_with_same_hash: Vec<(i64, String, Value)> = dependencies
//             .select((id, dst_package_name, raw_spec))
//             .filter(md5digest_with_version.eq(&dep.md5digest_with_version))
//             .load(&mut conn.conn)
//             .expect("Error loading dependencies");

//         let insert_query = diesel::insert_into(dependencies).values(&dep).returning(id);

//         // if there are no deps with the same hash, just insert the dep
//         if deps_with_same_hash.is_empty() {
//             let inserted = insert_query
//                 .get_result::<i64>(&mut conn.conn)
//                 .unwrap_or_else(|e| {
//                     eprintln!("Got error: {}", e);
//                     eprintln!("on dep: {:?}", dep);
//                     panic!("Error inserting dependency");
//                 });
//             ids.push(inserted);
//             continue;
//         }

//         // now, find the dep with the same name and spec
//         let mut did_find_match = false;
//         for dep_with_same_hash in deps_with_same_hash {
//             if dep_with_same_hash.1 == dep.dst_package_name && dep_with_same_hash.2 == dep.raw_spec
//             {
//                 // if the dep with the same name and spec is found, just update the freq_count
//                 diesel::update(dependencies)
//                     .filter(id.eq(dep_with_same_hash.0))
//                     .set(freq_count.eq(freq_count + dep.freq_count))
//                     .execute(&mut conn.conn)
//                     .expect("Error updating dependencies");
//                 ids.push(dep_with_same_hash.0);
//                 did_find_match = true;
//                 break;
//             }
//         }

//         if !did_find_match {
//             let inserted = insert_query
//                 .get_result::<i64>(&mut conn.conn)
//                 .unwrap_or_else(|e| {
//                     eprintln!("Got error: {}", e);
//                     eprintln!("on dep: {:?}", dep);
//                     panic!("Error inserting dependency");
//                 });
//             ids.push(inserted);
//         }
//     }

//     ids
// }
