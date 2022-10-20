mod sql;
pub mod manager;

use std::collections::HashMap;

use crate::custom_types::Semver;


#[derive(Debug, PartialEq, Eq, Clone)]
pub struct InternalDiffLogPackageState {
    package_pack_hash: Option<String>,
    deleted: bool,
    versions: HashMap<Semver, InternalDiffLogVersionState>
}



#[derive(Debug, PartialEq, Eq, Clone)]
pub struct InternalDiffLogVersionState {
    version_pack_hash: String,
    deleted: bool
}

