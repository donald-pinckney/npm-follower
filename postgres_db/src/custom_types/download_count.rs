use diesel::{
    deserialize::{self, FromSql},
    pg::{Pg, PgValue},
    serialize::{self, Output, ToSql, WriteTuple},
    sql_types::{BigInt, Date, Record, Nullable},
};

use crate::schema::sql_types::DownloadCountStruct;

use super::DownloadCount;

impl ToSql<DownloadCountStruct, Pg> for DownloadCount {
    fn to_sql(&self, out: &mut Output<Pg>) -> serialize::Result {
        WriteTuple::<(Date, Nullable<BigInt>)>::write_tuple(&(self.date, self.count), out)
    }
}

impl FromSql<DownloadCountStruct, Pg> for DownloadCount {
    fn from_sql(bytes: PgValue) -> deserialize::Result<Self> {
        let (date, count) = FromSql::<Record<(Date, Nullable<BigInt>)>, Pg>::from_sql(bytes)?;
        Ok(DownloadCount { date, count })
    }
}
