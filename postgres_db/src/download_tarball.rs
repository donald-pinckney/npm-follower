use super::schema;
use super::schema::downloaded_tarballs;
use super::DbConnection;
use chrono::{DateTime, Utc};
use diesel::prelude::*;
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

impl DownloadedTarball {}
