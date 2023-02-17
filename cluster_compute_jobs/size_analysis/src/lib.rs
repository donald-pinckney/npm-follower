use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SizeAnalysisTarball {
    pub tarball_url: String,
    // these are in bytes
    pub total_files: u64,
    pub total_size: u64,
    pub total_size_code: u64,
}
