use chrono::Utc;
use chrono::DateTime;
use postgres_db::custom_types::{Semver, ParsedSpec};
use serde_json::{Value, Map};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;


#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
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
        extra_version_times: HashMap<Semver, DateTime<Utc>>
    },
    // Marked as *not* deleted, but does not have any data in the change. 
    // Possibly has data if you hit registry.npmjs.org.
    MissingData, 
    Deleted
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct VersionPackument {
    pub prod_dependencies: Vec<(String, Spec)>,
    pub dev_dependencies: Vec<(String, Spec)>,
    pub peer_dependencies: Vec<(String, Spec)>,
    pub optional_dependencies: Vec<(String, Spec)>,
    pub dist: Dist,
    pub repository: Option<RepositoryInfo>,
    pub extra_metadata: HashMap<String, Value>
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
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

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Spec {
    pub raw: Value,
    pub parsed: ParsedSpec
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepositoryInfo {
    pub raw: Value,
    pub cloneable_repo_url: String,
    pub repo_dir: String
}

pub mod deserialize;
