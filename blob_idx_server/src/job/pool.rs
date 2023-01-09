use std::sync::Arc;

use dashmap::DashMap;
use lazy_static::__Deref;

use tokio::sync::{
    mpsc::{Receiver, Sender},
    Mutex, RwLock,
};

use crate::{
    debug,
    errors::JobError,
    job::worker::WorkerStatus,
    ssh::{Ssh, SshFactory},
};

use super::worker::Worker;

/// A resource pool data structure that is used to query available worker jobs.
pub(super) struct WorkerPool {
    /// The name of the pool. Max 8 characters. Has to be unique,
    /// other pools with the same name will be rejected.
    name: String,
    /// map of [discovery's job_id] -> [worker]
    pool: Arc<DashMap<u64, Worker>>,
    /// channel that notifies that a worker is available.
    avail_rx: Mutex<Receiver<u64>>,
    /// done channel, this will be cloned and sent to the workers.
    avail_tx: Sender<u64>,
    /// The maximum amount of worker jobs that can be running at the same time.
    /// This number may be exceeded by one for expiring jobs.
    max_worker_jobs: usize,
    /// ssh session for managing workers.
    ssh_session: Box<dyn Ssh>,
    /// ssh factory, for creating new ssh sessions.
    ssh_factory: Arc<Box<dyn SshFactory>>,
}

impl WorkerPool {
    /// Initializes the worker pool with the given maximum number of workers and the given ssh session.
    /// The given name of the pool is used to identify the pool in the squeue output, so it
    /// has to be unique.
    pub(crate) async fn init(
        max_worker_jobs: usize,
        pool_name: impl Into<String>,
        ssh_factory: Arc<Box<dyn SshFactory>>,
    ) -> Self {
        let name = pool_name.into();
        assert!(name.len() <= 8, "pool name too long");
        let (tx, rx): (Sender<u64>, Receiver<u64>) =
            tokio::sync::mpsc::channel(std::cmp::max(1, max_worker_jobs));
        let pool = Arc::new(DashMap::new());
        let ssh_session = ssh_factory
            .spawn()
            .await
            .expect("failed to create ssh session");
        Self {
            name,
            pool,
            avail_tx: tx,
            avail_rx: Mutex::new(rx),
            max_worker_jobs,
            ssh_session,
            ssh_factory,
        }
    }

