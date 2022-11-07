use tokio::sync::Mutex;

use crate::{debug, errors::JobError};

#[async_trait::async_trait]
pub trait Ssh {
    async fn connect(ssh_user_host: &str) -> Result<Self, JobError>
    where
        Self: Sized;

    async fn run_command(&self, cmd: &str) -> Result<String, JobError>;
}

pub struct SshSession {
    session: Mutex<openssh::Session>,
    ssh_user_host: String,
}

#[async_trait::async_trait]
impl Ssh for SshSession {
    async fn connect(ssh_user_host: &str) -> Result<Self, JobError> {
        let session = openssh::Session::connect(ssh_user_host, openssh::KnownHosts::Accept).await?;
        Ok(Self {
            session: Mutex::new(session),
            ssh_user_host: ssh_user_host.to_string(),
        })
    }

    /// Runs the given command on the remote host. If the command fails due to a connection error,
    /// it will try to reconnect and run the command again.
    async fn run_command(&self, cmd: &str) -> Result<String, JobError> {
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
