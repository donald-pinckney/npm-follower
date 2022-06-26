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



// Unit tests
#[cfg(test)]
mod tests {
    use diesel::prelude::*;
    use diesel::RunQueryDsl;
    use crate::custom_types::Repository;
    use crate::testing;

    table! {
        use diesel::sql_types::*;
        use crate::custom_types::sql_type_names::Repository_struct;

        test_repository_to_sql {
            id -> Integer,
            repo -> Nullable<Repository_struct>,
        }
    }

    #[derive(Insertable, Queryable, Identifiable, Debug, PartialEq)]
    #[table_name = "test_repository_to_sql"]
    struct TestRepositoryToSql {
        id: i32,
        repo: Option<Repository>,
    }

    #[test]
    fn test_repository_to_sql_fn() {
        use self::test_repository_to_sql::dsl::*;

        let data = vec![
            TestRepositoryToSql {
                id: 1,
                repo: Some(Repository::Git("here is a git url".into()))
            },
            TestRepositoryToSql {
                id: 2,
                repo: None
            },
            TestRepositoryToSql {
                id: 3,
                repo: Some(Repository::Git("another url".into()))
            }
        ];

        let conn = testing::test_connect();
        let _temp_table = testing::TempTable::new(&conn, "test_repository_to_sql", "id SERIAL PRIMARY KEY, repo repository");

        let inserted = diesel::insert_into(test_repository_to_sql).values(&data).get_results(&conn.conn).unwrap();
        assert_eq!(data, inserted);

        let filter_all = test_repository_to_sql
            .filter(id.ge(1))
            .load(&conn.conn)
            .unwrap();
        assert_eq!(data, filter_all);


        let filter_eq_data = vec![
            TestRepositoryToSql {
                id: 1,
                repo: Some(Repository::Git("here is a git url".into()))
            }
        ];
        let filter_eq = test_repository_to_sql
            .filter(repo.eq(Some(Repository::Git("here is a git url".into()))))
            .load(&conn.conn)
            .unwrap();
        assert_eq!(filter_eq_data, filter_eq);
    }
}

