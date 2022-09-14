use std::collections::HashMap;

use chrono::{DateTime, Utc};
use diesel::sql_types::Array;
use diesel::sql_types::Text;
use serde::{Deserialize, Serialize};
use sql_types::*;

#[derive(Debug, PartialEq, FromSqlRow, AsExpression, Clone, Eq, Hash, Serialize, Deserialize)]
#[sql_type = "Text"]
pub enum DownloadFailed {
    Res(u16),
    Io,
    BadlyFormattedUrl,
    Other,
}

#[derive(Debug, PartialEq, FromSqlRow, AsExpression, Clone, Eq, Hash, Serialize, Deserialize)]
#[sql_type = "SemverSql"]
pub struct Semver {
    pub major: i64,
    pub minor: i64,
    pub bug: i64,
    pub prerelease: Vec<PrereleaseTag>,
    pub build: Vec<String>,
}

#[derive(Debug, PartialEq, FromSqlRow, AsExpression, Clone, Eq, Hash, Serialize, Deserialize)]
#[sql_type = "PrereleaseTagStructSql"]
pub enum PrereleaseTag {
    String(String),
    Int(i64),
}

#[derive(Debug, Hash, FromSqlRow, AsExpression, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[sql_type = "ParsedSpecStructSql"]
pub enum ParsedSpec {
    Range(VersionConstraint),
    Tag(String),
    Git(String),
    Remote(String),
    Alias(String, Option<i64>, AliasSubspec),
    File(String),
    Directory(String),
    Invalid(String),
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub enum AliasSubspec {
    Range(VersionConstraint),
    Tag(String),
}

#[derive(Debug, FromSqlRow, AsExpression, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[sql_type = "VersionComparatorSql"]
pub enum VersionComparator {
    Any,
    Eq(Semver),
    Gt(Semver),
    Gte(Semver),
    Lt(Semver),
    Lte(Semver),
}

#[derive(Debug, FromSqlRow, AsExpression, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[sql_type = "Array<ConstraintConjunctsSql>"]
pub struct VersionConstraint(pub Vec<Vec<VersionComparator>>);

#[derive(Debug, PartialEq, FromSqlRow, AsExpression, Clone)]
#[sql_type = "PackageMetadataStructSql"]
pub enum PackageMetadata {
    Normal {
        dist_tag_latest_version: Option<i64>,
        created: DateTime<Utc>,
        modified: DateTime<Utc>,
        other_dist_tags: HashMap<String, String>,
    },
    Unpublished {
        created: DateTime<Utc>,
        modified: DateTime<Utc>,
        other_time_data: HashMap<Semver, DateTime<Utc>>,
        unpublished_data: serde_json::Value,
    },
    Deleted,
}


#[derive(Debug, FromSqlRow, AsExpression, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[sql_type = "RepoInfoSql"]
pub struct RepoInfo {
    pub cloneable_repo_url: String,
    pub cloneable_repo_dir: String,
    pub vcs: Vcs,
    pub host_info: RepoHostInfo
}

impl RepoInfo {
    pub fn new_github(dir: String, user: String, repo: String) -> RepoInfo {
        RepoInfo { 
            cloneable_repo_url: format!("https://github.com/{}/{}", user, repo),
            cloneable_repo_dir: dir,
            vcs: Vcs::Git,
            host_info: RepoHostInfo::Github { user, repo }
        }
    }
    pub fn new_bitbucket(dir: String, user: String, repo: String) -> RepoInfo {
        RepoInfo { 
            cloneable_repo_url: format!("https://bitbucket.org/{}/{}", user, repo),
            cloneable_repo_dir: dir,
            vcs: Vcs::Git,
            host_info: RepoHostInfo::Bitbucket { user, repo }
        }
    }
    pub fn new_gitlab(dir: String, user: String, repo: String) -> RepoInfo {
        RepoInfo { 
            cloneable_repo_url: format!("https://gitlab.com/{}/{}.git", user, repo),
            cloneable_repo_dir: dir,
            vcs: Vcs::Git,
            host_info: RepoHostInfo::Gitlab { user, repo }
        }
    }
    pub fn new_gist(id: String) -> RepoInfo {
        RepoInfo { 
            cloneable_repo_url: format!("https://gist.github.com/{}", id),
            cloneable_repo_dir: "/".to_string(),
            vcs: Vcs::Git,
            host_info: RepoHostInfo::Gist { id }
        }
    }
    pub fn new_thirdparty(url: String, dir: String) -> RepoInfo {
        RepoInfo { 
            cloneable_repo_url: url,
            cloneable_repo_dir: dir,
            vcs: Vcs::Git,
            host_info: RepoHostInfo::Thirdparty
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Vcs {
    Git
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RepoHostInfo {
    Github { user: String, repo: String },
    Bitbucket { user: String, repo: String },
    Gitlab { user: String, repo: String },
    Gist { id: String },
    Thirdparty
}





pub mod sql_types {
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

    #[derive(SqlType, QueryId)]
    #[postgres(type_name = "repo_info_struct")]
    pub struct RepoInfoSql;
}

#[allow(non_camel_case_types)]
pub mod sql_type_names {
    pub type Semver_struct = super::sql_types::SemverSql;
    pub type Version_comparator = super::sql_types::VersionComparatorSql;
    pub type Constraint_conjuncts_struct = super::sql_types::ConstraintConjunctsSql;
    pub type Parsed_spec_struct = super::sql_types::ParsedSpecStructSql;
    pub type Package_metadata_struct = super::sql_types::PackageMetadataStructSql;
    pub type Repo_info_struct = super::sql_types::RepoInfoSql;
}

mod download_failed;
mod package_metadata;
mod parsed_spec;
mod semver;
mod version_comparator;
mod version_constraint;
mod repo_info;
