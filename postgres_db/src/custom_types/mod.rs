use diesel::sql_types::Array;
use diesel::sql_types::Text;
use serde::{Deserialize, Serialize};
use sql_types::*;

#[derive(Debug, PartialEq, FromSqlRow, AsExpression, Clone, Eq, Hash, Serialize, Deserialize)]
#[diesel(sql_type = Text)]
pub enum DownloadFailed {
    Res(u16),
    Io,
    BadlyFormattedUrl,
    Other,
}

#[derive(
    Debug,
    PartialEq,
    FromSqlRow,
    AsExpression,
    Clone,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    PartialOrd,
    Ord,
)]
#[diesel(sql_type = crate::schema::sql_types::SemverStruct)]
pub struct Semver {
    pub major: i64,
    pub minor: i64,
    pub bug: i64,
    pub prerelease: Vec<PrereleaseTag>,
    pub build: Vec<String>,
}

impl std::fmt::Display for Semver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.bug)?;
        if !self.prerelease.is_empty() {
            write!(f, "-")?;
            for (i, tag) in self.prerelease.iter().enumerate() {
                if i != 0 {
                    write!(f, ".")?;
                }
                write!(f, "{}", tag)?;
            }
        }
        if !self.build.is_empty() {
            write!(f, "+")?;
            for (i, tag) in self.build.iter().enumerate() {
                if i != 0 {
                    write!(f, ".")?;
                }
                write!(f, "{}", tag)?;
            }
        }
        Ok(())
    }
}

#[derive(
    Debug,
    PartialEq,
    FromSqlRow,
    AsExpression,
    Clone,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    PartialOrd,
    Ord,
)]
#[diesel(sql_type = PrereleaseTagStructSql)]
pub enum PrereleaseTag {
    String(String),
    Int(i64),
}

impl std::fmt::Display for PrereleaseTag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrereleaseTag::String(s) => write!(f, "{}", s),
            PrereleaseTag::Int(i) => write!(f, "{}", i),
        }
    }
}

#[derive(Debug, Hash, FromSqlRow, AsExpression, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[diesel(sql_type = crate::schema::sql_types::ParsedSpecStruct)]
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
#[diesel(sql_type = VersionComparatorSql)]
pub enum VersionComparator {
    Any,
    Eq(Semver),
    Gt(Semver),
    Gte(Semver),
    Lt(Semver),
    Lte(Semver),
}

#[derive(Debug, FromSqlRow, AsExpression, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[diesel(sql_type = Array<ConstraintConjunctsSql>)]
pub struct VersionConstraint(pub Vec<Vec<VersionComparator>>);

// #[derive(Debug, PartialEq, Eq, FromSqlRow, AsExpression, Clone)]
// #[diesel(sql_type = crate::schema::sql_types::PackageMetadataStruct)]
// pub enum PackageMetadata {
//     Normal {
//         dist_tag_latest_version: Option<i64>,
//         created: DateTime<Utc>,
//         modified: DateTime<Utc>,
//         other_dist_tags: BTreeMap<String, String>,
//     },
//     Unpublished {
//         created: DateTime<Utc>,
//         modified: DateTime<Utc>,
//         other_time_data: BTreeMap<Semver, DateTime<Utc>>,
//         unpublished_data: serde_json::Value,
//     },
//     Deleted,
// }

#[derive(Debug, FromSqlRow, AsExpression, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[diesel(sql_type = crate::schema::sql_types::RepoInfoStruct)]
pub struct RepoInfo {
    pub cloneable_repo_url: String,
    pub cloneable_repo_dir: String,
    pub vcs: Vcs,
    pub host_info: RepoHostInfo,
}

