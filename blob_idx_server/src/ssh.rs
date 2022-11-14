use tokio::sync::Mutex;

use crate::{debug, errors::JobError};

#[async_trait::async_trait]
pub trait Ssh: Send + Sync {
    async fn connect(ssh_user_host: &str) -> Result<Self, JobError>
    where
        Self: Sized;

    async fn connect_jumped(ssh_user_host: &str, jump_to: &str) -> Result<Self, JobError>
    where
        Self: Sized;

    async fn run_command(&self, cmd: &str) -> Result<String, JobError>;
}

#[async_trait::async_trait]
pub trait SshFactory: Send + Sync {
    async fn spawn(&self) -> Result<Box<dyn Ssh>, JobError>;

    async fn spawn_jumped(&self, jump_to: &str) -> Result<Box<dyn Ssh>, JobError>;
}

#[derive(Clone)]
pub struct SshSessionFactory {
    ssh_user_host: String,
}

impl SshSessionFactory {
    pub fn new(ssh_user_host: &str) -> Self {
        Self {
            ssh_user_host: ssh_user_host.to_string(),
        }
    }
}

#[async_trait::async_trait]
impl SshFactory for SshSessionFactory {
    async fn spawn(&self) -> Result<Box<dyn Ssh>, JobError> {
        let ssh_user_host = self.ssh_user_host.clone();
        let ssh = SshSession::connect(&ssh_user_host).await?;
        Ok(Box::new(ssh) as Box<dyn Ssh>)
    }

    async fn spawn_jumped(&self, jump_to: &str) -> Result<Box<dyn Ssh>, JobError> {
        let ssh_user_host = self.ssh_user_host.clone();
        let ssh = SshSession::connect_jumped(&ssh_user_host, jump_to).await?;
        Ok(Box::new(ssh) as Box<dyn Ssh>)
    }
}

pub struct SshSession {
    session: Mutex<openssh::Session>,
    ssh_user_host: String,
}

#[async_trait::async_trait]
impl Ssh for SshSession {
    async fn connect(ssh_user_host: &str) -> Result<Self, JobError> {
        let session = openssh::SessionBuilder::default()
            .known_hosts_check(openssh::KnownHosts::Accept)
            .server_alive_interval(std::time::Duration::from_secs(10))
            .connect_mux(ssh_user_host)
            .await?;
        Ok(Self {
            session: Mutex::new(session),
            ssh_user_host: ssh_user_host.to_string(),
        })
    }

    async fn connect_jumped(ssh_user_host: &str, jump_to: &str) -> Result<Self, JobError> {
        let split: Vec<&str> = ssh_user_host.split('@').collect();
        let jump_to = if jump_to.contains('@') {
            jump_to.to_string()
        } else {
            format!("{}@{}", split[0], jump_to)
        };
        let session = openssh::SessionBuilder::default()
            .known_hosts_check(openssh::KnownHosts::Accept)
            .server_alive_interval(std::time::Duration::from_secs(10))
            .jump_hosts(vec![ssh_user_host])
            .connect_mux(jump_to)
            .await?;
        Ok(Self {
            session: Mutex::new(session),
            ssh_user_host: ssh_user_host.to_string(),
        })
    }

    /// Runs the given command on the remote host. If the command fails due to a connection error,
    /// it will try to reconnect and run the command again.
    /// This will return JobError::CommandNonZero if the command exits with a non-zero exit code.
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
                            output: String::from_utf8(output.stderr)
                                .expect("invalid utf8")
                                .trim_end_matches('\n')
                                .to_string(),
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
