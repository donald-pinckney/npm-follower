use diesel::connection::SimpleConnection;
use diesel::helper_types::{Limit, LoadIter};
use diesel::prelude::*;
use diesel::query_dsl::methods::{ExecuteDsl, LimitDsl};
use diesel::query_dsl::LoadQuery;
use diesel::PgConnection;

pub struct DbConnection {
    conn: PgConnection,
}

pub struct DbConnectionBorrowed<'conn> {
    conn: &'conn mut PgConnection,
}

#[cfg(not(test))]
pub fn connect() -> DbConnection {
    use dotenv::dotenv;
    use std::env;

    dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let conn = PgConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url));
    DbConnection { conn }
}

impl DbConnection {
    pub fn run_psql_transaction<F, R>(&mut self, transaction: F) -> Result<R, diesel::result::Error>
    where
        F: FnOnce(DbConnectionBorrowed) -> Result<R, diesel::result::Error>,
    {
        self.conn.transaction(|trans_conn| {
            let borrowed_self = DbConnectionBorrowed { conn: trans_conn };
            transaction(borrowed_self)
        })
    }
}

impl DbConnection {
    pub fn execute<Q>(&mut self, query: Q) -> QueryResult<usize>
    where
        Q: RunQueryDsl<PgConnection> + ExecuteDsl<PgConnection>,
    {
        query.execute(&mut self.conn)
    }

    pub fn load<'query, Q, U>(&mut self, query: Q) -> QueryResult<Vec<U>>
    where
        Q: RunQueryDsl<PgConnection> + LoadQuery<'query, PgConnection, U>,
    {
        query.load(&mut self.conn)
    }

    pub fn load_iter<'conn, 'query: 'conn, Q, U, B>(
        &'conn mut self,
        query: Q,
    ) -> QueryResult<LoadIter<'conn, 'query, Q, PgConnection, U, B>>
    where
        U: 'conn,
        Q: RunQueryDsl<PgConnection> + LoadQuery<'query, PgConnection, U, B> + 'conn,
    {
        query.load_iter(&mut self.conn)
    }

    pub fn get_result<'query, Q, U>(&mut self, query: Q) -> QueryResult<U>
    where
        Q: RunQueryDsl<PgConnection> + LoadQuery<'query, PgConnection, U>,
    {
        query.get_result(&mut self.conn)
    }

    pub fn get_results<'query, Q, U>(&mut self, query: Q) -> QueryResult<Vec<U>>
    where
        Q: RunQueryDsl<PgConnection> + LoadQuery<'query, PgConnection, U>,
    {
        query.get_results(&mut self.conn)
    }

    pub fn first<'query, Q, U>(&mut self, query: Q) -> QueryResult<U>
    where
        Q: RunQueryDsl<PgConnection> + LimitDsl,
        Limit<Q>: LoadQuery<'query, PgConnection, U>,
    {
        query.first(&mut self.conn)
    }

    pub fn batch_execute(&mut self, query: &str) -> QueryResult<()> {
        self.conn.batch_execute(query)
    }
}

impl<'conn> DbConnectionBorrowed<'conn> {
    pub fn execute<Q>(&mut self, query: Q) -> QueryResult<usize>
    where
        Q: RunQueryDsl<PgConnection> + ExecuteDsl<PgConnection>,
    {
        query.execute(self.conn)
    }

    pub fn load<'query, Q, U>(&mut self, query: Q) -> QueryResult<Vec<U>>
    where
        Q: RunQueryDsl<PgConnection> + LoadQuery<'query, PgConnection, U>,
    {
        query.load(self.conn)
    }

    pub fn load_iter<'query: 'conn, Q, U, B>(
        &'conn mut self,
        query: Q,
    ) -> QueryResult<LoadIter<'conn, 'query, Q, PgConnection, U, B>>
    where
        U: 'conn,
        Q: RunQueryDsl<PgConnection> + LoadQuery<'query, PgConnection, U, B> + 'conn,
    {
        query.load_iter(self.conn)
    }

    pub fn get_result<'query, Q, U>(&mut self, query: Q) -> QueryResult<U>
    where
        Q: RunQueryDsl<PgConnection> + LoadQuery<'query, PgConnection, U>,
    {
        query.get_result(self.conn)
    }

    pub fn get_results<'query, Q, U>(&mut self, query: Q) -> QueryResult<Vec<U>>
    where
        Q: RunQueryDsl<PgConnection> + LoadQuery<'query, PgConnection, U>,
    {
        query.get_results(self.conn)
    }

    pub fn first<'query, Q, U>(&mut self, query: Q) -> QueryResult<U>
    where
        Q: RunQueryDsl<PgConnection> + LimitDsl,
        Limit<Q>: LoadQuery<'query, PgConnection, U>,
    {
        query.first(self.conn)
    }

    pub fn batch_execute(&mut self, query: &str) -> QueryResult<()> {
        self.conn.batch_execute(query)
    }
}

#[cfg(test)]
pub(crate) mod testing {
    use crate::connection::DbConnection;
    use crate::diesel::connection::SimpleConnection;
    use diesel::prelude::*;
    use diesel::{pg::PgConnection, sql_query};
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
        if !database_user.is_empty() {
            command_to_run.arg("-U").arg(database_user);
        }
        command_to_run.arg("-w");
        command_to_run.arg("--if-exists");
        command_to_run.arg("testing_npm_data");
        let _status = command_to_run.status().expect("failed to execute process");
    }

    fn setup_test_db() -> DbConnection {
        dotenv().ok();

        let database_url =
            env::var("TESTING_DATABASE_URL").expect("TESTING_DATABASE_URL must be set");

        // 1. Drop testing DB
        drop_testing_db();

        let my_wd = env::current_dir().unwrap();
        let mut postgres_db_dir = my_wd.parent().unwrap().to_path_buf();
        postgres_db_dir.push("postgres_db");

        // 2. Create DB
        let status = Command::new("diesel")
            .arg("setup")
            .arg("--locked-schema")
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
        F: FnOnce(&mut DbConnection) -> R,
    {
        let _locked = match TEST_CONN_LOCK.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };

        let res = {
            let mut conn = setup_test_db();
            f(&mut conn)
        };

        // drop_testing_db();

        res
    }
}
