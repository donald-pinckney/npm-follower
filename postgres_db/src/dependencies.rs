use std::collections::HashMap;

use crate::{connection::QueryRunner, custom_types::ParsedSpec};

use super::schema::dependencies;
use diesel::{upsert::on_constraint, Queryable};
use serde_json::Value;

use diesel::prelude::*;
use sha2::{Digest, Sha256};

#[derive(Insertable, Debug)]
#[diesel(table_name = dependencies)]
pub struct NewDependency {
    dst_package_name: String,
    dst_package_id_if_exists: Option<i64>,
    raw_spec: Value,
    spec: ParsedSpec,
    prod_freq_count: i64,
    dev_freq_count: i64,
    peer_freq_count: i64,
    optional_freq_count: i64,
    md5digest: String,
    md5digest_with_version: String,
}

pub enum DependencyType {
    Prod,
    Dev,
    Peer,
    Optional,
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
        dep_type: DependencyType,
    ) -> NewDependency {
        // sha hash of only the package name
        let mut hasher = Sha256::new();
        hasher.update(&dst_package_name);
        let md5digest = format!("{:x}", hasher.finalize_reset());

        // sha hash of both the package name and the spec
        hasher.update(format!("{}{}", dst_package_name, raw_spec));
        let md5digest_with_version = format!("{:x}", hasher.finalize());

        let (prod_freq_count, dev_freq_count, peer_freq_count, optional_freq_count) = match dep_type
        {
            DependencyType::Prod => (1, 0, 0, 0),
            DependencyType::Dev => (0, 1, 0, 0),
            DependencyType::Peer => (0, 0, 1, 0),
            DependencyType::Optional => (0, 0, 0, 1),
        };

        NewDependency {
            dst_package_name,
            dst_package_id_if_exists,
            raw_spec,
            spec,
            prod_freq_count,
            dev_freq_count,
            peer_freq_count,
            optional_freq_count,
            md5digest,
            md5digest_with_version,
        }
    }

    fn add_counts(&mut self, other: NewDependency) {
        assert_eq!(self.md5digest_with_version, other.md5digest_with_version);
        self.prod_freq_count += other.prod_freq_count;
        self.dev_freq_count += other.dev_freq_count;
        self.peer_freq_count += other.peer_freq_count;
        self.optional_freq_count += other.optional_freq_count;
    }

    pub fn get_md5digest_with_version(&self) -> &str {
        &self.md5digest_with_version
    }

    pub fn get_prod_freq_count(&self) -> i64 {
        self.prod_freq_count
    }

    pub fn get_dev_freq_count(&self) -> i64 {
        self.dev_freq_count
    }

    pub fn get_peer_freq_count(&self) -> i64 {
        self.peer_freq_count
    }

    pub fn get_optional_freq_count(&self) -> i64 {
        self.optional_freq_count
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

// returns (id, prod_freq_count, dev_freq_count, peer_freq_count, optional_freq_count)
pub fn insert_dependency_inc_counts<R>(
    conn: &mut R,
    dep: NewDependency,
) -> (i64, i64, i64, i64, i64)
where
    R: QueryRunner,
{
    use super::schema::dependencies::dsl::*;

    let insert_query = diesel::insert_into(dependencies)
        .values(&dep)
        .on_conflict(on_constraint("dependencies_md5digest_with_version_unique"))
        .do_update()
        .set((
            prod_freq_count.eq(prod_freq_count + dep.prod_freq_count),
            dev_freq_count.eq(dev_freq_count + dep.dev_freq_count),
            peer_freq_count.eq(peer_freq_count + dep.peer_freq_count),
            optional_freq_count.eq(optional_freq_count + dep.optional_freq_count),
        ))
        .returning((
            id,
            prod_freq_count,
            dev_freq_count,
            peer_freq_count,
            optional_freq_count,
        ));

    // let sql = diesel::debug_query::<diesel::pg::Pg, _>(&insert_query);
    // println!("query is: {}", sql);
    // panic!("stop");

    conn.get_result(insert_query)
        .expect("Error inserting dependency")
}

pub fn set_dependency_counts<R>(conn: &mut R, dep_id: i64, counts: (i64, i64, i64, i64))
where
    R: QueryRunner,
{
    use super::schema::dependencies::dsl::*;

    let (new_prod_freq_count, new_dev_freq_count, new_peer_freq_count, new_optional_freq_count) =
        counts;

    let update_query = diesel::update(dependencies).filter(id.eq(dep_id)).set((
        prod_freq_count.eq(new_prod_freq_count),
        dev_freq_count.eq(new_dev_freq_count),
        peer_freq_count.eq(new_peer_freq_count),
        optional_freq_count.eq(new_optional_freq_count),
    ));

    let rows = conn
        .execute(update_query)
        .expect("Error updating dependency counts");
    assert_eq!(rows, 1);
}

// pub fn insert_dependencies<R>(conn: &mut R, deps: Vec<NewDependency>) -> Vec<i64>
// where
//     R: QueryRunner,
// {
//     use super::schema::dependencies::dsl::*;

//     let all_dep_hashes: Vec<_> = deps
//         .iter()
//         .map(|dep| dep.md5digest_with_version.clone())
//         .collect();

//     let mut collapsed_deps_map: HashMap<String, NewDependency> = HashMap::new();

//     for d in deps.into_iter() {
//         if let Some(acc_dep) = collapsed_deps_map.get_mut(&d.md5digest_with_version) {
//             acc_dep.add_counts(d);
//         } else {
//             collapsed_deps_map.insert(d.md5digest_with_version.clone(), d);
//         }
//     }

//     let collapsed_deps = collapsed_deps_map.into_values();

//     let mut insert_lists_map: HashMap<(i64, i64, i64, i64), Vec<NewDependency>> = HashMap::new();

//     for d in collapsed_deps {
//         let count = (
//             d.prod_freq_count,
//             d.dev_freq_count,
//             d.peer_freq_count,
//             d.optional_freq_count,
//         );

//         if let Some(insert_list) = insert_lists_map.get_mut(&count) {
//             insert_list.push(d);
//         } else {
//             insert_lists_map.insert(count, vec![d]);
//         }
//     }

//     let insert_lists = insert_lists_map.into_iter();

//     // return 5;

//     let mut returned_ids: HashMap<String, i64> = HashMap::new();

//     for (freq_counts, insert_list) in insert_lists {
//         let insert_query = diesel::insert_into(dependencies)
//             .values(&insert_list)
//             .on_conflict(on_constraint("dependencies_md5digest_with_version_unique"))
//             .do_update()
//             .set((
//                 prod_freq_count.eq(prod_freq_count + freq_counts.0),
//                 dev_freq_count.eq(dev_freq_count + freq_counts.1),
//                 peer_freq_count.eq(peer_freq_count + freq_counts.2),
//                 optional_freq_count.eq(optional_freq_count + freq_counts.3),
//             ))
//             .returning(id);

//         // let sql = diesel::debug_query::<diesel::pg::Pg, _>(&insert_query);
//         // println!("query is: {}", sql);
//         // panic!("stop");

//         let ids: Vec<i64> = conn
//             .get_results(insert_query)
//             .expect("Error inserting dependencies");

//         // let ids: Vec<i64> = std::iter::repeat(1234).take(insert_list.len()).collect();

//         let hash_id_pairs = insert_list
//             .into_iter()
//             .map(|dep| dep.md5digest_with_version)
//             .zip(ids.into_iter());

//         for (dep_hash, dep_id) in hash_id_pairs {
//             assert!(returned_ids.insert(dep_hash, dep_id).is_none());
//         }
//     }

//     all_dep_hashes
//         .into_iter()
//         .map(|h| *returned_ids.get(&h).unwrap())
//         .collect()

//     // deps.into_iter()
//     //     .map(|dep| insert_dependency(conn, dep))
//     //     .collect()
// }

// pub fn insert_dependencies<R>(conn: &mut R, deps: Vec<NewDependency>) -> Vec<i64>
// where
//     R: QueryRunner,
// {
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
