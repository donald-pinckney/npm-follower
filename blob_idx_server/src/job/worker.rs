use std::sync::Arc;

use tokio::sync::mpsc::Sender;

use crate::{errors::JobError, ssh::Ssh};

/// parse time from "hour:min:sec", but could just be "min:sec"
pub(super) fn parse_time(time: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    let time_now = chrono::Utc::now();
    // parse time from "hour:min:sec", but could just be "min:sec"
    let job_time = if time.matches(':').count() == 3 {
        let mut parts = time.split(':');
        let hour = parts.next().unwrap().parse::<i64>().ok()?;
        let min = parts.next().unwrap().parse::<i64>().ok()?;
        let sec = parts.next().unwrap().parse::<i64>().ok()?;
        // get current time and subtract the time from the job
        time_now
            - chrono::Duration::hours(hour)
            - chrono::Duration::minutes(min)
            - chrono::Duration::seconds(sec)
    } else {
        let mut parts = time.split(':');
        let min = parts.next().unwrap().parse::<i64>().ok()?;
        let sec = parts.next().unwrap().parse::<i64>().ok()?;
        // get current time and subtract the time from the job
        time_now - chrono::Duration::minutes(min) - chrono::Duration::seconds(sec)
    };
    Some(job_time)
}

#[derive(Clone)]
pub(super) struct Worker {
    /// the discovery job id
    pub(super) job_id: u64,
    /// the status of the worker
    pub(super) status: Arc<WorkerStatus>,
    pub(super) avail_tx: Sender<u64>, // where u64 is the job_id
}

impl Worker {
    /// Checks if the worker is out of the queue or not. Returns the time the worker was
    /// started at if it is running.
    pub(crate) async fn started_running_at(
        &self,
        session: &dyn Ssh,
    ) -> Result<Option<chrono::DateTime<chrono::Utc>>, JobError> {
        let out = session
            .run_command(&format!(
                "squeue -u $USER | grep {} | awk -F ' +' '{{print $6, $7}}'",
                self.job_id
            ))
            .await?;
        if out.is_empty() || out.matches(' ').count() != 1 {
            Ok(None)
        } else {
            let mut parts = out.split(' ');
            let status = parts.next().unwrap();
            let time = parts.next().unwrap();
            let job_time = match parse_time(time) {
                Some(j) => j,
                None => return Ok(None),
            };
            if status == "R" {
                Ok(Some(job_time))
            } else {
                Ok(None)
            }
        }
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
    /// the worker.
    ///
    /// # Panics
    /// If the worker is not running.
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

    /// Gets a reference to the ssh session of the worker.
    ///
    /// # Panics
    /// If the worker is not running.
    pub(crate) fn get_ssh_session(&self) -> &dyn Ssh {
        match &*self.status {
            WorkerStatus::Running {
                started_at: _,
                node_id: _,
                ssh_session,
            } => ssh_session.as_ref(),
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
