pub mod manager;
pub mod sql;

use std::collections::BTreeMap;

use crate::custom_types::Semver;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct InternalDiffLogPackageState {
    pub package_pack_hash: Option<String>,
    pub deleted: bool,
    pub versions: BTreeMap<Semver, InternalDiffLogVersionState>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct InternalDiffLogVersionState {
    pub version_pack_hash: String,
    pub deleted: bool,
}
