#[derive(Debug)]
pub enum BlobError {
    AlreadyExists,
    CreateNotLocked,
    DuplicateKeys,
    DoesNotExist,
    NotWritten,
    WrongNode,
    LockExpired,
    ProhibitedKey,
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

impl std::fmt::Display for BlobError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlobError::AlreadyExists => write!(f, "Blob already exists"),
            BlobError::CreateNotLocked => write!(f, "Blob create not locked"),
            BlobError::DuplicateKeys => write!(f, "Blob duplicate keys"),
            BlobError::DoesNotExist => write!(f, "Blob does not exist"),
            BlobError::ProhibitedKey => write!(f, "Blob key is prohibited"),
            BlobError::WrongNode => write!(f, "Blob is locked by another node"),
            BlobError::LockExpired => write!(f, "Blob lock expired"),
            BlobError::NotWritten => write!(f, "Blob is not written"),
        }
    }
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
impl std::error::Error for BlobError {}
