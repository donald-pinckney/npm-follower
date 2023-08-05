use diesel::connection::SimpleConnection;
use diesel::helper_types::Limit;
use diesel::prelude::*;
use diesel::query_dsl::methods::{ExecuteDsl, LimitDsl};
use diesel::query_dsl::LoadQuery;
use diesel::PgConnection;

pub struct DbConnection {
    conn: PgConnection,
    dl_redis: Option<redis::Client>,
}

pub struct DbConnectionInTransaction<'conn> {
    conn: &'conn mut PgConnection,
    dl_redis: Option<&'conn mut redis::Client>,
}

#[cfg(not(test))]
impl DbConnection {
    pub fn connect() -> DbConnection {
        use dotenv::dotenv;
        use std::env;

        dotenv().expect("failed to load .env");

        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let conn = PgConnection::establish(&database_url)
            .unwrap_or_else(|_| panic!("Error connecting to {}", database_url));

        let dl_redis_url = env::var("DL_REDIS_URL").expect("DL_REDIS_URL must be set");
        let dl_redis = redis::Client::open(dl_redis_url).ok();

        DbConnection { conn, dl_redis }
    }
}

impl DbConnection {
    pub fn run_psql_transaction<F, R>(&mut self, transaction: F) -> Result<R, diesel::result::Error>
    where
        F: FnOnce(DbConnectionInTransaction) -> Result<(R, bool), diesel::result::Error>,
    {
        let mut res: Option<R> = None;

        let maybe_err = self
            .conn
            .transaction(|trans_conn| {
                let borrowed_self = DbConnectionInTransaction {
                    conn: trans_conn,
                    dl_redis: self.dl_redis.as_mut(),
                };
                let (result, should_commit) = transaction(borrowed_self)?;

                res = Some(result);
                if !should_commit {
                    return Err(diesel::result::Error::RollbackTransaction);
                }
                Ok(())
            })
            .err();

        if let Some(r) = res {
            Ok(r)
        } else {
            Err(maybe_err.unwrap())
        }
    }
}

pub trait QueryRunner {
    fn execute<Q>(&mut self, query: Q) -> QueryResult<usize>
    where
        Q: RunQueryDsl<PgConnection> + ExecuteDsl<PgConnection>;

    fn load<'query, Q, U>(&mut self, query: Q) -> QueryResult<Vec<U>>
    where
        Q: RunQueryDsl<PgConnection> + LoadQuery<'query, PgConnection, U>;

    // fn load_iter<'conn, 'query: 'conn, Q, U, B>(
    //     &'conn mut self,
    //     query: Q,
    // ) -> QueryResult<LoadIter<'conn, 'query, Q, PgConnection, U, B>>
    // where
    //     U: 'conn,
    //     Q: RunQueryDsl<PgConnection> + LoadQuery<'query, PgConnection, U, B> + 'conn;

    fn get_result<'query, Q, U>(&mut self, query: Q) -> QueryResult<U>
    where
        Q: RunQueryDsl<PgConnection> + LoadQuery<'query, PgConnection, U>;

    fn get_results<'query, Q, U>(&mut self, query: Q) -> QueryResult<Vec<U>>
    where
        Q: RunQueryDsl<PgConnection> + LoadQuery<'query, PgConnection, U>;

    fn first<'query, Q, U>(&mut self, query: Q) -> QueryResult<U>
    where
        Q: RunQueryDsl<PgConnection> + LimitDsl,
        Limit<Q>: LoadQuery<'query, PgConnection, U>;

    fn batch_execute(&mut self, query: &str) -> QueryResult<()>;

    fn get_dl_redis(&self) -> redis::Connection;
}

impl QueryRunner for DbConnection {
    fn execute<Q>(&mut self, query: Q) -> QueryResult<usize>
    where
        Q: RunQueryDsl<PgConnection> + ExecuteDsl<PgConnection>,
    {
        query.execute(&mut self.conn)
    }

    fn load<'query, Q, U>(&mut self, query: Q) -> QueryResult<Vec<U>>
    where
        Q: RunQueryDsl<PgConnection> + LoadQuery<'query, PgConnection, U>,
    {
        query.load(&mut self.conn)
    }

    // fn load_iter<'conn, 'query: 'conn, Q, U, B>(
    //     &'conn mut self,
    //     query: Q,
    // ) -> QueryResult<LoadIter<'conn, 'query, Q, PgConnection, U, B>>
    // where
    //     U: 'conn,
    //     Q: RunQueryDsl<PgConnection> + LoadQuery<'query, PgConnection, U, B> + 'conn,
    // {
    //     query.load_iter(&mut self.conn)
    // }

