#[macro_use]
extern crate diesel;

pub mod change_log;
pub mod internal_state;
pub mod download_queue;

mod schema;
mod custom_types;

#[cfg(test)]
mod testing;


use diesel::pg::PgConnection;
use diesel::prelude::*;
use dotenv::dotenv;
use std::env;


pub struct DbConnection {
    pub(crate) conn: PgConnection
}

pub fn connect() -> DbConnection {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    let conn = PgConnection::establish(&database_url)
        .expect(&format!("Error connecting to {}", database_url));
    DbConnection { conn }
}
