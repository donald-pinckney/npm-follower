use super::schema::versions;
use crate::connection::QueryRunner;
use crate::custom_types::RepoInfo;
use crate::custom_types::Semver;
use crate::custom_types::VersionStateTimePoint;
use crate::custom_types::VersionStateType;

use chrono::{DateTime, Utc};
use diesel::Queryable;
use serde_json::Value;

use diesel::prelude::*;
use diesel::Insertable;

#[derive(Queryable, Debug)]
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

#[derive(Insertable, Debug)]
#[diesel(table_name = versions)]
pub struct NewVersion {
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

pub fn get_version_by_id<R: QueryRunner>(conn: &mut R, version_id: i64) -> Version {
    let query = versions::table.filter(versions::id.eq(version_id));
    conn.get_result(query).expect("Error getting package")
}

pub fn get_versions_by_package_id<R: QueryRunner>(
    conn: &mut R,
    package_id: i64,
) -> Vec<Version> {
    let query = versions::table.filter(versions::package_id.eq(package_id));
    conn.get_results(query).expect("Error getting package")
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
    conn.get_result(query).expect("Error getting version")
}

pub fn get_version_times<R: QueryRunner>(
    conn: &mut R,
    package_id: i64,
) -> Vec<(Semver, DateTime<Utc>)> {
    let query = versions::table
        .filter(versions::package_id.eq(package_id))
        .select((versions::semver, versions::created));
    conn.get_results(query).expect("Error with postgres")
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

pub fn insert_new_version<R>(conn: &mut R, new_version: NewVersion) -> i64
where
    R: QueryRunner,
{
    use super::schema::versions::dsl::*;

    let insert_query = diesel::insert_into(versions)
        .values(new_version)
        .returning(id);

    conn.get_result(insert_query)
        .expect("Error saving new version")
}

pub fn set_version_extra_metadata<R>(conn: &mut R, version_id: i64, new_extra_metadata: Value)
where
    R: QueryRunner,
{
    use super::schema::versions::dsl::*;

    let update_query =
        diesel::update(versions.find(version_id)).set(extra_metadata.eq(new_extra_metadata));

    assert_eq!(
        conn.execute(update_query)
            .expect("Error updating version extra metadata"),
        1
    );
}

pub fn delete_version<R>(
    conn: &mut R,
    version_id: i64,
    seq: i64,
    diff_entry_id: i64,
    delete_time: Option<DateTime<Utc>>,
) where
    R: QueryRunner,
{
    use super::schema::versions::dsl::*;

    let mut current_data = get_version_by_id(conn, version_id);
    current_data
        .version_state_history
        .push(VersionStateTimePoint {
            seq,
            diff_entry_id,
            state: VersionStateType::Deleted,
            estimated_time: delete_time,
        });

    let update_query = diesel::update(versions.find(version_id)).set((
        current_version_state_type.eq(VersionStateType::Deleted),
        version_state_history.eq(current_data.version_state_history),
    ));

    conn.execute(update_query).expect("Error deleting version");
}

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
