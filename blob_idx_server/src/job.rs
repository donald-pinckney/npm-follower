use chrono::TimeZone;
use dashmap::DashMap;
use tokio::sync::{
    mpsc::{Receiver, Sender},
    Mutex,
};

use crate::debug;

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
    /// ssh session for managing jobs
    ssh_session: SshSession,
    worker_pool: WorkerPool,
}

struct WorkerPool {
    /// map of [discovery's job_id] -> [worker]
    pool: DashMap<u64, Worker>,
    /// channel that notifies that a worker is available.
    avail_rx: Receiver<u64>,
    /// done channel, this will be cloned and sent to the workers.
    avail_tx: Sender<u64>,
    /// The maximum amount of worker jobs that can be running at the same time.
    max_worker_jobs: usize,
    /// The current amount of worker jobs that are running.
    /// This also gets locked when a worker is being added to the pool.
    current_worker_jobs: Mutex<usize>,
    /// ssh session for managing workers.
    ssh_session: SshSession,
}

impl WorkerPool {
    async fn init(max_worker_jobs: usize, ssh_user_host: &str) -> Self {
        let (tx, rx): (Sender<u64>, Receiver<u64>) = tokio::sync::mpsc::channel(max_worker_jobs);
        let ssh_session = SshSession::connect(ssh_user_host)
            .await
            .expect("Failed to connect to ssh");
        Self {
            pool: DashMap::new(),
            avail_tx: tx,
            avail_rx: rx,
            max_worker_jobs,
            current_worker_jobs: Mutex::new(0),
            ssh_session,
        }
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

                // parse time from "hour:min:sec", but could just be "min:sec"
                let time = if time.matches(':').count() == 3 {
                    let mut parts = time.split(':');
                    let hour = parts.next().unwrap().parse::<i64>().unwrap();
                    let min = parts.next().unwrap().parse::<i64>().unwrap();
                    let sec = parts.next().unwrap().parse::<i64>().unwrap();
                    // get current time and subtract the time from the job
                    let now = chrono::Utc::now();
                    now - chrono::Duration::hours(hour)
                        - chrono::Duration::minutes(min)
                        - chrono::Duration::seconds(sec)
                } else {
                    let mut parts = time.split(':');
                    let min = parts.next().unwrap().parse::<i64>().unwrap();
                    let sec = parts.next().unwrap().parse::<i64>().unwrap();
                    // get current time and subtract the time from the job
                    let now = chrono::Utc::now();
                    now - chrono::Duration::minutes(min) - chrono::Duration::seconds(sec)
                };

                let node_id = parts.next().unwrap();
                debug!("Found worker: {} {} {} {}", job_id, status, time, node_id);
                let worker_status = {
                    if status == "R" {
                        WorkerStatus::Running {
                            started_at: time,
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
                        status: worker_status,
                        avail_tx: self.avail_tx.clone(),
                    },
                );
                self.avail_tx.send(job_id).await.unwrap();
                worker_count += 1;
            }
        }

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
        let mut current_worker_jobs = self.current_worker_jobs.lock().await;
        if *current_worker_jobs >= self.max_worker_jobs {
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
            status: WorkerStatus::Queued,
        };

        self.pool.insert(job_id, worker);
        *current_worker_jobs += 1;

        if do_send {
            self.avail_tx.send(job_id).await.unwrap();
        }

