use diesel::{
    pg::{Pg, PgValue},
    serialize::{WriteTuple, ToSql, Output, self},
    sql_types::{BigInt, Date, Record}, deserialize::{FromSql, self},
};

use super::{sql_types::DownloadCountSql, DownloadCount};

impl ToSql<DownloadCountSql, Pg> for DownloadCount {
    fn to_sql(
        &self,
        out: &mut Output<Pg>,
    ) -> serialize::Result {
        WriteTuple::<(Date, BigInt)>::write_tuple(&(self.date, self.count), out)
    }
}

impl FromSql<DownloadCountSql, Pg> for DownloadCount {
    fn from_sql(bytes: PgValue) -> deserialize::Result<Self> {
        let (date, count) = FromSql::<Record<(Date, BigInt)>, Pg>::from_sql(bytes)?;
        Ok(DownloadCount { date, count })
    }
}
