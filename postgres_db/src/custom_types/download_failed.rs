use super::DownloadFailed;
use diesel::deserialize::{self, FromSql};
use diesel::pg::{Pg, PgValue};
use diesel::serialize::{self, IsNull, Output, ToSql};
use diesel::sql_types::Text;
use std::io::Write;

impl ToSql<Text, Pg> for DownloadFailed {
    fn to_sql(&self, out: &mut Output<Pg>) -> serialize::Result {
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
    fn from_sql(bytes: PgValue) -> deserialize::Result<Self> {
        let bytes = bytes.as_bytes();

        if bytes.starts_with(b"res") {
            let code = std::str::from_utf8(&bytes[3..]).unwrap().parse::<u16>()?;
            return Ok(DownloadFailed::Res(code));
        }

        match bytes {
            b"other" => Ok(DownloadFailed::Other),
            b"io" => Ok(DownloadFailed::Io),
            b"badly_formatted_url" => Ok(DownloadFailed::BadlyFormattedUrl),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}
