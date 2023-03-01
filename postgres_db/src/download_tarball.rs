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
    fn from_task_help(
        task: &DownloadTask,
        tgz_local_path: Option<String>,
        blob_storage_key: Option<String>,
    ) -> Self {
        Self {
            tarball_url: task.url.clone(),
            downloaded_at: Utc::now(),
            shasum: task.shasum.clone(),
            unpacked_size: task.unpacked_size,
            file_count: task.file_count,
            integrity: task.integrity.clone(),
            signature0_sig: task.signature0_sig.clone(),
            signature0_keyid: task.signature0_keyid.clone(),
            npm_signature: task.npm_signature.clone(),
            tgz_local_path,
            blob_storage_key,
        }
    }
    /// Creates the downloaded tarball struct from the given download task and local path (full
    /// path to file). Sets the time of download to now.
    pub fn from_task(task: &DownloadTask, local_path: String) -> DownloadedTarball {
        Self::from_task_help(task, Some(local_path), None)
    }

    /// Creates the downloaded tarball struct from the given download task and blob storage key.
    /// Sets the time of download to now.
    pub fn from_task_blob(task: &DownloadTask, blob_key: String) -> DownloadedTarball {
        Self::from_task_help(task, None, Some(blob_key))
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

pub fn query_tarballs_after_url(
    conn: &mut DbConnection,
    after_url: &str,
    limit: i64,
) -> Vec<DownloadedTarball> {
    // returns ordered by url, ascending
    use schema::downloaded_tarballs::dsl::*;

    let query = downloaded_tarballs
        .filter(tarball_url.gt(after_url))
        .order(tarball_url.asc())
        .limit(limit);
    conn.load(query).expect("Error querying tarballs after url")
}

pub fn query_first_tarball_by_url(conn: &mut DbConnection) -> Option<DownloadedTarball> {
    // ordered by tarball_url, ascending
    use schema::downloaded_tarballs::dsl::*;

    let query = downloaded_tarballs.order(tarball_url.asc()).limit(1);
    conn.load(query)
        .expect("Error querying first tarball by url")
        .pop()
}

pub fn get_tarball_by_url(conn: &mut DbConnection, url: &str) -> Option<DownloadedTarball> {
    use schema::downloaded_tarballs::dsl::*;

    let query = downloaded_tarballs.filter(tarball_url.eq(url));
    conn.load(query)
        .expect("Error querying tarball by url")
        .pop()
}

pub fn num_total_downloaded_tarballs(conn: &mut DbConnection) -> i64 {
    use schema::downloaded_tarballs::dsl::*;

    let query = downloaded_tarballs.select(diesel::dsl::count(tarball_url));
    conn.load(query)
        .expect("Error querying number of downloaded tarballs")
        .pop()
        .unwrap()
}

pub fn set_blob_storage_key(conn: &mut DbConnection, tb_url: &str, blob_key: &str) {
    use schema::downloaded_tarballs::dsl::*;

    let query = diesel::update(downloaded_tarballs.filter(tarball_url.eq(tb_url)))
        .set(blob_storage_key.eq(blob_key));
    conn.execute(query).expect("Error setting blob storage key");
}
