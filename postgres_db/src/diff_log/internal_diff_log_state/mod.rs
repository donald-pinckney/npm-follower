pub mod manager;
mod sql;

use std::collections::BTreeMap;

use crate::custom_types::Semver;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct InternalDiffLogPackageState {
    package_pack_hash: Option<String>,
    deleted: bool,
    versions: BTreeMap<Semver, InternalDiffLogVersionState>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct InternalDiffLogVersionState {
    version_pack_hash: String,
    deleted: bool,
}
