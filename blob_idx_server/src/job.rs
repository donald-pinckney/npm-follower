use std::{collections::HashMap, sync::Arc};

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
    pub message: Option<serde_json::Value>,
    pub error: Option<ClientError>,
}

/// The result for a single tarball chunk computed by a worker.
#[derive(Debug, Serialize, Deserialize)]
pub struct ChunkResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Configuration to initialize a job manager.
pub struct JobManagerConfig {
    /// The ssh factory to use to create ssh sessions.
    pub ssh_factory: Box<dyn SshFactory>,
    /// The maximum amount of worker jobs that can be running at the same time.
    pub max_worker_jobs: usize,
}

pub struct JobManager {
    xfer_pool: WorkerPool,
    compute_pool: WorkerPool,
}

impl JobManager {
    pub async fn init(config: JobManagerConfig) -> Self {
        // distribute config.max_worker_jobs between the two pools,
        // where is the number is odd, the xfer pool gets the extra job.
        let (xfer_workers, compute_workers) = if config.max_worker_jobs % 2 == 0 {
            (config.max_worker_jobs / 2, config.max_worker_jobs / 2)
        } else {
            (config.max_worker_jobs / 2 + 1, config.max_worker_jobs / 2)
        };
        let arc_ssh_factory = Arc::new(config.ssh_factory);
        debug!(
            "Initializing job manager with {} xfer workers and {} compute workers",
            xfer_workers, compute_workers
        );
        let mut xfer_pool =
            WorkerPool::init(xfer_workers, "wp_xfer", arc_ssh_factory.clone()).await;
        xfer_pool
            .populate()
            .await
            .expect("populate worker pool failed");

        let mut compute_pool = WorkerPool::init(compute_workers, "wp_comp", arc_ssh_factory).await;
        compute_pool
            .populate()
            .await
            .expect("populate worker pool failed");

        Self {
            xfer_pool,
            compute_pool,
        }
    }

    /// Submits a download and write job to the discovery cluster.
    pub async fn submit_download_job(&self, urls: Vec<String>) -> Result<(), JobError> {
        debug!("Submitting download job with {} urls", urls.len());
        let worker = self.xfer_pool.get_worker().await?;

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
        let worker = self.xfer_pool.get_worker().await?;
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
            (Some(filepath), None) => Ok(filepath.as_str().unwrap().to_string()),
            (_, Some(e)) => Err(JobError::ClientError(e)),
            _ => Err(JobError::ClientOutputNotParsable(out)),
        }
    }

    /// Submits a compute job to the discovery cluster. Returns stdout for each tarball computed.
    /// Takes in the full path to the binary to run and a chunk of tarballs, where for each
    /// outer element, we have a list of tarballs to compute on a single node. We map
    /// all chunks to different nodes. We return a hashmap of [tarball_name] -> [ChunkResult].
    ///
    /// We return errors in cases:
    /// - tarball does not exist
    /// - the binary does not exist
    /// - there is a duplicate tarball name across the chunks
    pub async fn submit_compute(
        &self,
        binary: String,
        tarball_chunks: Vec<Vec<String>>,
    ) -> Result<HashMap<String, ChunkResult>, JobError> {
        let mut tarball_to_chunk = HashMap::new();
        for chunk in &tarball_chunks {
            debug!("Submitting compute job with {} tarballs", chunk.len());
        }
        Ok(tarball_to_chunk)
    }
}
