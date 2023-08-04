use diesel::{
    pg::Pg,
    serialize::WriteTuple,
    sql_types::{BigInt, Date},
    types::{FromSql, Record, ToSql},
};

use super::{sql_types::DownloadCountSql, DownloadCount};

impl ToSql<DownloadCountSql, Pg> for DownloadCount {
    fn to_sql<W: std::io::Write>(
        &self,
        out: &mut diesel::serialize::Output<W, Pg>,
    ) -> diesel::serialize::Result {
        WriteTuple::<(Date, BigInt)>::write_tuple(&(self.date, self.count), out)
    }
}

impl FromSql<DownloadCountSql, Pg> for DownloadCount {
    fn from_sql(bytes: Option<&[u8]>) -> diesel::deserialize::Result<Self> {
        let (date, count) = FromSql::<Record<(Date, BigInt)>, Pg>::from_sql(bytes)?;
        Ok(DownloadCount { date, count })
    }
}
