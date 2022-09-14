use crate::custom_types::PackageMetadata;
use crate::custom_types::RepoInfo;
use crate::custom_types::Semver;

use super::schema::versions;
use chrono::{DateTime, Utc};
use diesel::sql_types::BigInt;
use diesel::Queryable;
use serde_json::Value;

use super::schema;
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

pub fn insert_version(conn: &DbConnection, version: Version) -> i64 {
    use super::schema::versions::dsl::*;

    diesel::insert_into(versions)
        .values(&version)
        .get_result::<(
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
        .expect("Error saving new version")
        .0
}
