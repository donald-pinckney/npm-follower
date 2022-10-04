use crate::diesel::connection::SimpleConnection;
use crate::DbConnection;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use dotenv::dotenv;
use lazy_static::lazy_static;
use std::env;
use std::process::Command;
use std::sync::Mutex;


lazy_static! {
    static ref TEST_CONN_LOCK: Mutex<()> = Mutex::new(());
} 

fn setup_test_db() -> DbConnection {
    dotenv().ok();

    let database_url = env::var("TESTING_DATABASE_URL").expect("TESTING_DATABASE_URL must be set");

    // 1. Drop testing DB
    let _status = Command::new("dropdb")
        .arg("-p")
        .arg("5431")
        .arg("testing_npm_data")
        .status()
        .expect("failed to execute process");

    // 2. Create DB
    let status = Command::new("diesel")
        .arg("setup")
        .arg("--database-url")
        .arg(&database_url)
        .status()
        .expect("failed to execute process");
    assert!(status.success(), "Failed to run diesel setup --database-url {}", database_url);

    // 3. Connect
    let conn = PgConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url));
    DbConnection { conn }
}

pub fn using_test_db<F>(f: F)
where
    F: FnOnce(&DbConnection),
{
    let _locked = TEST_CONN_LOCK.lock().unwrap();

    let conn = setup_test_db();

    f(&conn);
}

// Used to create and automatically drop temporary tables, used for tests.
pub struct TempTable<'a> {
    pub connection: &'a DbConnection,
    pub table_name: &'static str,
}

impl<'a> TempTable<'a> {
    pub fn new(
        connection: &'a DbConnection,
        table_name: &'static str,
        columns: &'static str,
    ) -> Self {
        connection
            .conn
            .batch_execute(&format!(
                "DROP TABLE IF EXISTS {}; CREATE TABLE {} ({})",
                table_name, table_name, columns
            ))
            .unwrap();
        TempTable {
            connection,
            table_name,
        }
    }
}

impl<'a> Drop for TempTable<'a> {
    fn drop(&mut self) {
        self.connection
            .conn
            .execute(&format!("DROP TABLE {}", self.table_name))
            .unwrap();
    }
}
