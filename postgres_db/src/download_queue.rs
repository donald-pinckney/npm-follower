use diesel::prelude::*;
use super::DbConnection;
use super::schema::download_tasks;
use super::schema;
use diesel::Queryable;
use chrono::{DateTime, Utc};

#[derive(Queryable, Insertable, Debug)]
#[diesel(table_name = download_tasks)]
pub struct DownloadTask {
    pub package: String,
    pub version: String,

    pub url: String,
    pub change_seq: i64,

    pub shasum: Option<String>,
    pub unpacked_size: Option<i64>,
    pub file_count: Option<i32>,
    pub integrity: Option<String>,
    pub signature0_sig: Option<String>,
    pub signature0_keyid: Option<String>,
    pub npm_signature: Option<String>,
    pub queue_time: DateTime<Utc>,
    pub num_failures: i32,
    pub last_failure: Option<DateTime<Utc>>,
    pub success: bool
}


pub struct NewDownloadTask {
    pub url: String,
    pub change_seq: i64,
    pub package: String,
    pub version: String,
    pub shasum: Option<String>,
    pub unpacked_size: Option<i64>,
    pub file_count: Option<i32>,
    pub integrity: Option<String>,
    pub signature0_sig: Option<String>,
    pub signature0_keyid: Option<String>,
    pub npm_signature: Option<String>,
}

impl NewDownloadTask {
    pub fn prepare_enqueue(self) -> DownloadTask {
        DownloadTask {
            package: self.package,
            version: self.version,

            url: self.url,
            change_seq: self.change_seq,
            
            shasum: self.shasum,
            unpacked_size: self.unpacked_size,
            file_count: self.file_count,
            integrity: self.integrity,
            signature0_sig: self.signature0_sig,
            signature0_keyid: self.signature0_keyid,
            npm_signature: self.npm_signature,

            queue_time: Utc::now(),
            num_failures: 0,
            last_failure: None,
            success: false,
        }
    }
}

pub fn enqueue_downloads(the_downloads: Vec<DownloadTask>, conn: &DbConnection) -> usize {
    use schema::download_tasks::dsl::*;
    const CHUNK_SIZE: usize = 2048;

    let mut chunk_iter = the_downloads.chunks_exact(CHUNK_SIZE);
    let mut modify_count = 0;
    for chunk in &mut chunk_iter {
        modify_count += diesel::insert_into(download_tasks)
            .values(chunk)
            .on_conflict_do_nothing()
            .execute(&conn.conn)
            .expect("Failed to enqueue downloads into DB");
    }

    modify_count += diesel::insert_into(download_tasks)
        .values(chunk_iter.remainder())
        .on_conflict_do_nothing()
        .execute(&conn.conn)
        .expect("Failed to enqueue downloads into DB");
    
    modify_count
}

