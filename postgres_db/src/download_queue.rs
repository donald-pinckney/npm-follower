use crate::custom_types::DownloadFailed;
use crate::download_tarball;
use crate::download_tarball::DownloadedTarball;
use std::collections::HashSet;

use super::connection::DbConnection;
use super::schema;
use super::schema::download_tasks;
use chrono::{DateTime, Utc};
use diesel::pg::upsert::excluded;
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
    pub failed: Option<DownloadFailed>,
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
            failed: None,
        }
    }

    // gets the depth of an url, subtracting 3 for the '//' in 'https://' and the domain space
    pub fn url_depth(url: &str) -> usize {
        match url.split('/').count() as i16 - 3 {
            x if x < 0 => 0,
            x => x as usize,
        }
    }

    /// Produces the name of the resulting file from the given url. Where if it is a tarball that comes
    /// from a namespace, it names is appropriately.
    pub fn get_filename(url: &str) -> Result<String, DownloadFailed> {
        let slashsplit = url.split('/').collect::<Vec<&str>>();
        let base_name = slashsplit.last().ok_or(DownloadFailed::BadlyFormattedUrl)?;
        // 3 = normal package
        // 4 = namespace
        match Self::url_depth(url) {
            3 => Ok(base_name.to_string()),
            4 => {
                let namespace = slashsplit.get(3).ok_or(DownloadFailed::BadlyFormattedUrl)?;
                Ok(format!("{}-{}", namespace, base_name))
            }
            _ => Err(DownloadFailed::BadlyFormattedUrl),
        }
    }
}

const ENQUEUE_CHUNK_SIZE: usize = 2048;

pub fn enqueue_downloads(the_downloads: Vec<DownloadTask>, conn: &mut DbConnection) -> usize {
    let mut chunk_iter = the_downloads.chunks_exact(ENQUEUE_CHUNK_SIZE);
    let mut modify_count = 0;
    for chunk in &mut chunk_iter {
        modify_count += enqueue_chunk(conn, chunk);
    }

    modify_count += enqueue_chunk(conn, chunk_iter.remainder());

    modify_count
}

fn enqueue_chunk(conn: &mut DbConnection, chunk: &[DownloadTask]) -> usize {
    use schema::download_tasks::dsl::*;

    if chunk.len() > ENQUEUE_CHUNK_SIZE {
        panic!("Programming error: enqueue_chunk must be called with a chunk of size <= ENQUEUE_CHUNK_SIZE ({})", ENQUEUE_CHUNK_SIZE);
    }

    // A1. Filter out URL x if it exists in downloaded_tarballs
    let already_downloaded_urls: HashSet<_> =
        download_tarball::get_downloaded_urls_matching_tasks(conn, chunk)
            .into_iter()
            .collect();
    let chunk: Vec<_> = chunk
        .iter()
        .filter(|t| !already_downloaded_urls.contains(&t.url))
        .collect();

    // A2. If URL x didn't exist in downloaded_tarballs, enqueue into download_tasks
    let insert_query = diesel::insert_into(download_tasks)
        .values(chunk)
        .on_conflict_do_nothing();
    conn.execute(insert_query)
        .expect("Failed to enqueue downloads into DB")

    // Note: we could do a transaction here, but instead if we consider
    // the overleavings with the downloader, it is safe,
    // so long as the downloader can handle duplicate downloads.
    // Possible interleavings combined with:

    // B1. Downloader selects URL x from download_tasks
    // B2. Downloader inserts URL x into downloaded_tarballs
    // B3. Downloader removes URL x from download_tasks

    /*

    A1
    A2
    B1
    B2
    B3
    ---> normal case, ok


    A1
    B1
    A2 ---> this will be a nop, because the task is already in the table (not yet deleted by B3).
    B2
    B3
    ---> ok


    B1
    A1
    A2 ---> this will be a nop, because the task is already in the table (not yet deleted by B3).
    B2
    B3
    ---> ok


    A1
    B1
    B2
    B3
    A2 ---> ***this will re-insert URL x back into download_tasks***
    ---> NEED TO MAKE SURE DOWNLOADER HANDLES RE-DOWNLOADING OK


    B1
    A1
    B2
    B3
    A2 ---> ***this will re-insert URL x back into download_tasks***
    ---> NEED TO MAKE SURE DOWNLOADER HANDLES RE-DOWNLOADING OK


    B1
    B2
    B3
    A1 ---> will always filter, ok
    A2
    ---> ok


    A1
    B1
    B2
    A2 ---> this insert will be a nop, because URL x hasn't yet been deleted by B3
    B3
    ---> ok


    B1
    A1
    B2
    A2 ---> this insert will be a nop, because URL x hasn't yet been deleted by B3
    B3
    ---> ok


    B1
    B2
    A1 ---> will always filter, ok
    A2
    B3
    ---> ok


    B1
    B2
    A1 ---> will always filter, ok
    B3
    A2
    ---> ok

    */
}

pub const TASKS_CHUNK_SIZE: i64 = 2048;

pub fn get_total_tasks_num(conn: &mut DbConnection, retry_failed: bool) -> i64 {
    use schema::download_tasks::dsl::*;

    if retry_failed {
        conn.get_result(download_tasks.count())
            .expect("Failed to get number of tasks")
    } else {
        conn.get_result(download_tasks.filter(failed.is_null()).count())
            .expect("Failed to get number of tasks")
    }
}

