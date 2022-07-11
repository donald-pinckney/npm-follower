use std::os::unix::prelude::PermissionsExt;
use std::sync::mpsc::channel;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::sync::Mutex;

use crate::custom_types::DownloadFailed;
use crate::download_tarball::DownloadError;
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
    pub fn get_filename(url: &str) -> Result<String, DownloadError> {
        let slashsplit = url.split('/').collect::<Vec<&str>>();
        let base_name = slashsplit.last().ok_or(DownloadError::BadlyFormattedUrl)?;
        // 3 = normal package
        // 4 = namespace
        match Self::url_depth(url) {
            3 => Ok(base_name.to_string()),
            4 => {
                let namespace = slashsplit.get(3).ok_or(DownloadError::BadlyFormattedUrl)?;
                Ok(format!("{}-{}", namespace, base_name))
            }
            _ => Err(DownloadError::BadlyFormattedUrl),
        }
    }

    /// Downloads this task to the given directory. This function cannot panic.
    pub async fn do_download(&self, dest: &str) -> Result<DownloadedTarball, DownloadError> {
        // get the file and download it to dir
        let res = reqwest::get(&self.url).await?;
        let status = res.status();
        if status != reqwest::StatusCode::OK {
            return Err(DownloadError::StatusNotOk(status));
        }
        let name = Self::get_filename(&self.url)?;
        let path = std::path::Path::new(dest).join(name);
        let mut file = std::fs::File::create(path.clone())?;
        file.set_permissions(std::fs::Permissions::from_mode(0o774))?; // rwxrwxr--
        let mut body = std::io::Cursor::new(res.bytes().await?);
        std::io::copy(&mut body, &mut file)?;

        let task = DownloadedTarball::from_task(
            self,
            // makes the path absolute
            std::fs::canonicalize(path)?
                .to_str()
                .ok_or(DownloadError::BadlyFormattedUrl)?
                .to_string(),
        );

        Ok(task)
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

const TASKS_CHUNK_SIZE: i64 = 1024;

fn get_total_tasks_num(conn: &DbConnection, retry_failed: bool) -> i64 {
    use schema::download_tasks::dsl::*;

    if retry_failed {
        download_tasks
            .count()
            .get_result(&conn.conn)
            .expect("Failed to get number of tasks")
    } else {
        download_tasks
            .filter(failed.is_null())
            .count()
            .get_result(&conn.conn)
            .expect("Failed to get number of tasks")
    }
}

fn load_chunk_init(conn: &DbConnection, retry_failed: bool) -> Vec<DownloadTask> {
    use schema::download_tasks::dsl::*;
    if retry_failed {
        download_tasks
            .order(url.asc()) // order by the time they got queued, in ascending order
            .limit(TASKS_CHUNK_SIZE)
            .load(&conn.conn)
            .expect("Failed to load download tasks from DB")
    } else {
        download_tasks
            .order(url.asc()) // order by the time they got queued, in ascending order
            .filter(failed.is_null())
            .limit(TASKS_CHUNK_SIZE)
            .load(&conn.conn)
            .expect("Failed to load download tasks from DB")
    }
}

fn load_chunk_next(
    conn: &DbConnection,
    last_url: &String,
    retry_failed: bool,
) -> Vec<DownloadTask> {
    use schema::download_tasks::dsl::*;
    if retry_failed {
        download_tasks
            .order(url.asc()) // order by the time they got queued, in ascending order
            .filter(url.gt(last_url))
            .limit(TASKS_CHUNK_SIZE)
            .load(&conn.conn)
            .expect("Failed to load download tasks from DB")
    } else {
        download_tasks
            .order(url.asc()) // order by the time they got queued, in ascending order
            .filter(failed.is_null().and(url.gt(last_url)))
            .limit(TASKS_CHUNK_SIZE)
            .load(&conn.conn)
            .expect("Failed to load download tasks from DB")
    }
}

fn update_from_tarballs(conn: &DbConnection, tarballs: &mut Vec<DownloadedTarball>) {
    use schema::download_tasks::dsl::*;
    if !tarballs.is_empty() {
        println!("Inserting {} tarballs", tarballs.len());
        // insert all the tarballs from download_queue in the db
        diesel::insert_into(schema::downloaded_tarballs::table)
            .values(&*tarballs)
            .execute(&conn.conn)
            .expect("Failed to insert downloaded tarballs into DB");

        // delete the tasks from download_tasks that are contained in download_queue
        diesel::delete(download_tasks)
            .filter(url.eq_any(tarballs.iter().map(|x| x.tarball_url.clone())))
            .execute(&conn.conn)
            .expect("Failed to delete downloaded tasks from DB");

        // clear the queue
        tarballs.clear();
    }
}

/// Downloads all present tasks to the given directory. Inserts each task completed in the
/// downloaded_tarballs table, and removes the completed tasks from the download_tasks table.
/// The given number of workers represents the number of threads that will be used to download the
/// tasks, where for each thread there is a new parallel download. if retry_failed is true, it will
/// retry to download failed tasks.
///
/// # Panics
///
/// If the number of workers is 0 or greater than TASKS_CHUNK_SIZE (unreasonable amount).
pub fn download_to_dest(
    conn: &DbConnection,
    dest: &str,
    num_workers: usize,
    retry_failed: bool,
) -> std::io::Result<()> {
    use schema::download_tasks::dsl::*;
    assert!(TASKS_CHUNK_SIZE > num_workers as i64 && num_workers > 0);

    // get all tasks with no failed downloads
    let tasks_len = get_total_tasks_num(conn, retry_failed);
    println!("{} tasks to download", tasks_len);

    let (db_sender, db_receiver) = channel();
    let pool = DownloadThreadPool::new(num_workers, dest, db_sender);

    // get first round of tasks, with no failed downloads
    let tasks: Vec<DownloadTask> = load_chunk_init(conn, retry_failed);
    println!("Got {} tasks", tasks.len());

    if tasks.is_empty() {
        return Ok(());
    }

    // ---  variables to keep for safely querying new chunks of tasks ---

    // the last url of the task that was queried
    let mut last_url = tasks.last().unwrap().url.clone();
    // the last chunk size that was quried
    let mut last_chunk_size = tasks.len();
    // the counter of downloads per chunk (gets reset on each chunk)
    let mut download_counter = 0;
    // the queue of tarballs that have been downloaded and are waiting to be inserted into the db
    let mut tarballs_queue: Vec<DownloadedTarball> = vec![];

    // pool the first round of tasks
    pool.download_chunk(tasks);

    for i in 0..tasks_len {
        println!("{}/{}", i, tasks_len);
        if (download_counter + 1) == last_chunk_size {
            update_from_tarballs(conn, &mut tarballs_queue);

            println!("Sending new chunk of tasks to pool");
            // get next round of tasks, with no failed downloads and with tasks that have greater
            // url sort-position than the last chunk
            let tasks: Vec<DownloadTask> = load_chunk_next(conn, &last_url, retry_failed);
            println!("Got {} tasks", tasks.len());

            // reassign last_url and last_chunk_size to the new chunk of tasks
            if !tasks.is_empty() {
                last_url = tasks.last().unwrap().url.clone();
            }
            last_chunk_size = tasks.len();
            // reset download_counter
            download_counter = 0;

            // pool the new chunk of tasks
            pool.download_chunk(tasks);
        }
        match db_receiver.recv().unwrap() {
            // NOTE: the tarballs get inserted in chunks, but errors don't. This is because it
            // doesn't make sense to pool errors as we have to update each row in the tasks in a
            // loop.
            DbMessage::Tarball(tarball) => {
                println!("Done downloading task {}", tarball.tarball_url);
                tarballs_queue.push(*tarball);
            }
            DbMessage::Error(e, task) => {
                println!("Error downloading task {} -> {}", task.url, e);
                // modify the task in the DB such that the failed column is set to its
                // corresponding error
                diesel::update(
                    schema::download_tasks::table.filter(schema::download_tasks::url.eq(&task.url)),
                )
                .set(failed.eq(Some(DownloadFailed::from(e))))
                .execute(&conn.conn)
                .expect("Failed to update download task after error");
            }
        }
        download_counter += 1;
    }

    update_from_tarballs(conn, &mut tarballs_queue);

    println!("Done downloading tasks");

    Ok(())
}

// the channel message for resulting tarballs to be inserted into the database
pub enum DbMessage {
    Tarball(Box<DownloadedTarball>),
    Error(DownloadError, Box<DownloadTask>), // The error and the task that produced it
}

// The channel message for download tasks in the download thread pool
enum TaskMessage {
    Task(Box<DownloadTask>), // where the string is the path to the destination directory
    Exit,
}

#[derive(Debug)]
struct Worker {
    id: usize,
    thread: Option<tokio::task::JoinHandle<()>>,
}

impl Worker {
    fn make(
        id: usize,
        task_receiver: Arc<Mutex<Receiver<TaskMessage>>>,
        handle: tokio::runtime::Handle,
        db_sender: Sender<DbMessage>,
        destination: &str,
    ) -> Worker {
        let dest = destination.to_string();
        let task = handle.spawn(async move {
            println!("Worker {} started", id);
            loop {
                let msg = task_receiver.lock().unwrap().recv().unwrap();
                match msg {
                    TaskMessage::Task(dl) => {
                        println!("Worker {} downloading {}", id, dl.url);
                        let tarball = dl.do_download(&dest).await;
                        match tarball {
                            Ok(tar) => db_sender.send(DbMessage::Tarball(Box::new(tar))).unwrap(),
                            Err(e) => db_sender.send(DbMessage::Error(e, dl)).unwrap(),
                        }
                    }
                    TaskMessage::Exit => break,
                }
            }
        });

        Worker {
            id,
            thread: Some(task),
        }
    }
}

#[derive(Debug)]
pub struct DownloadThreadPool {
    workers: Vec<Worker>,
    task_sender: Sender<TaskMessage>,
    tokio_runtime: tokio::runtime::Runtime,
}

impl DownloadThreadPool {
    pub fn new(size: usize, dest: &str, db_sender: Sender<DbMessage>) -> DownloadThreadPool {
        assert!(size > 0);

        let (task_sender, task_receiver) = channel();

        let arc_task_receiver = Arc::new(Mutex::new(task_receiver));

        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(size)
            .enable_all()
            .build()
            .unwrap();

        let mut workers = Vec::with_capacity(size);
        for id in 0..size {
            workers.push(Worker::make(
                id,
                Arc::clone(&arc_task_receiver),
                rt.handle().clone(),
                db_sender.clone(),
                dest,
            ));
        }

        DownloadThreadPool {
            workers,
            task_sender,
            tokio_runtime: rt,
        }
    }

    pub fn download_chunk(&self, tasks: Vec<DownloadTask>) {
        for task in tasks {
            self.download(task);
        }
    }

    pub fn download(&self, task: DownloadTask) {
        self.task_sender
            .send(TaskMessage::Task(Box::new(task)))
            .unwrap();
    }
}

impl Drop for DownloadThreadPool {
    fn drop(&mut self) {
        println!("Sending terminate message to all workers.");

        for _ in &self.workers {
            self.task_sender.send(TaskMessage::Exit).unwrap();
        }

        for worker in &mut self.workers {
            println!("Shutting down worker {}", worker.id);

            let thread = worker.thread.take().unwrap();
            self.tokio_runtime.block_on(async {
                thread.await.unwrap();
            });
        }
    }
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
