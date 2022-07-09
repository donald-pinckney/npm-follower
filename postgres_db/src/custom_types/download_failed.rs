use crate::download_tarball::DownloadError;

use super::DownloadFailed;
use diesel::deserialize;
use diesel::pg::Pg;
use diesel::serialize::{self, IsNull, Output};
use diesel::sql_types::Text;
use diesel::types::{FromSql, ToSql};
use std::io::Write;

impl ToSql<Text, Pg> for DownloadFailed {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        match self {
            DownloadFailed::Res(code) => out.write_all(format!("res{}", code).as_bytes())?,
            DownloadFailed::Io => out.write_all(b"io")?,
            DownloadFailed::BadlyFormattedUrl => out.write_all(b"badly_formatted_url")?,
            DownloadFailed::Other => out.write_all(b"other")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<Text, Pg> for DownloadFailed {
    fn from_sql(bytes: Option<&[u8]>) -> deserialize::Result<Self> {
        if let Some(bytes) = bytes {
            if bytes.starts_with(b"res") {
                let code = std::str::from_utf8(&bytes[3..]).unwrap().parse::<u16>()?;
                return Ok(DownloadFailed::Res(code));
            }
        }
        match not_none!(bytes) {
            b"other" => Ok(DownloadFailed::Other),
            b"io" => Ok(DownloadFailed::Io),
            b"badly_formatted_url" => Ok(DownloadFailed::BadlyFormattedUrl),
            _ => Err("Unrecognized enum variant".into()),
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
