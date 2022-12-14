use crate::connection::QueryRunner;
use crate::download_queue::DownloadTask;

use super::schema::downloaded_tarballs;
use chrono::{DateTime, Utc};
use diesel::Queryable;

use super::connection::DbConnection;
use super::schema;
use diesel::prelude::*;

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

    pub tgz_local_path: Option<String>,
    pub blob_storage_key: Option<String>,
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

            tgz_local_path: Some(local_path),
            blob_storage_key: None,
        }
    }
}

pub fn get_downloaded_urls_matching_tasks(
    conn: &mut DbConnection,
    chunk: &[DownloadTask],
) -> Vec<String> {
    use schema::downloaded_tarballs::dsl::*;

    let get_matching_tarball_urls_query = downloaded_tarballs
        .select(tarball_url)
        .filter(tarball_url.eq_any(chunk.iter().map(|t| &t.url)));
    conn.load(get_matching_tarball_urls_query)
        .expect("Error checking for max sequence in change_log table")
}
