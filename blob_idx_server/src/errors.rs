use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum BlobError {
    AlreadyExists(String),
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
    /// Error from the client of a job.
    ClientError(ClientError),
    /// The output of the client wasn't parsable.
    ClientOutputNotParsable(String),
}

/// Errors that the client can return. This enum is serialized to JSON and sent to the server.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ClientError {
    /// Some download urls failed. The vector contains the urls that failed.
    DownloadFailed {
        urls: Vec<(String, std::num::NonZeroU16)>,
    },
    /// Some IO error occurred.
    IoError,
    /// Some reqwest error occurred.
    ReqwestError,
    /// Some error related to requesting the http server occurred.
    HttpServerError(std::num::NonZeroU16),
    /// Some blob error occurred.
    BlobError(BlobError),
    /// Some serde json error occurred.
    SerdeJsonError,
    // this stuff is for the compute job
    /// Duplicate tarball name
    DuplicateTarballName(String),
    /// The binary doesn't exist.
    BinaryDoesNotExist,
    /// The binary produced a bad stdout.
    InvalidOutput,
}

#[derive(Debug)]
pub enum HTTPError {
    Hyper(hyper::Error),
    Io(std::io::Error),
    Blob(BlobError),
    Job(JobError),
    Serde(serde_json::Error),
    InvalidBody(String), // missing a field in the body
    InvalidMethod(String),
    InvalidKey,
    InvalidPath(String),
}

impl std::fmt::Display for BlobError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlobError::AlreadyExists(key) => write!(f, "Blob already exists: {}", key),
            BlobError::CreateNotLocked => write!(f, "Blob create not locked"),
            BlobError::DuplicateKeys => write!(f, "Duplicate keys"),
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
            JobError::ClientError(e) => write!(f, "{}", e),
            JobError::ClientOutputNotParsable(s) => {
                write!(f, "Client output not parsable: {}", s)
            }
        }
    }
}

/// Display is implemented here using Serialize, so that the error can be sent to the server.
impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", serde_json::to_string(self).unwrap())
    }
}

impl std::fmt::Display for HTTPError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            HTTPError::Hyper(e) => write!(f, "Hyper error: {}", e),
            HTTPError::Io(e) => write!(f, "IO error: {}", e),
            HTTPError::Blob(e) => write!(
                f,
                "{}",
                serde_json::to_string(e).map_err(|_| { std::fmt::Error })?
            ),
            HTTPError::Job(JobError::ClientError(e)) => write!(f, "{}", e),
            HTTPError::Job(e) => write!(f, "Job error: {}", e),
            HTTPError::InvalidBody(e) => write!(f, "Invalid body: {}", e),
            HTTPError::InvalidMethod(e) => write!(f, "Invalid method: {}", e),
            HTTPError::InvalidPath(e) => write!(f, "Invalid path: {}", e),
            HTTPError::Serde(e) => write!(f, "Serde error: {}", e),
            HTTPError::InvalidKey => write!(f, "Invalid api key"),
        }
    }
}

impl From<openssh::Error> for JobError {
    fn from(e: openssh::Error) -> Self {
        JobError::SshError(e)
    }
}

impl From<std::io::Error> for ClientError {
    fn from(_: std::io::Error) -> Self {
        ClientError::IoError
    }
}

impl From<reqwest::Error> for ClientError {
    fn from(_: reqwest::Error) -> Self {
        ClientError::ReqwestError
    }
}

impl From<serde_json::Error> for ClientError {
    fn from(_: serde_json::Error) -> Self {
        ClientError::SerdeJsonError
    }
}

impl From<hyper::Error> for HTTPError {
    fn from(e: hyper::Error) -> Self {
        HTTPError::Hyper(e)
    }
}

impl From<std::io::Error> for HTTPError {
    fn from(e: std::io::Error) -> Self {
        HTTPError::Io(e)
    }
}

impl From<BlobError> for HTTPError {
    fn from(e: BlobError) -> Self {
        HTTPError::Blob(e)
    }
}

impl From<JobError> for HTTPError {
    fn from(e: JobError) -> Self {
        HTTPError::Job(e)
    }
}

impl From<serde_json::Error> for HTTPError {
    fn from(e: serde_json::Error) -> Self {
        HTTPError::Serde(e)
    }
}

impl std::error::Error for JobError {}
impl std::error::Error for BlobError {}
impl std::error::Error for ClientError {}
