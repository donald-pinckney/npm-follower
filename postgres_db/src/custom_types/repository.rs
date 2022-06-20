use diesel::pg::Pg;
use diesel::types::{ToSql, FromSql};
use diesel::deserialize;
use diesel::serialize::{self, Output, WriteTuple, IsNull};
use diesel::sql_types::{Record, Text};
use std::io::Write;
use super::sql_types::*;
use super::Repository;


// ---------- RepositorySql <----> Repository



impl ToSql<RepositorySql, Pg> for Repository {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        match self {
            Repository::Git(url) => {
                WriteTuple::<(RepositoryTypeSql, Text)>::write_tuple(&(RepositoryType::Git, url.as_str()), out)
            }
        }
    }
}

impl FromSql<RepositorySql, Pg> for Repository {
    fn from_sql(bytes: Option<&[u8]>) -> deserialize::Result<Self> {
        let (repo_type, url) = FromSql::<Record<(RepositoryTypeSql, Text)>, Pg>::from_sql(bytes)?;
        match repo_type {
            RepositoryType::Git => Ok(Repository::Git(url)),
        }
    }
}



#[derive(SqlType)]
#[postgres(type_name = "repository_type_enum")]
struct RepositoryTypeSql;

#[derive(Debug, PartialEq, FromSqlRow, AsExpression)]
#[sql_type = "RepositoryTypeSql"]
enum RepositoryType {
    Git,
}

impl ToSql<RepositoryTypeSql, Pg> for RepositoryType {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        match *self {
            RepositoryType::Git => out.write_all(b"git")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<RepositoryTypeSql, Pg> for RepositoryType {
    fn from_sql(bytes: Option<&[u8]>) -> deserialize::Result<Self> {
        match not_none!(bytes) {
            b"git" => Ok(RepositoryType::Git),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}



