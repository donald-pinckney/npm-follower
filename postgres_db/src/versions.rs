use crate::custom_types::RepoInfo;
use crate::custom_types::Semver;
use crate::custom_types::VersionStateTimePoint;
use crate::custom_types::VersionStateType;

use super::schema::versions;
use chrono::{DateTime, Utc};
use diesel::pg::upsert::excluded;
use diesel::Queryable;
use serde_json::Value;

use super::schema;
use crate::connection::DbConnection;
use crate::connection::QueryRunner;
use crate::custom_types::DiffTypeEnum;
use crate::custom_types::PackageStateTimePoint;
use crate::custom_types::PackageStateType;
use crate::packument::PackageOnlyPackument;
use crate::packument::VersionOnlyPackument;
use diesel::insert_into;
use diesel::prelude::*;
use diesel::Insertable;
use serde::Deserialize;
use serde::Serialize;

#[derive(Queryable, Debug)]
#[diesel(table_name = versions)]
pub struct Version {
    pub id: i64,
    pub package_id: i64,
    pub semver: Semver,

    pub current_version_state_type: VersionStateType,
    pub version_state_history: Vec<VersionStateTimePoint>,

    pub tarball_url: String,
    pub repository_raw: Option<Value>,
    pub repository_parsed: Option<RepoInfo>,
    pub created: DateTime<Utc>,
    pub extra_metadata: Value,

    pub prod_dependencies: Vec<i64>, // this is a list of version ids
    pub dev_dependencies: Vec<i64>,  // this is a list of version ids
    pub peer_dependencies: Vec<i64>, // this is a list of version ids
    pub optional_dependencies: Vec<i64>, // this is a list of version ids
}

pub fn get_version_by_semver<R: QueryRunner>(conn: &mut R, package_id: i64, v: Semver) -> Version {
    let query = versions::table.filter(
        versions::package_id
            .eq(package_id)
            .and(versions::semver.eq(v)),
    );
    conn.get_result(query).expect("Error getting package")
}

// TODO[perf]: memoize this?
pub fn get_version_id_by_semver<R: QueryRunner>(conn: &mut R, package_id: i64, v: Semver) -> i64 {
    let query = versions::table
        .filter(
            versions::package_id
                .eq(package_id)
                .and(versions::semver.eq(v)),
        )
        .select(versions::id);
    conn.get_result(query).expect("Error getting package")
}

// impl Version {
//     pub fn create(
//         package_id: i64,
//         semver: Semver,
//         tarball_url: String,
//         repository_raw: Option<Value>,
//         repository_parsed: Option<RepoInfo>,
//         created: DateTime<Utc>,
//         deleted: bool,
//         extra_metadata: Value,
//         prod_dependencies: Vec<i64>,
//         dev_dependencies: Vec<i64>,
//         peer_dependencies: Vec<i64>,
//         optional_dependencies: Vec<i64>,
//         secret: bool,
//     ) -> Version {
//         Version {
//             package_id,
//             semver,
//             tarball_url,
//             repository_raw,
//             repository_parsed,
//             created,
//             deleted,
//             extra_metadata,
//             prod_dependencies,
//             dev_dependencies,
//             peer_dependencies,
//             optional_dependencies,
//             secret,
//         }
//     }
// }

// pub fn insert_versions(conn: &mut DbConnection, version_vec: Vec<Version>) -> Vec<(i64, Semver)> {
//     use super::schema::versions::dsl::*;

//     let semvers: Vec<_> = version_vec.iter().map(|v| v.semver.clone()).collect();

//     // TODO [perf]: This insert is fairly slow, but we are doing it more often than needed.
//     // We only need to do this if either:
//     // a) the version is new, or
//     // b) the version metadata changed. Let's assume that the version metadata is immutable, and rule this out.

//     // Thus, we only have to insert versions which are new. There are 2 cases for versions being new:
//     // a) the package is new, in which case all versions are new, so we have to insert all, and there are no conflicts
//     // b) or the package already exists, but there are new versions.

//     // println!("UPDATE");
//     // TODO [bug]: batch into chunks, otherwise we will hit a crash
//     let ids: Vec<i64> = diesel::insert_into(versions)
//         .values(version_vec)
//         .on_conflict((package_id, semver))
//         .do_update()
//         .set((
//             tarball_url.eq(excluded(tarball_url)),
//             repository_raw.eq(excluded(repository_raw)),
//             repository_parsed.eq(excluded(repository_parsed)),
//             created.eq(excluded(created)),
//             deleted.eq(excluded(deleted)),
//             extra_metadata.eq(excluded(extra_metadata)),
//             prod_dependencies.eq(excluded(prod_dependencies)),
//             dev_dependencies.eq(excluded(dev_dependencies)),
//             peer_dependencies.eq(excluded(peer_dependencies)),
//             optional_dependencies.eq(excluded(optional_dependencies)),
//             secret.eq(excluded(secret)),
//         ))
//         .returning(id)
//         .get_results::<i64>(&conn.conn)
//         .expect("Error saving new version");

//     assert!(ids.len() == semvers.len());
//     ids.into_iter().zip(semvers.into_iter()).collect()
// }

// pub fn delete_versions_not_in(conn: &mut DbConnection, pkg_id: i64, vers: Vec<&Semver>) {
//     use super::schema::versions::dsl::*;

//     println!("The maybe slow query is running!");

//     // get all versions with the given package id
//     let all_vers = versions
//         .filter(package_id.eq(pkg_id))
//         .select((id, semver))
//         .load::<(i64, Semver)>(&conn.conn)
//         .expect("Error loading versions");

//     // TODO [perf]: Replace with hashset op
//     // Delete = all_vers - vers
//     for (ver_id, server_semver) in &all_vers {
//         if !vers.contains(&server_semver) {
//             diesel::update(versions.find(ver_id))
//                 .set(deleted.eq(true))
//                 .execute(&conn.conn)
//                 .expect("Error deleting version");
//         }
//     }
// }
