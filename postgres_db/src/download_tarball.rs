use crate::download_queue::DownloadTask;

use super::schema::downloaded_tarballs;
use chrono::{DateTime, Utc};
use diesel::Queryable;

#[derive(Queryable, Insertable, Debug)]
#[diesel(table_name = downloaded_tarballs)]
pub struct DownloadedTarball {
    pub tarball_url: String,
    pub downloaded_at: DateTime<Utc>,

    pub shasum: Option<String>,
    pub unpacked_size: Option<i64>,
    pub file_count: Option<i32>,
    pub integrity: Option<String>,
    pub signature0_sig: Option<String>,
    pub signature0_keyid: Option<String>,
    pub npm_signature: Option<String>,

    pub tgz_local_path: String,
}

impl DownloadedTarball {
    /// Creates the downloaded tarball struct from the given download task and local path (full
    /// path to file). Sets the time of download to now.
    pub fn from_task(task: &DownloadTask, local_path: String) -> DownloadedTarball {
        DownloadedTarball {
            tarball_url: task.url.clone(),
            downloaded_at: Utc::now(),

            shasum: task.shasum.clone(),
            unpacked_size: task.unpacked_size,
            file_count: task.file_count,
            integrity: task.integrity.clone(),
            signature0_sig: task.signature0_sig.clone(),
            signature0_keyid: task.signature0_keyid.clone(),
            npm_signature: task.npm_signature.clone(),

            tgz_local_path: local_path,
        }
    }
}

#[derive(Debug)]
pub enum DownloadError {
    Request(reqwest::Error),
    StatusNotOk(reqwest::StatusCode),
    Io(std::io::Error),
}

impl std::error::Error for DownloadError {}

impl std::fmt::Display for DownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DownloadError::Request(e) => write!(f, "Request error: {}", e),
            DownloadError::StatusNotOk(e) => write!(f, "Status not OK: {}", e),
            DownloadError::Io(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl From<std::io::Error> for DownloadError {
    fn from(e: std::io::Error) -> Self {
        DownloadError::Io(e)
    }
}

impl From<reqwest::Error> for DownloadError {
    fn from(e: reqwest::Error) -> Self {
        DownloadError::Request(e)
    }
}
