use crate::download_tarball::DownloadedTarball;

use super::schema;
use super::schema::download_tasks;
use super::DbConnection;
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel::Queryable;

#[derive(Queryable, Insertable, Debug)]
#[diesel(table_name = download_tasks)]
pub struct DownloadTask {
    pub url: String,

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
    pub success: bool,
}

impl DownloadTask {
    pub fn fresh_task(
        url: String,
        shasum: Option<String>,
        unpacked_size: Option<i64>,
        file_count: Option<i32>,
        integrity: Option<String>,
        signature0_sig: Option<String>,
        signature0_keyid: Option<String>,
        npm_signature: Option<String>,
    ) -> DownloadTask {
        DownloadTask {
            url,

            shasum,
            unpacked_size,
            file_count,
            integrity,
            signature0_sig,
            signature0_keyid,
            npm_signature,

            queue_time: Utc::now(),
            num_failures: 0,
            last_failure: None,
            success: false,
        }
    }

    /// Downloads this task to the given directory. Inserts the task into the downloaded_tarballs table,
    /// and deletes it from the download_tasks table.
    pub async fn do_download(&self, conn: &DbConnection, dest: &str) -> std::io::Result<()> {
        // get the file and download it to dir
        let res = reqwest::get(&self.url).await.map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to download tarball, reqwest error: {}", e),
            )
        })?;
        let name = self.url.split('/').last().unwrap();
        let path = std::path::Path::new(dest).join(name);
        let mut file = std::fs::File::create(path.clone())?;
        let mut body = std::io::Cursor::new(res.bytes().await.map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to extract request body, reqwest error: {}", e),
            )
        })?);
        std::io::copy(&mut body, &mut file)?;

        // insert the task into the downloaded_tarballs table
        let task = DownloadedTarball::from_task(
            self,
            // makes the path absolute
            std::fs::canonicalize(path)?.to_str().unwrap().to_string(),
        );
        diesel::insert_into(schema::downloaded_tarballs::table)
            .values(&task)
            .execute(&conn.conn)
            .expect("Failed to insert downloaded tarball");

        // delete the task from the download_tasks table
        diesel::delete(
            schema::download_tasks::table.filter(schema::download_tasks::url.eq(&self.url)),
        )
        .execute(&conn.conn)
        .expect("Failed to delete download task");

        Ok(())
    }
}

const ENQUEUE_CHUNK_SIZE: usize = 2048;

pub fn enqueue_downloads(the_downloads: Vec<DownloadTask>, conn: &DbConnection) -> usize {
    let mut chunk_iter = the_downloads.chunks_exact(ENQUEUE_CHUNK_SIZE);
    let mut modify_count = 0;
    for chunk in &mut chunk_iter {
        modify_count += enqueue_chunk(conn, chunk);
    }

    modify_count += enqueue_chunk(conn, chunk_iter.remainder());

    modify_count
}

fn enqueue_chunk(conn: &DbConnection, chunk: &[DownloadTask]) -> usize {
    use schema::download_tasks::dsl::*;

    if chunk.len() > ENQUEUE_CHUNK_SIZE {
        panic!("Programming error: enqueue_chunk must be called with a chunk of size <= ENQUEUE_CHUNK_SIZE ({})", ENQUEUE_CHUNK_SIZE);
    }

    diesel::insert_into(download_tasks)
        .values(chunk)
        .on_conflict_do_nothing()
        .execute(&conn.conn)
        .expect("Failed to enqueue downloads into DB")
}

/// Downloads all present tasks to the given directory. Inserts each task completed in the
/// downloaded_tarballs table, and removes the completed tasks from the download_tasks table.
pub fn download_to_dest(conn: &DbConnection, dest: &str) -> std::io::Result<()> {
    use schema::download_tasks::dsl::*;

    let mut tasks: Vec<DownloadTask> = download_tasks
        .load(&conn.conn)
        .expect("Failed to load download tasks from DB");

    for task in tasks {
        // task.do_download(conn, dest)?;
        println!("{}", task.url);
    }

    Ok(())
}