    fn get_result<'query, Q, U>(&mut self, query: Q) -> QueryResult<U>
    where
        Q: RunQueryDsl<PgConnection> + LoadQuery<'query, PgConnection, U>,
    {
        query.get_result(&mut self.conn)
    }

    fn get_results<'query, Q, U>(&mut self, query: Q) -> QueryResult<Vec<U>>
    where
        Q: RunQueryDsl<PgConnection> + LoadQuery<'query, PgConnection, U>,
    {
        query.get_results(&mut self.conn)
    }

    fn first<'query, Q, U>(&mut self, query: Q) -> QueryResult<U>
    where
        Q: RunQueryDsl<PgConnection> + LimitDsl,
        Limit<Q>: LoadQuery<'query, PgConnection, U>,
    {
        query.first(&mut self.conn)
    }

    fn batch_execute(&mut self, query: &str) -> QueryResult<()> {
        self.conn.batch_execute(query)
    }

    fn get_dl_redis(&self) -> redis::Connection {
        self.dl_redis
            .as_ref()
            .expect("DL Redis not configured")
            .get_connection()
            .expect("Failed to connect to DL redis")
    }
}

impl<'conn> QueryRunner for DbConnectionInTransaction<'conn> {
    fn execute<Q>(&mut self, query: Q) -> QueryResult<usize>
    where
        Q: RunQueryDsl<PgConnection> + ExecuteDsl<PgConnection>,
    {
        query.execute(self.conn)
    }

    fn load<'query, Q, U>(&mut self, query: Q) -> QueryResult<Vec<U>>
    where
        Q: RunQueryDsl<PgConnection> + LoadQuery<'query, PgConnection, U>,
    {
        query.load(self.conn)
    }

    // fn load_iter<'query: 'conn, Q, U, B>(
    //     &'conn mut self,
    //     query: Q,
    // ) -> QueryResult<LoadIter<'conn, 'query, Q, PgConnection, U, B>>
    // where
    //     U: 'conn,
    //     Q: RunQueryDsl<PgConnection> + LoadQuery<'query, PgConnection, U, B> + 'conn,
    // {
    //     query.load_iter(self.conn)
    // }

    fn get_result<'query, Q, U>(&mut self, query: Q) -> QueryResult<U>
    where
        Q: RunQueryDsl<PgConnection> + LoadQuery<'query, PgConnection, U>,
    {
        query.get_result(self.conn)
    }

    fn get_results<'query, Q, U>(&mut self, query: Q) -> QueryResult<Vec<U>>
    where
        Q: RunQueryDsl<PgConnection> + LoadQuery<'query, PgConnection, U>,
    {
        query.get_results(self.conn)
    }

    fn first<'query, Q, U>(&mut self, query: Q) -> QueryResult<U>
    where
        Q: RunQueryDsl<PgConnection> + LimitDsl,
        Limit<Q>: LoadQuery<'query, PgConnection, U>,
    {
        query.first(self.conn)
    }

    fn batch_execute(&mut self, query: &str) -> QueryResult<()> {
        self.conn.batch_execute(query)
    }

    fn get_dl_redis(&self) -> redis::Connection {
        self.dl_redis
            .as_ref()
            .expect("DL Redis not configured")
            .get_connection()
            .expect("Failed to connect to DL redis")
    }
}

pub mod testing {
    use super::DbConnection;
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
        dotenv().expect("failed to load .env");

        let database_url =
            env::var("TESTING_DATABASE_URL").expect("TESTING_DATABASE_URL must be set");

        let my_wd = env::current_dir().unwrap();
        let mut postgres_db_dir = my_wd.parent().unwrap().to_path_buf();
        postgres_db_dir.push("postgres_db");

        // 2. Create DB
        let status = Command::new("diesel")
            .args([
                "database",
                "reset",
                "--locked-schema",
                "--database-url",
                &database_url,
            ])
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
        DbConnection {
            conn,
            dl_redis: None,
        }
    }

    pub fn using_test_db<F, R>(f: F) -> R
    where
        F: FnOnce(&mut DbConnection) -> R,
    {
        let _locked = match TEST_CONN_LOCK.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };

        let mut conn = setup_test_db();
        f(&mut conn)
        // drop_testing_db();
    }
}
