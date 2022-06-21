use chrono::Utc;
use chrono::DateTime;
use postgres_db::custom_types::{Semver, VersionComparator};
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
    pub description: Option<String>,
    pub shasum: String,
    pub tarball: String,
    pub dependencies: PackumentDependencies,
    pub extra_metadata: HashMap<String, Value>
}

#[derive(Debug)]
pub struct PackumentDependencies {
    pub prod_dependencies: Vec<(String, VersionComparator)>,
    pub dev_dependencies: Vec<(String, VersionComparator)>,
    pub peer_dependencies: Vec<(String, VersionComparator)>,
    pub optional_dependencies: Vec<(String, VersionComparator)>
}

pub mod parsing;
