use postgres_db::{download_queue::DownloadTask, download_tarball::DownloadedTarball};
use std::sync::mpsc::channel;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::sync::Mutex;

use crate::download_db::download_task;
use crate::download_error::DownloadError;

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
                        let tarball = download_task(&dl, &dest).await;
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
