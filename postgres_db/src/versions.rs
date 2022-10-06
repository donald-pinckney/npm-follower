use crate::custom_types::RepoInfo;
use crate::custom_types::Semver;

use super::schema::versions;
use chrono::{DateTime, Utc};
use diesel::pg::upsert::excluded;
use diesel::Queryable;
use serde_json::Value;

use super::DbConnection;
use diesel::prelude::*;

#[derive(Queryable, Insertable, Debug)]
#[diesel(table_name = versions)]
pub struct Version {
    pub package_id: i64,
    pub semver: Semver,
    pub tarball_url: String,
    pub repository_raw: Option<Value>,
    pub repository_parsed: Option<RepoInfo>,
    pub created: DateTime<Utc>,
    pub deleted: bool,
    pub extra_metadata: Value,
    pub prod_dependencies: Vec<i64>, // this is a list of version ids
    pub dev_dependencies: Vec<i64>,  // this is a list of version ids
    pub peer_dependencies: Vec<i64>, // this is a list of version ids
    pub optional_dependencies: Vec<i64>, // this is a list of version ids
    pub secret: bool,
}

impl Version {
    pub fn create(
        package_id: i64,
        semver: Semver,
        tarball_url: String,
        repository_raw: Option<Value>,
        repository_parsed: Option<RepoInfo>,
        created: DateTime<Utc>,
        deleted: bool,
        extra_metadata: Value,
        prod_dependencies: Vec<i64>,
        dev_dependencies: Vec<i64>,
        peer_dependencies: Vec<i64>,
        optional_dependencies: Vec<i64>,
        secret: bool,
    ) -> Version {
        Version {
            package_id,
            semver,
            tarball_url,
            repository_raw,
            repository_parsed,
            created,
            deleted,
            extra_metadata,
            prod_dependencies,
            dev_dependencies,
            peer_dependencies,
            optional_dependencies,
            secret,
        }
    }
}


pub fn insert_versions(conn: &DbConnection, version_vec: Vec<Version>) -> Vec<(i64, Semver)> {
    use super::schema::versions::dsl::*;

    let semvers: Vec<_> = version_vec.iter().map(|v| v.semver.clone()).collect();

    // TODO [perf]: This insert is fairly slow, but we are doing it more often than needed.
    // We only need to do this if either:
    // a) the package is new, or
    // b) the version metadata changed

    // TODO [perf]: We should batch these together and insert multiple
    // versions at once 
    let ids: Vec<i64> = diesel::insert_into(versions)
        .values(version_vec)
        .on_conflict((package_id, semver))
        .do_update()
        .set((
            tarball_url.eq(excluded(tarball_url)),
            repository_raw.eq(excluded(repository_raw)),
            repository_parsed.eq(excluded(repository_parsed)),
            created.eq(excluded(created)),
            deleted.eq(excluded(deleted)),
            extra_metadata.eq(excluded(extra_metadata)),
            prod_dependencies.eq(excluded(prod_dependencies)),
            dev_dependencies.eq(excluded(dev_dependencies)),
            peer_dependencies.eq(excluded(peer_dependencies)),
            optional_dependencies.eq(excluded(optional_dependencies)),
            secret.eq(excluded(secret)),
        ))
        .returning(id)
        .get_results::<i64>(&conn.conn)
        .expect("Error saving new version");
    
    assert!(ids.len() == semvers.len());
    ids.into_iter().zip(semvers.into_iter()).collect()
}

pub fn delete_versions_not_in(conn: &DbConnection, pkg_id: i64, vers: Vec<&Semver>) {
    use super::schema::versions::dsl::*;

    println!("The maybe slow query is running!");
    
    // get all versions with the given package id
    let all_vers = versions
        .filter(package_id.eq(pkg_id))
        .load::<(
            i64,
            i64,
            Semver,
            String,
            Option<Value>,
            Option<RepoInfo>,
            DateTime<Utc>,
            bool,
            Value,
            Vec<i64>,
            Vec<i64>,
            Vec<i64>,
            Vec<i64>,
            bool,
        )>(&conn.conn)
        .expect("Error loading versions");

    for (ver_id, _, server_semver, _, _, _, _, _, _, _, _, _, _, _) in &all_vers {
        if !vers.contains(&server_semver) {
            diesel::update(versions.find(ver_id))
                .set(deleted.eq(true))
                .execute(&conn.conn)
                .expect("Error deleting version");
        }
    }
}
