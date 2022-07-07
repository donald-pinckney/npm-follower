use std::sync::mpsc::channel;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::sync::Mutex;

use crate::custom_types::DownlaodFailed;
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
    pub failed: Option<DownlaodFailed>,
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

    /// Downloads this task to the given directory.
    pub async fn do_download(&self, dest: &str) -> Result<DownloadedTarball, DownloadError> {
        // get the file and download it to dir
        let res = reqwest::get(&self.url).await?;
        let status = res.status();
        if status != reqwest::StatusCode::OK {
            return Err(DownloadError::StatusNotOk(status));
        }
        let name = self.url.split('/').last().unwrap();
        let path = std::path::Path::new(dest).join(name);
        let mut file = std::fs::File::create(path.clone())?;
        let mut body = std::io::Cursor::new(res.bytes().await?);
        std::io::copy(&mut body, &mut file)?;

        let task = DownloadedTarball::from_task(
            self,
            // makes the path absolute
            std::fs::canonicalize(path)?.to_str().unwrap().to_string(),
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

/// Downloads all present tasks to the given directory. Inserts each task completed in the
/// downloaded_tarballs table, and removes the completed tasks from the download_tasks table.
pub fn download_to_dest(
    conn: &DbConnection,
    dest: &str,
    num_workers: usize,
) -> std::io::Result<()> {
    use schema::download_tasks::dsl::*;

    let tasks: Vec<DownloadTask> = download_tasks
        .order(queue_time.asc()) // order by the time they got queued, in ascending order
        .load(&conn.conn)
        .expect("Failed to load download tasks from DB");
    let tasks_len = tasks.len();

    let (db_sender, db_receiver) = channel();
    let pool = DownloadThreadPool::new(num_workers, dest, db_sender);

    for task in tasks {
        pool.download(task);
    }

    for _ in 0..tasks_len {
        // TODO: handle error
        match db_receiver.recv().unwrap() {
            DbMessage::Tarball(tarball) => {
                // insert the tarball into the DB
                diesel::insert_into(schema::downloaded_tarballs::table)
                    .values(&*tarball)
                    .execute(&conn.conn)
                    .expect("Failed to insert downloaded tarball");

                // delete the task from the download_tasks table
                diesel::delete(
                    schema::download_tasks::table
                        .filter(schema::download_tasks::url.eq(&tarball.tarball_url)),
                )
                .execute(&conn.conn)
                .expect("Failed to delete download task");
                println!("Inserted and removed task {}", tarball.tarball_url);
            }
            DbMessage::Error(e, task) => {
                println!("Error: {} from task: {}", e, task.url);
            }
        }
    }

    println!("done");

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
