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
