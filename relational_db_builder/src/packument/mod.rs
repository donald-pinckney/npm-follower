use chrono::Utc;
use chrono::DateTime;
use postgres_db::custom_types::{Semver, VersionConstraint, Repository};
use serde_json::Value;
use std::collections::HashMap;


#[derive(Debug)]
pub struct Packument {
    pub latest: Option<Semver>,
    pub created: DateTime<Utc>,
    pub modified: DateTime<Utc>,
    pub version_times: HashMap<Semver, DateTime<Utc>>,
    pub versions: HashMap<Semver, VersionPackument>,
    pub other_dist_tags: Option<HashMap<String, Semver>>
}

#[derive(Debug)]
pub struct VersionPackument {
    pub prod_dependencies: Vec<(String, VersionConstraint)>,
    pub dev_dependencies: Vec<(String, VersionConstraint)>,
    pub peer_dependencies: Vec<(String, VersionConstraint)>,
    pub optional_dependencies: Vec<(String, VersionConstraint)>,
    pub dist: Dist,
    pub description: Option<String>,
    pub repository: Option<Repository>,
    pub extra_metadata: HashMap<String, Value>
}

#[derive(Debug)]
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

pub mod parsing;