impl RepoInfo {
    pub fn new_github(dir: String, user: String, repo: String) -> RepoInfo {
        RepoInfo {
            cloneable_repo_url: format!("https://github.com/{}/{}", user, repo),
            cloneable_repo_dir: dir,
            vcs: Vcs::Git,
            host_info: RepoHostInfo::Github { user, repo },
        }
    }
    pub fn new_bitbucket(dir: String, user: String, repo: String) -> RepoInfo {
        RepoInfo {
            cloneable_repo_url: format!("https://bitbucket.org/{}/{}", user, repo),
            cloneable_repo_dir: dir,
            vcs: Vcs::Git,
            host_info: RepoHostInfo::Bitbucket { user, repo },
        }
    }
    pub fn new_gitlab(dir: String, user: String, repo: String) -> RepoInfo {
        RepoInfo {
            cloneable_repo_url: format!("https://gitlab.com/{}/{}.git", user, repo),
            cloneable_repo_dir: dir,
            vcs: Vcs::Git,
            host_info: RepoHostInfo::Gitlab { user, repo },
        }
    }
    pub fn new_gist(id: String) -> RepoInfo {
        RepoInfo {
            cloneable_repo_url: format!("https://gist.github.com/{}", id),
            cloneable_repo_dir: "".to_string(),
            vcs: Vcs::Git,
            host_info: RepoHostInfo::Gist { id },
        }
    }
    pub fn new_thirdparty(url: String, dir: String) -> RepoInfo {
        RepoInfo {
            cloneable_repo_url: url,
            cloneable_repo_dir: dir,
            vcs: Vcs::Git,
            host_info: RepoHostInfo::Thirdparty,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Vcs {
    Git,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RepoHostInfo {
    Github { user: String, repo: String },
    Bitbucket { user: String, repo: String },
    Gitlab { user: String, repo: String },
    Gist { id: String },
    Thirdparty,
}

// TODO: make not pub
#[derive(Debug, PartialEq, FromSqlRow, AsExpression, Clone, Eq, Hash, Serialize, Deserialize)]
#[diesel(sql_type = crate::schema::sql_types::DiffType)]
pub enum DiffTypeEnum {
    CreatePackage,
    UpdatePackage,
    // SetPackageLatestTag,
    PatchPackageReferences,
    CreateVersion,
    UpdateVersion,
    DeleteVersion,
}

pub mod sql_types {
    // #[derive(SqlType, QueryId)]
    // #[diesel(postgres_type(name = "semver_struct"))] // or should it be semver (domain)?
    // pub struct SemverSql;

    #[derive(SqlType)]
    #[diesel(postgres_type(name = "prerelease_tag_struct"))] // or should it be prerelease_tag (domain)?
    pub struct PrereleaseTagStructSql;

    #[derive(SqlType, QueryId)]
    #[diesel(postgres_type(name = "version_comparator_struct"))] // or should it be version_comparator (domain)?
    pub struct VersionComparatorSql;

    #[derive(SqlType)]
    #[diesel(postgres_type(name = "constraint_conjuncts_struct"))] // or should it be constraint_conjuncts (domain)?
    pub struct ConstraintConjunctsSql;

    // #[derive(SqlType)]
    // #[diesel(postgres_type(name = "parsed_spec_struct"))] // or should it be parsed_spec (domain)?
    // pub struct ParsedSpecStructSql;

    // #[derive(SqlType)]
    // #[diesel(postgres_type(name = "package_metadata_struct"))] // or should it be package_metadata (domain)?
    // pub struct PackageMetadataStructSql;

    // #[derive(SqlType, QueryId)]
    // #[diesel(postgres_type(name = "repo_info_struct"))]
    // pub struct RepoInfoSql;

    // #[derive(SqlType)]
    // #[diesel(postgres_type(name = "diff_type"))]
    // pub struct DiffTypeEnumSql;

    // #[derive(SqlType)]
    // #[diesel(postgres_type(name = "internal_diff_log_version_state"))]
    // pub struct InternalDiffLogVersionStateSql;
}

// #[allow(non_camel_case_types)]
// pub mod sql_type_names {
//     pub type Semver_struct = super::sql_types::SemverSql;
//     pub type Version_comparator = super::sql_types::VersionComparatorSql;
//     pub type Constraint_conjuncts_struct = super::sql_types::ConstraintConjunctsSql;
//     pub type Parsed_spec_struct = super::sql_types::ParsedSpecStructSql;
//     pub type Package_metadata_struct = super::sql_types::PackageMetadataStructSql;
//     pub type Repo_info_struct = super::sql_types::RepoInfoSql;
//     pub type Diff_type = super::sql_types::DiffTypeEnumSql;
//     pub type Internal_diff_log_version_state = super::sql_types::InternalDiffLogVersionStateSql;
// }

mod diff_log;
mod download_failed;
mod helpers;
mod package_metadata;
pub use package_metadata::{
    PackageStateTimePoint, PackageStateType, VersionStateTimePoint, VersionStateType,
};
mod parsed_spec;
mod repo_info;
mod semver;
mod version_comparator;
mod version_constraint;