        Ok(job_id)
    }

    /// Waits that the given worker shows up as running on discovery and updates the worker's status.
    async fn wait_running(&self, worker: Worker) -> Result<Worker, JobError> {
        while !worker.is_running(&self.ssh_session).await? {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
        // now that the worker is running, we can update the status.
        let new_worker = Worker {
            status: WorkerStatus::Running {
                started_at: chrono::Utc::now(),
                node_id: worker.get_node_id(&self.ssh_session).await?,
            },
            ..worker
        };

        debug!("Worker {:?} is running", new_worker);

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
    async fn get_worker(&mut self) -> Result<Worker, JobError> {
        let job_id = self.avail_rx.recv().await.unwrap();
        let worker = self.pool.get(&job_id).unwrap().value().clone();
        match worker.status {
            WorkerStatus::Queued => {
                // check/wait until worker is running, update status to running
                self.wait_running(worker).await
            }
            WorkerStatus::Running {
                started_at,
                node_id: _,
            } => {
                // check if worker is expired
                let now = chrono::Utc::now();
                let worker_age = now - started_at;
                if worker_age > chrono::Duration::hours(23) {
                    // expired, remove from pool and add a new worker
                    self.pool.remove(&job_id);
                    *self.current_worker_jobs.lock().await -= 1;
                    let new_worker = self.spawn_worker(false).await?;
                    self.wait_running(self.pool.get(&new_worker).unwrap().value().clone())
                        .await
                } else {
                    Ok(worker)
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
struct Worker {
    /// the discovery job id
    job_id: u64,
    /// the status of the worker
    status: WorkerStatus,
    avail_tx: Sender<u64>, // where u64 is the job_id
}

impl Worker {
    /// Checks if the worker is out of the queue or not.
    async fn is_running(&self, session: &SshSession) -> Result<bool, JobError> {
        let out = session
            .run_command(&format!(
                "squeue -u $USER | grep {} | awk -F ' +' '{{print $6}}'",
                self.job_id
            ))
            .await?;
        Ok(out == "R")
    }

    /// Gets the node id of the worker.
    async fn get_node_id(&self, session: &SshSession) -> Result<String, JobError> {
        let out = session
            .run_command(&format!(
                "squeue -u $USER | grep {} | awk -F ' +' '{{print $9}}'",
                self.job_id
            ))
            .await?;
        Ok(out)
    }
}

#[derive(Debug, Clone)]
enum WorkerStatus {
    Queued,
    Running {
        started_at: chrono::DateTime<chrono::Utc>,
        node_id: String,
    },
}

pub struct SshSession {
    session: Mutex<openssh::Session>,
    ssh_user_host: String,
}

impl SshSession {
    pub async fn connect(ssh_user_host: &str) -> Result<Self, JobError> {
        let session = openssh::Session::connect(ssh_user_host, openssh::KnownHosts::Accept).await?;
        Ok(Self {
            session: Mutex::new(session),
            ssh_user_host: ssh_user_host.to_string(),
        })
    }

    /// Runs the given command on the remote host. If the command fails due to a connection error,
    /// it will try to reconnect and run the command again.
    pub async fn run_command(&self, cmd: &str) -> Result<String, JobError> {
        let mut session = self.session.lock().await;
        let mut tries = 0;
        loop {
            let result = session.command("bash").args(["-c", cmd]).output().await;
            match result {
                Ok(output) => {
                    if output.status.success() {
                        return Ok(String::from_utf8(output.stdout)
                            .expect("invalid utf8")
                            .trim_end_matches('\n')
                            .to_string());
                    } else {
                        return Err(JobError::CommandNonZero {
                            cmd: cmd.to_string(),
                            output: String::from_utf8(output.stderr).expect("invalid utf8"),
                        });
                    }
                }
                Err(err) => {
                    if tries >= 3 {
                        return Err(JobError::CommandFailed {
                            cmd: cmd.to_string(),
                            output: err.to_string(),
                        });
                    }
                    debug!("ssh command '{}' failed, retrying: {}", cmd, err);
                    // try to reconnect
                    *session =
                        openssh::Session::connect(&self.ssh_user_host, openssh::KnownHosts::Accept)
                            .await?;
                    tries += 1;
                }
            }
        }
    }
}

impl JobManager {
    pub async fn init(config: JobManagerConfig) -> Self {
        let ssh_session = SshSession::connect(&config.ssh_user_host)
            .await
            .expect("ssh connect");
        let mut worker_pool = WorkerPool::init(config.max_worker_jobs, &config.ssh_user_host).await;
        worker_pool
            .populate()
            .await
            .expect("populate worker pool failed");
        Self {
            config,
            ssh_session,
            worker_pool,
        }
    }
}

#[derive(Debug)]
pub enum JobError {
    /// The maximum number of worker jobs is already running.
    MaxWorkerJobsReached,
    /// Error related to ssh.
    SshError(openssh::Error),
    /// The command failed.
    CommandFailed { cmd: String, output: String },
    /// The command returned a non-zero exit code.
    CommandNonZero { cmd: String, output: String },
}

impl std::fmt::Display for JobError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobError::MaxWorkerJobsReached => write!(f, "Maximum number of worker jobs reached"),
            JobError::SshError(e) => write!(f, "Ssh error: {}", e),
            JobError::CommandFailed { cmd, output } => {
                write!(f, "Command failed: {} - {}", cmd, output)
            }
            JobError::CommandNonZero { cmd, output } => {
                write!(
                    f,
                    "Command returned non-zero exit code: {} - {}",
                    cmd, output
                )
            }
        }
    }
}

impl From<openssh::Error> for JobError {
    fn from(e: openssh::Error) -> Self {
        JobError::SshError(e)
    }
}

impl std::error::Error for JobError {}
