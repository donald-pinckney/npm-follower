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
            DownlaodFailed::Res404 => out.write_all(b"Res404")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<Text, Pg> for DownlaodFailed {
    fn from_sql(bytes: Option<&[u8]>) -> deserialize::Result<Self> {
        match not_none!(bytes) {
            b"string" => Ok(DownlaodFailed::Res404),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}
