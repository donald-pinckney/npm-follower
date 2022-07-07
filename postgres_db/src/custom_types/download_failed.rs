use crate::download_tarball::DownloadError;

use super::DownlaodFailed;
use diesel::deserialize;
use diesel::pg::Pg;
use diesel::serialize::{self, IsNull, Output};
use diesel::sql_types::Text;
use diesel::types::{FromSql, ToSql};
use std::io::Write;

impl ToSql<Text, Pg> for DownlaodFailed {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        match self {
            DownlaodFailed::Res404 => out.write_all(b"res404")?,
            DownlaodFailed::Other => out.write_all(b"other")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<Text, Pg> for DownlaodFailed {
    fn from_sql(bytes: Option<&[u8]>) -> deserialize::Result<Self> {
        match not_none!(bytes) {
            b"res404" => Ok(DownlaodFailed::Res404),
            b"other" => Ok(DownlaodFailed::Other),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}

impl From<DownloadError> for DownlaodFailed {
    fn from(e: DownloadError) -> Self {
        #[allow(clippy::collapsible_match)]
        match e {
            DownloadError::StatusNotOk(e) => match e {
                reqwest::StatusCode::NOT_FOUND => DownlaodFailed::Res404,
                _ => DownlaodFailed::Other,
            },
            _ => DownlaodFailed::Other,
        }
    }
}
