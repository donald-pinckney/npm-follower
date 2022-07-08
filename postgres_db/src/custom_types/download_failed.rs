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
            DownlaodFailed::Res(code) => out.write_all(format!("res{}", code).as_bytes())?,
            DownlaodFailed::Other => out.write_all(b"other")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<Text, Pg> for DownlaodFailed {
    fn from_sql(bytes: Option<&[u8]>) -> deserialize::Result<Self> {
        if let Some(bytes) = bytes {
            if bytes.starts_with(b"res") {
                let code = std::str::from_utf8(&bytes[3..]).unwrap().parse::<u16>()?;
                return Ok(DownlaodFailed::Res(code));
            }
        }
        match not_none!(bytes) {
            b"other" => Ok(DownlaodFailed::Other),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}

impl From<DownloadError> for DownlaodFailed {
    fn from(e: DownloadError) -> Self {
        #[allow(clippy::collapsible_match)]
        match e {
            DownloadError::StatusNotOk(e) => DownlaodFailed::Res(e.into()),
            _ => DownlaodFailed::Other,
        }
    }
}
