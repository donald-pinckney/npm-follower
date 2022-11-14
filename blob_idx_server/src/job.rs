use std::sync::Arc;

use dashmap::DashMap;
use lazy_static::__Deref;
use serde::{Deserialize, Serialize};
use tokio::sync::{
    mpsc::{Receiver, Sender},
    Mutex,
};
use tokio::task::spawn;

use crate::{
    debug,
    errors::{ClientError, JobError},
    job::worker::WorkerStatus,
    ssh::{Ssh, SshFactory, SshSessionFactory},
};

use self::pool::WorkerPool;

pub(super) mod pool;
pub(super) mod worker;

/// The response that the worker client sends to the server.
#[derive(Debug, Serialize, Deserialize)]
pub struct ClientResponse {
    pub error: Option<ClientError>,
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

        let out_res = ssh.run_command(&cmd).await;
        drop(worker);
        let out = out_res?;
        debug!("Output:\n{}", out);

        // parse into a ClientResponse
        let response: ClientResponse =
            serde_json::from_str(&out).map_err(|_| JobError::ClientOutputNotParsable(out))?;

        match response.error {
            Some(e) => Err(JobError::ClientError(e)),
            None => Ok(()),
        }
    }

    /// Submits a read job to the discovery cluster. Returns the data in base64 format.
    /// This should not be used for computation, just for situational retrieval
    /// of data.
    pub async fn submit_read_job(&self, key: String) -> Result<String, JobError> {
        debug!("Submitting read job with key {}", key);
        let worker = self.worker_pool.get_worker().await?;

        todo!()
    }
}
