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
    pub repository_raw: Value,
    pub repository_parsed: RepoInfo,
    pub created: DateTime<Utc>,
    pub deleted: bool,
    pub extra_metadata: Value,
    pub prod_dependencies: Vec<i64>, // this is a list of version ids
    pub dev_dependencies: Vec<i64>,  // this is a list of version ids
    pub peer_dependencies: Vec<i64>, // this is a list of version ids
    pub optional_dependencies: Vec<i64>, // this is a list of version ids
    pub secret: bool,
}