    /// Populates the worker pool with workers.
    /// Checks if there are any workers already running in discovery, if so,
    /// it will add them to the pool.
    ///
    /// # Panics
    /// If the worker pool is already populated (i.e. not empty).
    pub(crate) async fn populate(&mut self) -> Result<(), JobError> {
        assert!(self.pool.is_empty());

        // produces "job_id status hour:min:sec node_id"
        let cmd = format!(
            "squeue -u $USER | grep {} | awk -F ' +' '{{print $2, $6, $7, $9}}'",
            self.name
        );
        let output = self.ssh_session.run_command(&cmd).await?;

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
                let job_time = crate::job::worker::parse_time(time).expect("Failed to parse time");
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
                    } else if status == "PD" {
                        WorkerStatus::Queued
                    } else {
                        continue;
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
                self.avail_tx.send(job_id).await.unwrap();
                worker_count += 1;
            }
        }

        // adding new workers if needed
        for i in worker_count..self.max_worker_jobs {
            debug!("Adding worker {}", i);
            self.spawn_worker().await?;
        }

        Ok(())
    }

    /// Spawns a new worker and adds it to the pool. This worker will be queued on discovery,
    /// so it won't be available for work until discovery is done.
    pub(crate) async fn spawn_worker(&self) -> Result<u64, JobError> {
        if self.pool.len() >= self.max_worker_jobs {
            return Err(JobError::MaxWorkerJobsReached);
        }

        let cmd = format!("sbatch --job-name={} worker.sh | cut -d ' ' -f4", self.name);
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
        self.avail_tx.send(job_id).await.unwrap();

        Ok(job_id)
    }

    /// Waits that the given worker shows up as running on discovery and updates the worker's status.
    pub(crate) async fn wait_running(&self, worker: Worker) -> Result<Worker, JobError> {
        let mut run_time = worker.started_running_at(&*self.ssh_session).await?;
        while run_time.is_none() {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            run_time = worker.started_running_at(&*self.ssh_session).await?;
        }
        // get status, assume it's Queued
        let status = match &*worker.status {
            WorkerStatus::Queued => {
                let node_id = worker.get_node_id(&*self.ssh_session).await?;
                let ssh_session = self.ssh_factory.spawn_jumped(&node_id).await?;
                WorkerStatus::Running {
                    started_at: run_time.unwrap(),
                    ssh_session,
                    node_id,
                }
            }
            _ => panic!("Worker should be queued"),
        };
        // now that the worker is running, we can update the status.
        let new_worker = Worker {
            status: Arc::new(status),
            job_id: worker.job_id,
            avail_tx: worker.avail_tx.clone(),
        };

        debug!("Worker {} is running", new_worker.job_id);

        self.pool.insert(new_worker.job_id, new_worker.clone());

        debug!("Inserted running worker {} into pool", new_worker.job_id);

        Ok(new_worker)
    }

    /// Returns a worker from the pool, if there is no worker available, it will wait until one is
    /// available.
    /// - A worker lives for 8 hours, after that it will be dropped.
    ///   We want to get workers that are maximum 7 hours old, so we can reuse them.
    ///   Therefore, this function will also check for expired workers and remove them from the pool,
    ///   adding a new worker to the pool.
    /// - Workers processed may still be queued, in that case we will wait until they are running.
    /// - Some workers may have network issues, in that case, we will trash them and add a new one.
    pub(crate) async fn get_worker(&self) -> Result<WorkerGuard, JobError> {
        async fn helper(wp: &WorkerPool) -> Result<Option<Worker>, JobError> {
            debug!("Waiting for jobs to be available");
            let job_id = wp.avail_rx.lock().await.recv().await.unwrap();
            debug!("Got job {}", job_id);
            let worker = match wp.pool.get(&job_id) {
                Some(j) => j.value().clone(),
                None => return Ok(None),
            };
            match &*worker.status {
                WorkerStatus::Queued => {
                    // check/wait until worker is running, update status to running
                    debug!("Found queued worker {}, waiting for it to run", job_id);
                    match wp.wait_running(worker).await {
                        Ok(w) => Ok(Some(w)),
                        Err(e) => {
                            // put worker back in pool
                            wp.avail_tx.send(job_id).await.unwrap();
                            Err(e)
                        }
                    }
                }
                WorkerStatus::Running {
                    started_at,
                    node_id: _,
                    ssh_session: _,
                } => {
                    // check if worker is expired
                    let now = chrono::Utc::now();
                    let worker_age = now - *started_at;
                    debug!(
                        "Found running worker {}, age: {}m ({}h)",
                        job_id,
                        worker_age.num_minutes(),
                        worker_age.num_hours()
                    );
                    if worker_age > chrono::Duration::hours(7) {
                        // expired, remove from pool and add a new worker
                        debug!("Found expired worker {}, removing", job_id);
                        wp.replace_worker(&worker).await.ok();
                        Ok(None)
                    } else {
                        debug!("Found running worker {}", job_id);
                        // check if network is ok and not on login node
                        let node_id = worker.get_node_id_via_hostname().await;
                        let network_up = worker.is_network_up().await;
                        if node_id.is_ok()
                            && network_up.is_ok()
                            && network_up.unwrap()
                            && !node_id.unwrap().contains("login")
                        {
                            debug!("Network is up for worker {}", job_id);
                            Ok(Some(worker))
                        } else {
                            // network is down, remove from pool and add a new worker
                            debug!("Network is down for worker {}, removing", job_id);
                            wp.replace_worker(&worker).await.ok();
                            Ok(None)
                        }
                    }
                }
            }
        }

        loop {
            match helper(self).await? {
                Some(worker) => return Ok(WorkerGuard { worker }),
                None => continue,
            }
        }
    }

    /// Replaces the given worker with a new one.
    pub async fn replace_worker(&self, worker: &Worker) -> Result<(), JobError> {
        self.pool.remove(&worker.job_id);
        worker.cancel(&*self.ssh_session).await?;
        self.spawn_worker().await?;
        Ok(())
    }
}

pub(super) struct WorkerGuard {
    worker: Worker,
}

impl Drop for WorkerGuard {
    fn drop(&mut self) {
        let tx = self.worker.avail_tx.clone();
        let job_id = self.worker.job_id;
        tokio::spawn(async move {
            debug!("Releasing worker {}", job_id);
            tx.send(job_id).await.unwrap();
        });
    }
}

impl __Deref for WorkerGuard {
    type Target = Worker;

    fn deref(&self) -> &Self::Target {
        &self.worker
    }
}
