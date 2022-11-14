use std::sync::Arc;

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tokio::sync::{
    mpsc::{Receiver, Sender},
    Mutex,
};
use tokio::task::spawn;

use crate::{
    debug,
    errors::{ClientError, JobError},
    ssh::{Ssh, SshFactory, SshSessionFactory},
};

/// The response that the worker client sends to the server.
#[derive(Debug, Serialize, Deserialize)]
pub struct ClientResponse {
    pub error: Option<ClientError>,
}

/// A resource pool data structure that is used to query available worker jobs.
struct WorkerPool {
    /// map of [discovery's job_id] -> [worker]
    pool: Arc<DashMap<u64, Worker>>,
    /// channel that notifies that a worker is available.
    avail_rx: Mutex<Receiver<u64>>,
    /// done channel, this will be cloned and sent to the workers.
    avail_tx: Sender<u64>,
    /// The maximum amount of worker jobs that can be running at the same time.
    max_worker_jobs: usize,
    /// ssh session for managing workers.
    ssh_session: Box<dyn Ssh>,
    /// ssh factory, for creating new ssh sessions.
    ssh_factory: Box<dyn SshFactory>,
    /// The cleanup tasks for each worker.
    cleanup_tasks: Arc<DashMap<u64, tokio::task::JoinHandle<()>>>,
}

impl WorkerPool {
    /// Initializes the worker pool with the given maximum number of workers and the given ssh session.
    async fn init(max_worker_jobs: usize, ssh_factory: Box<dyn SshFactory>) -> Self {
        let (tx, rx): (Sender<u64>, Receiver<u64>) = tokio::sync::mpsc::channel(max_worker_jobs);
        Self {
            pool: Arc::new(DashMap::new()),
            avail_tx: tx,
            avail_rx: Mutex::new(rx),
            max_worker_jobs,
            ssh_session: ssh_factory
                .spawn()
                .await
                .expect("failed to create ssh session"),
            ssh_factory,
            cleanup_tasks: Arc::new(DashMap::new()),
        }
    }

    /// Spawns a cleanup task that will remove the worker from the pool after a given amount of time.
    fn spawn_cleanup(&self, job_id: u64, when: chrono::DateTime<chrono::Utc>) {
        let pool = self.pool.clone();
        let cleanup_tasks = self.cleanup_tasks.clone();
        debug!(
            "Spawning cleanup task for job {}. cleaning at {}",
            job_id, when
        );
        self.cleanup_tasks.insert(
            job_id,
            spawn(async move {
                let now = chrono::Utc::now();
                if now < when {
                    let dur = when - now;
                    tokio::time::sleep(std::time::Duration::from_millis(
                        dur.num_milliseconds() as u64
                    ))
                    .await;
                }
                pool.remove(&job_id);
                cleanup_tasks.remove(&job_id);
                debug!("Cleaned up job {} from pool", job_id);
            }),
        );
    }

    /// Populates the worker pool with workers.
    /// Checks if there are any workers already running in discovery, if so,
    /// it will add them to the pool.
    ///
    /// # Panics
    /// If the worker pool is already populated (i.e. not empty).
    async fn populate(&mut self) -> Result<(), JobError> {
        assert!(self.pool.is_empty());

        // produces "job_id status hour:min:sec node_id"
        let cmd = "squeue -u $USER | grep job_work | awk -F ' +' '{print $2, $6, $7, $9}'";
        let output = self.ssh_session.run_command(cmd).await?;

        // check if empty
        let mut worker_count = 0;
        if !output.is_empty() {
            let lines = output.lines();
            for line in lines {
                if worker_count >= self.max_worker_jobs {
                    break;
                }
                let mut parts = line.split_whitespace();
                let job_id = parts
                    .next()
                    .unwrap()
                    .parse::<u64>()
                    .expect("Failed to parse job_id");
                let status = parts.next().unwrap();
                let time = parts.next().unwrap();

                let time_now = chrono::Utc::now();
                // parse time from "hour:min:sec", but could just be "min:sec"
                let job_time = if time.matches(':').count() == 3 {
                    let mut parts = time.split(':');
                    let hour = parts.next().unwrap().parse::<i64>().unwrap();
                    let min = parts.next().unwrap().parse::<i64>().unwrap();
                    let sec = parts.next().unwrap().parse::<i64>().unwrap();
                    // get current time and subtract the time from the job
                    time_now
                        - chrono::Duration::hours(hour)
                        - chrono::Duration::minutes(min)
                        - chrono::Duration::seconds(sec)
                } else {
                    let mut parts = time.split(':');
                    let min = parts.next().unwrap().parse::<i64>().unwrap();
                    let sec = parts.next().unwrap().parse::<i64>().unwrap();
                    // get current time and subtract the time from the job
                    time_now - chrono::Duration::minutes(min) - chrono::Duration::seconds(sec)
                };

                let node_id = parts.next().unwrap();
                debug!(
                    "Found worker: {}, {}, {}, {}",
                    job_id, status, job_time, node_id
                );
                let worker_status = {
                    if status == "R" {
                        WorkerStatus::Running {
                            started_at: job_time,
                            ssh_session: self.ssh_factory.spawn_jumped(node_id).await?,
                            node_id: node_id.to_string(),
                        }
                    } else {
                        WorkerStatus::Queued
                    }
                };
                self.pool.insert(
                    job_id,
                    Worker {
                        job_id,
                        status: Arc::new(worker_status),
                        avail_tx: self.avail_tx.clone(),
                    },
                );
                let cleanup_time = time_now + (chrono::Duration::hours(24) - (time_now - job_time));
                self.spawn_cleanup(job_id, cleanup_time);
                self.avail_tx.send(job_id).await.unwrap();
                worker_count += 1;
            }
        }

        // adding new workers if needed
        for i in worker_count..self.max_worker_jobs {
            debug!("Adding worker {}", i);
            self.spawn_worker(true).await?;
        }

        Ok(())
    }

