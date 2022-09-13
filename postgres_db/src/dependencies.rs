use crate::custom_types::PackageMetadata;
use crate::custom_types::ParsedSpec;
use crate::custom_types::RepoInfo;
use crate::custom_types::Semver;

use super::schema::dependencies;
use chrono::{DateTime, Utc};
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
}
