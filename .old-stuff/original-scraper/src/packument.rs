use chrono::Utc;
use chrono::DateTime;
use crate::Version;
use serde_json::Value;
use std::collections::HashMap;
use std::collections::HashSet;

#[derive(PartialEq, Eq, Hash, Debug)]
pub enum PackageReference<'pkgs> {
    Known(&'pkgs String),
    Unknown(String)
}

impl PackageReference<'_> {
    pub fn lookup(all_packages: &HashSet<String>, p: String) -> PackageReference {
        match all_packages.get(&p) {
            Some(pkg) => PackageReference::Known(pkg),
            None => PackageReference::Unknown(p)
        }
    }
}

#[derive(Debug)]
pub struct Dependencies<'pkgs> {
    pub prod_dependencies: HashMap<PackageReference<'pkgs>, (u64, Option<String>)>,
    pub dev_dependencies: HashMap<PackageReference<'pkgs>, (u64, Option<String>)>,
    pub peer_dependencies: HashMap<PackageReference<'pkgs>, (u64, Option<String>)>,
    pub optional_dependencies: HashMap<PackageReference<'pkgs>, (u64, Option<String>)>
}

#[derive(Debug)]
pub struct VersionPackument<'pkgs> {
    pub description: Option<String>,
    pub shasum: String,
    pub tarball: String,
    pub dependencies: Dependencies<'pkgs>,
    pub extra_metadata: HashMap<String, Value>
}

#[derive(Debug)]
pub struct Packument<'pkgs> {
    pub latest: Option<Version>,
    pub created: DateTime<Utc>,
    pub modified: DateTime<Utc>,
    pub version_times: HashMap<Version, DateTime<Utc>>,
    pub versions: HashMap<Version, VersionPackument<'pkgs>>,
    pub other_dist_tags: Option<HashMap<String, Version>>
}
