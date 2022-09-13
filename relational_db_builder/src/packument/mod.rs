use chrono::DateTime;
use chrono::Utc;
use postgres_db::custom_types::PackageMetadata;
use postgres_db::custom_types::{ParsedSpec, RepoInfo, Semver};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Packument {
    Normal {
        latest: Option<Semver>,
        created: DateTime<Utc>,
        modified: DateTime<Utc>,
        other_dist_tags: Map<String, Value>,
        version_times: HashMap<Semver, DateTime<Utc>>,
        versions: HashMap<Semver, VersionPackument>,
    },
    Unpublished {
        created: DateTime<Utc>,
        modified: DateTime<Utc>,
        unpublished_blob: Value,
        extra_version_times: HashMap<Semver, DateTime<Utc>>,
    },
    // Marked as *not* deleted, but does not have any data in the change.
    // Possibly has data if you hit registry.npmjs.org.
    MissingData,
    Deleted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VersionPackument {
    pub prod_dependencies: Vec<(String, Spec)>,
    pub dev_dependencies: Vec<(String, Spec)>,
    pub peer_dependencies: Vec<(String, Spec)>,
    pub optional_dependencies: Vec<(String, Spec)>,
    pub dist: Dist,
    pub repository: Option<RepositoryInfo>,
    pub extra_metadata: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Dist {
    pub tarball_url: String,
    pub shasum: Option<String>,
    pub unpacked_size: Option<i64>,
    pub file_count: Option<i32>,
    pub integrity: Option<String>,
    pub signature0_sig: Option<String>,
    pub signature0_keyid: Option<String>,
    pub npm_signature: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Spec {
    pub raw: Value,
    pub parsed: ParsedSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepositoryInfo {
    pub raw: Value,
    pub info: RepoInfo,
}

impl From<Packument> for PackageMetadata {
    /// Convert a packument into a package metadata.
    /// NOTE: this sets the dist_tag_latest_version to None.
    fn from(pack: Packument) -> Self {
        match pack {
            Packument::Normal {
                latest,
                created,
                modified,
                other_dist_tags,
                version_times,
                versions,
            } => PackageMetadata::Normal {
                dist_tag_latest_version: None,
                created,
                modified,
                other_dist_tags: other_dist_tags
                    .into_iter()
                    .map(|(k, v)| (k, v.to_string()))
                    .collect(),
            },
            Packument::Unpublished {
                created,
                modified,
                unpublished_blob,
                extra_version_times,
            } => PackageMetadata::Unpublished {
                created,
                modified,
                other_time_data: extra_version_times,
                unpublished_data: unpublished_blob,
            }, // TODO: i think MissingData should go into Deleted right?
            Packument::MissingData | Packument::Deleted => PackageMetadata::Deleted,
        }
    }
}

pub mod deserialize;
mod deserialize_repo;
