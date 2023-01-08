use std::future::Future;

use async_trait::async_trait;
use bb8::Pool;
use bb8_diesel::{DieselConnection, DieselConnectionManager};
use diesel::connection::SimpleConnection;
use diesel::helper_types::Limit;
use diesel::prelude::*;
use diesel::query_dsl::methods::{ExecuteDsl, LimitDsl};
use diesel::query_dsl::LoadQuery;
use diesel::PgConnection;

#[derive(Clone)]
pub struct DbConnection {
    pool: Pool<DieselConnectionManager<PgConnection>>,
}

// pub struct DbConnectionInTransaction<'conn> {
//     conn: &'conn mut DieselConnection<PgConnection>,
// }

// #[cfg(not(test))]
impl DbConnection {
    pub async fn connect() -> DbConnection {
        use dotenv::dotenv;
        use std::env;

        dotenv().expect("failed to load .env");

        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let mgr = bb8_diesel::DieselConnectionManager::<PgConnection>::new("localhost:1234");
        let pool = bb8::Pool::builder()
            .build(mgr)
            .await
            .unwrap_or_else(|_| panic!("Error connecting to {}", database_url));

        DbConnection { pool }
    }
}

// impl DbConnection {
//     pub async fn run_psql_transaction<F, R, Fut>(
//         &mut self,
//         transaction: F,
//     ) -> Result<R, diesel::result::Error>
//     where
//         F: FnOnce(DbConnectionInTransaction) -> Fut,
//         Fut: Future<Output = Result<(R, bool), diesel::result::Error>>,
//     {
//         let mut res: Option<R> = None;

//         let mut conn = self.pool.get().await.unwrap();

//         let maybe_err = conn
//             .transaction(|trans_conn| {
//                 let borrowed_self = DbConnectionInTransaction { conn: trans_conn };
//                 let (result, should_commit) = transaction(borrowed_self).await?;

//                 res = Some(result);
//                 if !should_commit {
//                     return Err(diesel::result::Error::RollbackTransaction);
//                 }
//                 Ok(())
//             })
//             .err();

//         if let Some(r) = res {
//             Ok(r)
//         } else {
//             Err(maybe_err.unwrap())
//         }
//     }
// }

#[async_trait]
pub trait QueryRunner {
    async fn execute<Q>(&self, query: Q) -> QueryResult<usize>
    where
        Q: RunQueryDsl<PgConnection> + ExecuteDsl<PgConnection> + Send;

    async fn load<'query, Q, U>(&self, query: Q) -> QueryResult<Vec<U>>
    where
        Q: RunQueryDsl<PgConnection> + LoadQuery<'query, PgConnection, U> + Send;

    // fn load_iter<'conn, 'query: 'conn, Q, U, B>(
    //     &'conn mut self,
    //     query: Q,
    // ) -> QueryResult<LoadIter<'conn, 'query, Q, PgConnection, U, B>>
    // where
    //     U: 'conn,
    //     Q: RunQueryDsl<PgConnection> + LoadQuery<'query, PgConnection, U, B> + 'conn;

    async fn get_result<'query, Q, U>(&self, query: Q) -> QueryResult<U>
    where
        Q: RunQueryDsl<PgConnection> + LoadQuery<'query, PgConnection, U> + Send;

    async fn get_results<'query, Q, U>(&self, query: Q) -> QueryResult<Vec<U>>
    where
        Q: RunQueryDsl<PgConnection> + LoadQuery<'query, PgConnection, U> + Send;

    async fn first<'query, Q, U>(&self, query: Q) -> QueryResult<U>
    where
        Q: RunQueryDsl<PgConnection> + LimitDsl + Send,
        Limit<Q>: LoadQuery<'query, PgConnection, U>;

    async fn batch_execute(&self, query: &str) -> QueryResult<()>;
}