pub fn load_chunk_init(conn: &mut DbConnection, retry_failed: bool) -> Vec<DownloadTask> {
    use schema::download_tasks::dsl::*;
    if retry_failed {
        conn.load(
            download_tasks
                .order(url.asc()) // order by the time they got queued, in ascending order
                .limit(TASKS_CHUNK_SIZE),
        )
        .expect("Failed to load download tasks from DB")
    } else {
        conn.load(
            download_tasks
                .order(url.asc()) // order by the time they got queued, in ascending order
                .filter(failed.is_null())
                .limit(TASKS_CHUNK_SIZE),
        )
        .expect("Failed to load download tasks from DB")
    }
}

pub fn load_chunk_next(
    conn: &mut DbConnection,
    last_url: &String,
    retry_failed: bool,
) -> Vec<DownloadTask> {
    use schema::download_tasks::dsl::*;
    if retry_failed {
        conn.load(
            download_tasks
                .order(url.asc()) // order by the time they got queued, in ascending order
                .filter(url.gt(last_url))
                .limit(TASKS_CHUNK_SIZE),
        )
        .expect("Failed to load download tasks from DB")
    } else {
        conn.load(
            download_tasks
                .order(url.asc()) // order by the time they got queued, in ascending order
                .filter(failed.is_null().and(url.gt(last_url)))
                .limit(TASKS_CHUNK_SIZE),
        )
        .expect("Failed to load download tasks from DB")
    }
}

pub fn update_from_tarballs(conn: &mut DbConnection, tarballs: &Vec<DownloadedTarball>) {
    println!("Inserting {} tarballs", tarballs.len());

    // insert all the tarballs from download_queue in the db
    {
        use schema::downloaded_tarballs::dsl::*; // have to scope the imports as they conflict.
        conn.execute(
            diesel::insert_into(schema::downloaded_tarballs::table)
                .values(tarballs)
                .on_conflict(tarball_url)
                .do_update()
                .set((
                    tarball_url.eq(excluded(tarball_url)),
                    downloaded_at.eq(excluded(downloaded_at)),
                    shasum.eq(excluded(shasum)),
                    unpacked_size.eq(excluded(unpacked_size)),
                    file_count.eq(excluded(file_count)),
                    integrity.eq(excluded(integrity)),
                    signature0_sig.eq(excluded(signature0_sig)),
                    signature0_keyid.eq(excluded(signature0_keyid)),
                    npm_signature.eq(excluded(npm_signature)),
                    tgz_local_path.eq(excluded(tgz_local_path)),
                )),
        )
        .expect("Failed to insert downloaded tarballs into DB");
    }

    // delete the tasks from download_tasks that are contained in download_queue
    {
        use schema::download_tasks::dsl::*;
        conn.execute(
            diesel::delete(download_tasks)
                .filter(url.eq_any(tarballs.iter().map(|x| x.tarball_url.clone()))),
        )
        .expect("Failed to delete downloaded tasks from DB");
    }
}

pub fn update_from_error(conn: &mut DbConnection, task: &DownloadTask, error: DownloadFailed) {
    use schema::download_tasks::dsl::*;
    // modify the task in the DB such that the failed column is set to its
    // corresponding error
    conn.execute(
        diesel::update(
            schema::download_tasks::table.filter(schema::download_tasks::url.eq(&task.url)),
        )
        .set((
            failed.eq(Some(error)),
            last_failure.eq(Utc::now()),
            num_failures.eq(num_failures + 1),
        )),
    )
    .expect("Failed to update download task after error");
}

#[cfg(test)]
mod dl_queue_tests {
    use crate::download_queue::DownloadTask;

    // tests for the url path to file stuff
    #[test]
    fn test_url_depth() {
        assert_eq!(
            DownloadTask::url_depth("https://www.example.com/foo/bar/baz.tar.gz"),
            3
        );
        assert_eq!(DownloadTask::url_depth("https://www.example.com/"), 1);
        assert_eq!(DownloadTask::url_depth("aaaaa"), 0);
        assert_eq!(
            DownloadTask::url_depth(
                "https://registry.npmjs.org/@_000407/transpose.js/-/transpose.js-1.0.1.tgz"
            ),
            4
        );
        assert_eq!(DownloadTask::url_depth(""), 0);
        assert_eq!(
            DownloadTask::url_depth(
                "https://registry.npmjs.org/@bolt/components-button-group/-/components-button-group-2.21.0-canary.12348.5.0.tgz"
            ),
            4
        );
        assert_eq!(
            DownloadTask::url_depth("https://registry.npmjs.org/vs-deploy/-/vs-deploy-1.5.0.tgz"),
            3
        );
    }

    #[test]
    fn test_filename_for_task() {
        // macro for making the string
        macro_rules! s {
            ($url:expr) => {
                String::from($url)
            };
        }
        assert_eq!(
            DownloadTask::get_filename("https://www.example.com/foo/bar/baz.tar.gz").unwrap(),
            s!("baz.tar.gz")
        );
        assert!(DownloadTask::get_filename("https://www.example.com/").is_err());
        assert!(DownloadTask::get_filename("aaaaa").is_err());
        assert_eq!(
            DownloadTask::get_filename(
                "https://registry.npmjs.org/@_000407/transpose.js/-/transpose.js-1.0.1.tgz"
            )
            .unwrap(),
            s!("@_000407-transpose.js-1.0.1.tgz")
        );
        assert_eq!(
            DownloadTask::get_filename(
                "https://registry.npmjs.org/@bolt/components-button-group/-/components-button-group-2.21.0-canary.12348.5.0.tgz"
            ).unwrap(),
            s!("@bolt-components-button-group-2.21.0-canary.12348.5.0.tgz")
        );
        assert_eq!(
            DownloadTask::get_filename(
                "https://registry.npmjs.org/vs-deploy/-/vs-deploy-1.5.0.tgz"
            )
            .unwrap(),
            s!("vs-deploy-1.5.0.tgz")
        );
    }
}
