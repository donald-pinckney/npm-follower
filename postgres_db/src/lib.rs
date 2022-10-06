#[macro_use]
extern crate diesel;

pub mod change_log;
#[allow(clippy::let_unit_value)] // for redis
pub mod dependencies;
pub mod download_queue;
pub mod download_tarball;
pub mod internal_state;
pub mod packages;
#[allow(unused_imports)]
mod schema;
pub mod versions;

pub mod custom_types;
pub mod download_metrics;

pub mod testing;

use diesel::pg::PgConnection;
use diesel::prelude::*;
use dotenv::dotenv;
use std::env;

pub struct DbConnection {
    pub(crate) conn: PgConnection,
}

#[cfg(not(test))]
pub fn connect() -> DbConnection {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let conn = PgConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url));
    DbConnection { conn }
}

impl DbConnection {
    pub fn run_psql_transaction<F>(&self, transaction: F) -> Result<(), diesel::result::Error>
    where
        F: FnOnce() -> Result<(), diesel::result::Error>,
    {
        self.conn
            .transaction::<_, diesel::result::Error, _>(transaction)
    }
}
