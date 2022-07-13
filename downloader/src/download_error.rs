use postgres_db::custom_types::DownloadFailed;

#[derive(Debug)]
pub enum DownloadError {
    Request(reqwest::Error),
    StatusNotOk(reqwest::StatusCode),
    Io(std::io::Error),
    BadlyFormattedUrl,
}

impl std::error::Error for DownloadError {}

impl std::fmt::Display for DownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DownloadError::Request(e) => write!(f, "Request error: {}", e),
            DownloadError::StatusNotOk(e) => write!(f, "Status not OK: {}", e),
            DownloadError::Io(e) => write!(f, "IO error: {}", e),
            DownloadError::BadlyFormattedUrl => write!(f, "Badly formatted URL"),
        }
    }
}

impl From<reqwest::Error> for DownloadError {
    fn from(e: reqwest::Error) -> Self {
        DownloadError::Request(e)
    }
}

impl From<std::io::Error> for DownloadError {
    fn from(e: std::io::Error) -> Self {
        DownloadError::Io(e)
    }
}

impl From<DownloadFailed> for DownloadError {
    fn from(e: DownloadFailed) -> Self {
        match e {
            DownloadFailed::Res(code) => {
                DownloadError::StatusNotOk(reqwest::StatusCode::from_u16(code).unwrap())
            }
            DownloadFailed::Io => {
                DownloadError::Io(std::io::Error::new(std::io::ErrorKind::Other, "IO error"))
            }
            DownloadFailed::BadlyFormattedUrl => DownloadError::BadlyFormattedUrl,
            // NOTE: Other can really only be some kind of io error
            DownloadFailed::Other => DownloadError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Other error",
            )),
        }
    }
}

impl From<DownloadError> for DownloadFailed {
    fn from(e: DownloadError) -> Self {
        #[allow(clippy::collapsible_match)]
        match e {
            DownloadError::StatusNotOk(e) => DownloadFailed::Res(e.into()),
            DownloadError::Io(_) => DownloadFailed::Io,
            DownloadError::BadlyFormattedUrl => DownloadFailed::BadlyFormattedUrl,
            _ => DownloadFailed::Other,
        }
    }
}
