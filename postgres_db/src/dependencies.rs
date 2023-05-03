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
    pub prod_freq_count: i64,
    pub dev_freq_count: i64,
    pub peer_freq_count: i64,
    pub optional_freq_count: i64,
    pub md5digest: String,
    pub md5digest_with_version: String,
}

impl Dependency {
    pub fn mark_as_delete(self, dep_type: DependencyType) -> Dependency {
        match dep_type {
            DependencyType::Prod => Dependency {
                prod_freq_count: -1,
                dev_freq_count: 0,
                peer_freq_count: 0,
                optional_freq_count: 0,
                ..self
            },
            DependencyType::Dev => Dependency {
                prod_freq_count: 0,
                dev_freq_count: -1,
                peer_freq_count: 0,
                optional_freq_count: 0,
                ..self
            },
            DependencyType::Peer => Dependency {
                prod_freq_count: 0,
                dev_freq_count: 0,
                peer_freq_count: -1,
                optional_freq_count: 0,
                ..self
            },
            DependencyType::Optional => Dependency {
                prod_freq_count: 0,
                dev_freq_count: 0,
                peer_freq_count: 0,
                optional_freq_count: -1,
                ..self
            },
        }
    }

    pub fn mark_as_add(self, dep_type: DependencyType) -> Dependency {
        match dep_type {
            DependencyType::Prod => Dependency {
                prod_freq_count: 1,
                dev_freq_count: 0,
                peer_freq_count: 0,
                optional_freq_count: 0,
                ..self
            },
            DependencyType::Dev => Dependency {
                prod_freq_count: 0,
                dev_freq_count: 1,
                peer_freq_count: 0,
                optional_freq_count: 0,
                ..self
            },
            DependencyType::Peer => Dependency {
                prod_freq_count: 0,
                dev_freq_count: 0,
                peer_freq_count: 1,
                optional_freq_count: 0,
                ..self
            },
            DependencyType::Optional => Dependency {
                prod_freq_count: 0,
                dev_freq_count: 0,
                peer_freq_count: 0,
                optional_freq_count: 1,
                ..self
            },
        }
    }

    pub fn as_new(self) -> NewDependency {
        NewDependency {
            dst_package_name: self.dst_package_name,
            dst_package_id_if_exists: self.dst_package_id_if_exists,
            raw_spec: self.raw_spec,
            spec: self.spec,
            prod_freq_count: self.prod_freq_count,
            dev_freq_count: self.dev_freq_count,
            peer_freq_count: self.peer_freq_count,
            optional_freq_count: self.optional_freq_count,
            md5digest: self.md5digest,
            md5digest_with_version: self.md5digest_with_version,
        }
    }
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

pub fn get_dependency_by_id<R>(conn: &mut R, dep_id: i64) -> Dependency
where
    R: QueryRunner,
{
    use super::schema::dependencies::dsl::*;

    conn.get_result(dependencies.filter(id.eq(dep_id)))
        .expect("Error getting dep by id")
}