#[async_trait]
impl QueryRunner for DbConnection {
    async fn execute<Q>(&self, query: Q) -> QueryResult<usize>
    where
        Q: RunQueryDsl<PgConnection> + ExecuteDsl<PgConnection> + Send,
    {
        let mut conn = self.pool.get().await.unwrap();
        query.execute(&mut conn)
    }

    async fn load<'query, Q, U>(&self, query: Q) -> QueryResult<Vec<U>>
    where
        Q: RunQueryDsl<PgConnection> + LoadQuery<'query, PgConnection, U> + Send,
    {
        let mut conn = self.pool.get().await.unwrap();
        query.load(&mut conn)
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

    async fn get_result<'query, Q, U>(&self, query: Q) -> QueryResult<U>
    where
        Q: RunQueryDsl<PgConnection> + LoadQuery<'query, PgConnection, U> + Send,
    {
        let mut conn = self.pool.get().await.unwrap();
        query.get_result(&mut conn)
    }

    async fn get_results<'query, Q, U>(&self, query: Q) -> QueryResult<Vec<U>>
    where
        Q: RunQueryDsl<PgConnection> + LoadQuery<'query, PgConnection, U> + Send,
    {
        let mut conn = self.pool.get().await.unwrap();
        query.get_results(&mut conn)
    }

    async fn first<'query, Q, U>(&self, query: Q) -> QueryResult<U>
    where
        Q: RunQueryDsl<PgConnection> + LimitDsl + Send,
        Limit<Q>: LoadQuery<'query, PgConnection, U>,
    {
        let mut conn = self.pool.get().await.unwrap();
        query.first(&mut conn)
    }

    async fn batch_execute(&self, query: &str) -> QueryResult<()> {
        let mut conn = self.pool.get().await.unwrap();
        conn.batch_execute(query)
    }
}

// #[async_trait]
// impl<'conn> QueryRunner for DbConnectionInTransaction<'conn> {
//     async fn execute<Q>(&mut self, query: Q) -> QueryResult<usize>
//     where
//         Q: RunQueryDsl<PgConnection> + ExecuteDsl<PgConnection> + Send,
//     {
//         query.execute(self.conn)
//     }

//     async fn load<'query, Q, U>(&mut self, query: Q) -> QueryResult<Vec<U>>
//     where
//         Q: RunQueryDsl<PgConnection> + LoadQuery<'query, PgConnection, U> + Send,
//     {
//         query.load(self.conn)
//     }

//     // fn load_iter<'query: 'conn, Q, U, B>(
//     //     &'conn mut self,
//     //     query: Q,
//     // ) -> QueryResult<LoadIter<'conn, 'query, Q, PgConnection, U, B>>
//     // where
//     //     U: 'conn,
//     //     Q: RunQueryDsl<PgConnection> + LoadQuery<'query, PgConnection, U, B> + 'conn,
//     // {
//     //     query.load_iter(self.conn)
//     // }

//     async fn get_result<'query, Q, U>(&mut self, query: Q) -> QueryResult<U>
//     where
//         Q: RunQueryDsl<PgConnection> + LoadQuery<'query, PgConnection, U> + Send,
//     {
//         query.get_result(self.conn)
//     }

//     async fn get_results<'query, Q, U>(&mut self, query: Q) -> QueryResult<Vec<U>>
//     where
//         Q: RunQueryDsl<PgConnection> + LoadQuery<'query, PgConnection, U> + Send,
//     {
//         query.get_results(self.conn)
//     }

//     async fn first<'query, Q, U>(&mut self, query: Q) -> QueryResult<U>
//     where
//         Q: RunQueryDsl<PgConnection> + LimitDsl + Send,
//         Limit<Q>: LoadQuery<'query, PgConnection, U>,
//     {
//         query.first(self.conn)
//     }

//     async fn batch_execute(&mut self, query: &str) -> QueryResult<()> {
//         self.conn.batch_execute(query)
//     }
// }
