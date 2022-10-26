use crate::custom_types::{ParsedSpec, RepoInfo, Semver};
use chrono::DateTime;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PackageOnlyPackument {
    Normal {
        latest: Option<Semver>,
        created: DateTime<Utc>,
        modified: DateTime<Utc>,
        other_dist_tags: Map<String, Value>,
    },
    Unpublished {
        created: DateTime<Utc>,
        modified: DateTime<Utc>,
        unpublished_blob: Value,
        #[serde(with = "crate::serde_non_string_key_serialization")]
        extra_version_times: BTreeMap<Semver, DateTime<Utc>>,
    },
    // Marked as *not* deleted, but does not have any data in the change.
    // Possibly has data if you hit registry.npmjs.org.
    MissingData,
    Deleted,
}

impl PackageOnlyPackument {
    pub fn serialize_and_hash(&self) -> (Value, String, usize) {
        let v = serde_json::to_value(self).unwrap();
        let s = serde_json::to_vec(&v).unwrap();
        let n_bytes = s.len();
        let mut hasher = Sha256::new();
        hasher.update(s);
        let result = hasher.finalize();

        (v, format!("{:x}", result), n_bytes)
    }

    /// Returns `true` if the package only packument is [`Normal`].
    ///
    /// [`Normal`]: PackageOnlyPackument::Normal
    #[must_use]
    pub fn is_normal(&self) -> bool {
        matches!(self, Self::Normal { .. })
    }
}

pub type AllVersionPackuments = BTreeMap<Semver, VersionOnlyPackument>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VersionOnlyPackument {
    pub prod_dependencies: Vec<(String, Spec)>,
    pub dev_dependencies: Vec<(String, Spec)>,
    pub peer_dependencies: Vec<(String, Spec)>,
    pub optional_dependencies: Vec<(String, Spec)>,
    pub dist: Dist,
    pub repository: Option<RepositoryInfo>,
    pub time: DateTime<Utc>,
    pub extra_metadata: BTreeMap<String, Value>,
}

impl VersionOnlyPackument {
    pub fn serialize_and_hash(&self) -> (Value, String, usize) {
        let v = serde_json::to_value(self).unwrap();
        let s = serde_json::to_vec(&v).unwrap();
        let n_bytes = s.len();
        let mut hasher = Sha256::new();
        hasher.update(s);
        let result = hasher.finalize();

        (v, format!("{:x}", result), n_bytes)
    }
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

// impl From<PackageOnlyPackument> for PackageMetadata {
//     /// Convert a packument into a package metadata.
//     /// NOTE: this sets the dist_tag_latest_version to None.
//     fn from(pack: PackageOnlyPackument) -> Self {
//         match pack {
//             PackageOnlyPackument::Normal {
//                 latest: _,
//                 created,
//                 modified,
//                 other_dist_tags,
//             } => PackageMetadata::Normal {
//                 dist_tag_latest_version: None,
//                 created,
//                 modified,
//                 // TODO[bug]: why are we doing .to_string here???
//                 other_dist_tags: other_dist_tags
//                     .into_iter()
//                     .map(|(k, v)| (k, v.to_string()))
//                     .collect(),
//             },
//             PackageOnlyPackument::Unpublished {
//                 created,
//                 modified,
//                 unpublished_blob,
//                 extra_version_times,
//             } => PackageMetadata::Unpublished {
//                 created,
//                 modified,
//                 other_time_data: extra_version_times,
//                 unpublished_data: unpublished_blob,
//             }, // TODO: i think MissingData should go into Deleted right?
//             PackageOnlyPackument::MissingData | PackageOnlyPackument::Deleted => {
//                 PackageMetadata::Deleted
//             }
//         }
//     }
// }
