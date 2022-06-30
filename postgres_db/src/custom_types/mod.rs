use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use sql_types::*;
use diesel::sql_types::Array;


#[derive(Debug, PartialEq, FromSqlRow, AsExpression, Clone, Eq, Hash, Serialize, Deserialize)]
#[sql_type = "SemverSql"]
pub struct Semver {
    pub major: i32,
    pub minor: i32,
    pub bug: i32,
    pub prerelease: Vec<PrereleaseTag>,
    pub build: Vec<String>
}

#[derive(Debug, PartialEq, FromSqlRow, AsExpression, Clone, Eq, Hash, Serialize, Deserialize)]
#[sql_type = "PrereleaseTagStructSql"]
pub enum PrereleaseTag {
    String(String),
    Int(i32)
}


#[derive(Debug, PartialEq, FromSqlRow, AsExpression, Serialize, Deserialize, Clone)]
#[sql_type = "ParsedSpecStructSql"]
pub enum ParsedSpec {
    Range(VersionConstraint),
    Tag(String),
    Git(String),
    Remote(String),
    Alias(String, Option<i64>, AliasSubspec),
    File(String),
    Directory(String)
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum AliasSubspec {
    Range(VersionConstraint),
    Tag(String)
}

#[derive(Debug, PartialEq, FromSqlRow, AsExpression, Clone, Serialize, Deserialize)]
#[sql_type = "VersionComparatorSql"]
pub enum VersionComparator {
    Any,
    Eq(Semver),
    Gt(Semver),
    Gte(Semver),
    Lt(Semver),
    Lte(Semver)
}

#[derive(Debug, PartialEq, FromSqlRow, AsExpression, Clone, Serialize, Deserialize)]
#[sql_type = "Array<ConstraintConjunctsSql>"]
pub struct VersionConstraint(pub Vec<Vec<VersionComparator>>);


#[derive(Debug, PartialEq, FromSqlRow, AsExpression, Clone)]
#[sql_type = "PackageMetadataStructSql"]
pub enum PackageMetadata {
    Normal { 
        dist_tag_latest_version: Option<i64>, 
        created: DateTime<Utc>, 
        modified: DateTime<Utc>, 
        other_dist_tags: HashMap<String, String> 
    },
    Unpublished {
        created: DateTime<Utc>, 
        modified: DateTime<Utc>,
        other_time_data: HashMap<String, DateTime<Utc>>,
        unpublished_data: serde_json::Value,
    },
    Deleted,
}

pub mod sql_types {
    #[derive(SqlType, QueryId)]
    #[postgres(type_name = "repository_struct")] // or should it be repository (domain)?
    pub struct RepositorySql;

    #[derive(SqlType, QueryId)]
    #[postgres(type_name = "semver_struct")] // or should it be semver (domain)?
    pub struct SemverSql;

    #[derive(SqlType)]
    #[postgres(type_name = "prerelease_tag_struct")] // or should it be prerelease_tag (domain)?
    pub struct PrereleaseTagStructSql;

    #[derive(SqlType, QueryId)]
    #[postgres(type_name = "version_comparator_struct")] // or should it be version_comparator (domain)?
    pub struct VersionComparatorSql;

    #[derive(SqlType)]
    #[postgres(type_name = "constraint_conjuncts_struct")] // or should it be constraint_conjuncts (domain)?
    pub struct ConstraintConjunctsSql;

    #[derive(SqlType)]
    #[postgres(type_name = "parsed_spec_struct")] // or should it be parsed_spec (domain)?
    pub struct ParsedSpecStructSql;

    #[derive(SqlType)]
    #[postgres(type_name = "package_metadata_struct")] // or should it be package_metadata (domain)?
    pub struct PackageMetadataStructSql;
}


#[allow(non_camel_case_types)]
pub mod sql_type_names {
    pub type Repository_struct = super::sql_types::RepositorySql;
    pub type Semver_struct = super::sql_types::SemverSql;
    pub type Version_comparator = super::sql_types::VersionComparatorSql;
    pub type Constraint_conjuncts_struct = super::sql_types::ConstraintConjunctsSql;
    pub type Parsed_spec_struct = super::sql_types::ParsedSpecStructSql;
    pub type Package_metadata_struct = super::sql_types::PackageMetadataStructSql;

}


mod semver;
mod version_comparator;
mod version_constraint;
mod parsed_spec;
mod package_metadata;