    /// Spawns a new worker and adds it to the pool. This worker will be queued on discovery,
    /// so it won't be available for work until discovery is done.
    /// `do_send` determines whether we should notify the channel that a worker is available.
    async fn spawn_worker(&self, do_send: bool) -> Result<u64, JobError> {
        if self.pool.len() >= self.max_worker_jobs {
            return Err(JobError::MaxWorkerJobsReached);
        }

        let cmd = "sbatch worker.sh | cut -d ' ' -f4".to_string();
        debug!("Running command: {}", cmd);
        let out = self.ssh_session.run_command(&cmd).await?;
        let job_id = out
            .parse::<u64>()
            .map_err(|_| JobError::CommandFailed { cmd, output: out })?;
        let worker = Worker {
            job_id,
            avail_tx: self.avail_tx.clone(),
            status: Arc::new(WorkerStatus::Queued),
        };

        self.pool.insert(job_id, worker);

        if do_send {
            self.avail_tx.send(job_id).await.unwrap();
        }
        self.spawn_cleanup(job_id, chrono::Utc::now() + chrono::Duration::hours(24));

        Ok(job_id)
    }

    /// Waits that the given worker shows up as running on discovery and updates the worker's status.
    async fn wait_running(&self, worker: Worker) -> Result<Worker, JobError> {
        while !worker.is_running(&*self.ssh_session).await? {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
        // get status, assume it's Queued
        let status = match &*worker.status {
            WorkerStatus::Queued => {
                let node_id = worker.get_node_id(&*self.ssh_session).await?;
                let ssh_session = self.ssh_factory.spawn_jumped(&node_id).await?;
                WorkerStatus::Running {
                    started_at: chrono::Utc::now(),
                    ssh_session,
                    node_id,
                }
            }
            _ => panic!("Worker should be queued"),
        };
        // now that the worker is running, we can update the status.
        let new_worker = Worker {
            status: Arc::new(status),
            ..worker
        };

        debug!("Worker {} is running", new_worker.job_id);

        self.pool.insert(new_worker.job_id, new_worker.clone());

        Ok(new_worker)
    }

    /// Returns a worker from the pool, if there is no worker available, it will wait until one is
    /// available.
    /// - A worker lives for 24 hours, after that it will be dropped.
    ///   We want to get workers that are maximum 23 hours old, so we can reuse them.
    ///   Therefore, this function will also check for expired workers and remove them from the pool,
    ///   adding a new worker to the pool.
    /// - Workers processed may still be queued, in that case we will wait until they are running.
    /// - Some workers may have network issues, in that case, we will trash them and add a new one.
    async fn get_worker(&self) -> Result<Worker, JobError> {
        debug!("Waiting for jobs to be available");
        let job_id = self.avail_rx.lock().await.recv().await.unwrap();
        debug!("Got job {}", job_id);
        let worker = self.pool.get(&job_id).unwrap().value().clone();
        match &*worker.status {
            WorkerStatus::Queued => {
                // check/wait until worker is running, update status to running
                debug!("Found queued worker {}, waiting for it to run", job_id);
                self.wait_running(worker).await
            }
            WorkerStatus::Running {
                started_at,
                node_id: _,
                ssh_session: _,
            } => {
                // check if worker is expired
                let now = chrono::Utc::now();
                let worker_age = now - *started_at;
                if worker_age > chrono::Duration::hours(23) {
                    // expired, remove from pool and add a new worker
                    debug!("Found expired worker {}, removing", job_id);
                    self.pool.remove(&job_id);
                    let new_worker = self.spawn_worker(false).await?;
                    debug!("Added new worker {}", new_worker);
                    self.wait_running(self.pool.get(&new_worker).unwrap().value().clone())
                        .await
                } else {
                    debug!("Found running worker {}", job_id);
                    // check if network is ok
                    if worker.is_network_up().await? {
                        Ok(worker)
                    } else {
                        // network is down, remove from pool and add a new worker
                        debug!("Network is down for worker {}, removing", job_id);
                        self.pool.remove(&job_id);
                        let new_worker = self.spawn_worker(false).await?;
                        debug!("Added new worker {}, waiting for running...", new_worker);
                        self.wait_running(self.pool.get(&new_worker).unwrap().value().clone())
                            .await
                    }
                }
            }
        }
    }
}

#[derive(Clone)]
struct Worker {
    /// the discovery job id
    job_id: u64,
    /// the status of the worker
    status: Arc<WorkerStatus>,
    avail_tx: Sender<u64>, // where u64 is the job_id
}

impl Worker {
    /// Checks if the worker is out of the queue or not.
    async fn is_running(&self, session: &dyn Ssh) -> Result<bool, JobError> {
        let out = session
            .run_command(&format!(
                "squeue -u $USER | grep {} | awk -F ' +' '{{print $6}}'",
                self.job_id
            ))
            .await?;
        Ok(out == "R")
    }

