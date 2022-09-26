use crate::diesel::connection::SimpleConnection;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::result::DatabaseErrorKind;
use dotenv::dotenv;
use std::env;
use crate::DbConnection;

pub fn test_connect() -> DbConnection {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    let conn = PgConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url));

    match conn.execute("CREATE SCHEMA IF NOT EXISTS testing") {
        Ok(_) => (),
        Err(diesel::result::Error::DatabaseError(DatabaseErrorKind::UniqueViolation, err)) => {
            if err.message().contains("pg_namespace_nspname_index") {
                // the schema may fail due to race conditions with concurrently created schema.
                // but that's ok, the schema was still made, by someone else.
                
            } else {
                Err(diesel::result::Error::DatabaseError(DatabaseErrorKind::UniqueViolation, err)).unwrap()
            }
        },
        err => { err.unwrap();  }
    };
    
    conn.execute("SET search_path TO testing,public").unwrap();
    
    DbConnection { conn }
}

// Used to create and automatically drop temporary tables, used for tests.
pub struct TempTable<'a> {
    pub connection: &'a DbConnection,
    pub table_name: &'static str,
}

impl<'a> TempTable<'a> {
    pub fn new(connection: &'a DbConnection, table_name: &'static str, columns: &'static str) -> Self {
        connection.conn.batch_execute(&format!("DROP TABLE IF EXISTS {}; CREATE TABLE {} ({})", table_name, table_name, columns)).unwrap();
        TempTable {
            connection,
            table_name,
        }
    }
}

impl<'a> Drop for TempTable<'a> {
    fn drop(&mut self) {
        self.connection.conn
            .execute(&format!("DROP TABLE {}", self.table_name))
            .unwrap();
    }
}