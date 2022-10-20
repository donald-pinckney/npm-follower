use crate::custom_types::{PrereleaseTag, Semver};
use crate::diesel::connection::SimpleConnection;
use crate::DbConnection;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use dotenv::dotenv;
use lazy_static::lazy_static;
use std::env;
use std::process::Command;
use std::sync::Mutex;
use url::Url;

lazy_static! {
    static ref TEST_CONN_LOCK: Mutex<()> = Mutex::new(());
}

fn drop_testing_db() {
    dotenv().ok();

    let database_url_str =
        env::var("TESTING_DATABASE_URL").expect("TESTING_DATABASE_URL must be set");
    let database_url = Url::parse(&database_url_str).unwrap();
    let database_host = database_url.host_str().unwrap();
    let database_port = database_url.port().unwrap_or(5432);
    let database_user = database_url.username();

    // Drop testing DB
    // TODO: check for the error case of concurrent access
    let mut command_to_run = Command::new("dropdb");
    command_to_run.arg("-h");
    command_to_run.arg(database_host);
    command_to_run.arg("-p");
    command_to_run.arg(database_port.to_string());
    if database_user != "" {
        command_to_run.arg("-U").arg(database_user);
    }
    command_to_run.arg("-w");
    command_to_run.arg("--if-exists");
    command_to_run.arg("testing_npm_data");
    let _status = command_to_run.status().expect("failed to execute process");
}

fn setup_test_db() -> DbConnection {
    dotenv().ok();

    let database_url = env::var("TESTING_DATABASE_URL").expect("TESTING_DATABASE_URL must be set");

    // 1. Drop testing DB
    drop_testing_db();

    let my_wd = env::current_dir().unwrap();
    let mut postgres_db_dir = my_wd.parent().unwrap().to_path_buf();
    postgres_db_dir.push("postgres_db");

    // 2. Create DB
    let status = Command::new("diesel")
        .arg("setup")
        .arg("--database-url")
        .arg(&database_url)
        .current_dir(postgres_db_dir)
        .status()
        .expect("failed to execute process");
    assert!(
        status.success(),
        "Failed to run diesel setup --database-url {}",
        database_url
    );

    // 3. Connect
    let conn = PgConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url));
    DbConnection { conn }
}

pub fn using_test_db<F, R>(f: F) -> R
where
    F: FnOnce(&DbConnection) -> R,
{
    let _locked = match TEST_CONN_LOCK.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };

    let res = {
        let conn = setup_test_db();
        f(&conn)
    };

    // drop_testing_db();

    res
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

#[cfg(test)]
impl Semver {
    pub fn new_testing_semver(n: i64) -> Semver {
        if n % 2 == 0 {
            Semver {
                major: n,
                minor: n + 1,
                bug: n,
                prerelease: vec![PrereleaseTag::String("alpha".into()), PrereleaseTag::Int(n)],
                build: vec!["stuff".into()],
            }
        } else {
            Semver {
                major: n + 2,
                minor: n + 1,
                bug: n,
                prerelease: vec![],
                build: vec![],
            }
        }
    }
}