    /// Gets the node id of the worker.
    async fn get_node_id(&self, session: &dyn Ssh) -> Result<String, JobError> {
        let out = session
            .run_command(&format!(
                "squeue -u $USER | grep {} | awk -F ' +' '{{print $9}}'",
                self.job_id
            ))
            .await?;
        Ok(out)
    }

    /// Checks if the worker is able to ping `1.1.1.1`, if it can't, the network is down on
    /// the worker. Assumes the given worker is running.
    async fn is_network_up(&self) -> Result<bool, JobError> {
        match &*self.status {
            WorkerStatus::Running {
                started_at: _,
                node_id: _,
                ssh_session,
            } => {
                let out = match ssh_session.run_command("ping -w 3 -c 1 1.1.1.1").await {
                    Ok(out) => out,
                    Err(JobError::CommandNonZero { cmd: _, output }) => output,
                    Err(e) => return Err(e),
                };
                Ok(!out.contains("100% packet loss"))
            }
            _ => panic!("Worker should be running"),
        }
    }

    /// Releases the worker, making it available for other jobs.
    async fn release(&self) {
        self.avail_tx.send(self.job_id).await.unwrap();
    }
}

enum WorkerStatus {
    Queued,
    Running {
        started_at: chrono::DateTime<chrono::Utc>,
        ssh_session: Box<dyn Ssh>,
        node_id: String,
    },
}

/// Configuration to initialize a job manager.
#[derive(Debug, Clone)]
pub struct JobManagerConfig {
    /// The user and host for the ssh connection.
    pub ssh_user_host: String,
    /// The maximum amount of worker jobs that can be running at the same time.
    pub max_worker_jobs: usize,
}

pub struct JobManager {
    config: JobManagerConfig,
    worker_pool: WorkerPool,
}

impl JobManager {
    pub(crate) async fn init_with_ssh(
        config: JobManagerConfig,
        ssh_factory: Box<dyn SshFactory>,
    ) -> Self {
        let mut worker_pool = WorkerPool::init(config.max_worker_jobs, ssh_factory).await;
        worker_pool
            .populate()
            .await
            .expect("populate worker pool failed");

        // print if on debug
        for worker in worker_pool.pool.iter() {
            debug!("Worker {} is running", worker.value().job_id);
        }

        Self {
            config,
            worker_pool,
        }
    }

    pub async fn init(config: JobManagerConfig) -> Self {
        let factory = Box::new(SshSessionFactory::new(&config.ssh_user_host));

        Self::init_with_ssh(config, factory).await
    }

    /// Submits a download and write job to the discovery cluster.
    pub async fn submit_download_job(&self, urls: Vec<String>) -> Result<(), JobError> {
        debug!("Submitting download job with {} urls", urls.len());
        let worker = self.worker_pool.get_worker().await?;
        let (node_id, ssh) = match &*worker.status {
            WorkerStatus::Running {
                node_id,
                ssh_session,
                ..
            } => (node_id, ssh_session),
            _ => panic!("Worker should be running"),
        };

        let urls = urls.join(" ");

        let cmd = format!(
            "cd $HOME/npm-follower/blob_idx_client && ./run.sh write {} \"{}\"",
            node_id, urls
        );

        debug!("Running command:\n{}", cmd);

        let out = ssh.run_command(&cmd).await?;
        debug!("Output:\n{}", out);

        // parse into a ClientResponse
        let response: ClientResponse =
            serde_json::from_str(&out).map_err(|_| JobError::ClientOutputNotParsable(out))?;

        debug!("Releasing worker {}", worker.job_id);
        worker.release().await;

        match response.error {
            Some(e) => Err(JobError::ClientError(e)),
            None => Ok(()),
        }
    }
}
