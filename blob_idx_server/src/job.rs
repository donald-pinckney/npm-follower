use serde::{Deserialize, Serialize};

use crate::{
    debug,
    errors::{ClientError, JobError},
    job::worker::WorkerStatus,
    ssh::{Ssh, SshFactory},
};

use self::pool::WorkerPool;

pub(super) mod pool;
pub(super) mod worker;

/// The response that the worker client sends to the server.
#[derive(Debug, Serialize, Deserialize)]
pub struct ClientResponse {
    pub message: Option<String>,
    pub error: Option<ClientError>,
}

/// Configuration to initialize a job manager.
pub struct JobManagerConfig {
    /// The ssh factory to use to create ssh sessions.
    pub ssh_factory: Box<dyn SshFactory>,
    /// The maximum amount of worker jobs that can be running at the same time.
    pub max_worker_jobs: usize,
}

pub struct JobManager {
    worker_pool: WorkerPool,
}

impl JobManager {
    pub async fn init(config: JobManagerConfig) -> Self {
        let mut worker_pool = WorkerPool::init(config.max_worker_jobs, config.ssh_factory).await;
        worker_pool
            .populate()
            .await
            .expect("populate worker pool failed");

        Self { worker_pool }
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
        let ssh = worker.get_ssh_session();

        let cmd = format!(
            "cd $HOME/npm-follower/blob_idx_client && ./run.sh read {}",
            key
        );

        debug!("Running command:\n{}", cmd);

        let out = ssh.run_command(&cmd).await?;
        debug!("Output:\n{}", out);

        // parse into a ClientResponse
        let response: ClientResponse = serde_json::from_str(&out)
            .map_err(|_| JobError::ClientOutputNotParsable(out.clone()))?;

        match (response.message, response.error) {
            (Some(filepath), None) => Ok(filepath),
            (_, Some(e)) => Err(JobError::ClientError(e)),
            _ => Err(JobError::ClientOutputNotParsable(out)),
        }
    }
}
