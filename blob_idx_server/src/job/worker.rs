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
    ssh::{Ssh, SshFactory, SshSessionFactory},
};

#[derive(Clone)]
pub(super) struct Worker {
    /// the discovery job id
    pub(super) job_id: u64,
    /// the status of the worker
    pub(super) status: Arc<WorkerStatus>,
    pub(super) avail_tx: Sender<u64>, // where u64 is the job_id
}

impl Worker {
    /// Checks if the worker is out of the queue or not.
    pub(crate) async fn is_running(&self, session: &dyn Ssh) -> Result<bool, JobError> {
        let out = session
            .run_command(&format!(
                "squeue -u $USER | grep {} | awk -F ' +' '{{print $6}}'",
                self.job_id
            ))
            .await?;
        Ok(out == "R")
    }

    /// Gets the node id of the worker.
    pub(crate) async fn get_node_id(&self, session: &dyn Ssh) -> Result<String, JobError> {
        let out = session
            .run_command(&format!(
                "squeue -u $USER | grep {} | awk -F ' +' '{{print $9}}'",
                self.job_id
            ))
            .await?;
        Ok(out)
    }

    /// Cancels the job of the worker on discovery.
    pub(crate) async fn cancel(&self, session: &dyn Ssh) -> Result<(), JobError> {
        session
            .run_command(&format!("scancel {}", self.job_id))
            .await?;
        Ok(())
    }

    /// Checks if the worker is able to ping `1.1.1.1`, if it can't, the network is down on
    /// the worker. Assumes the given worker is running.
    pub(crate) async fn is_network_up(&self) -> Result<bool, JobError> {
        match &*self.status {
            WorkerStatus::Running {
                started_at: _,
                node_id: _,
                ssh_session,
            } => {
                let out = ssh_session.run_command("curl -m 3 https://ip.me");
                match out.await {
                    Ok(_) => Ok(true),
                    Err(_) => Ok(false),
                }
            }
            _ => panic!("Worker should be running"),
        }
    }
}

pub(super) enum WorkerStatus {
    Queued,
    Running {
        started_at: chrono::DateTime<chrono::Utc>,
        ssh_session: Box<dyn Ssh>,
        node_id: String,
    },
}
