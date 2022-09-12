use crate::custom_types::PackageMetadata;

use super::schema::packages;
use chrono::{DateTime, Utc};
use diesel::sql_types::BigInt;
use diesel::Queryable;

use super::schema;
use super::DbConnection;
use diesel::prelude::*;

#[derive(Queryable, Insertable, Debug)]
#[diesel(table_name = packages)]
pub struct Package {
    pub name: String,
    pub metadata: PackageMetadata,
    pub secret: bool,
}

impl Package {
    pub fn create(
        conn: &DbConnection,
        name: String,
        metadata: PackageMetadata,
        secret: bool,
    ) -> Package {
        // let id = get_package_sequence_number(conn);
        // increase_package_sequence_number(conn);

        Package {
            name,
            metadata,
            secret,
        }
    }
}

pub fn insert_package(conn: &DbConnection, package: Package) {
    use super::schema::packages::dsl::*;

    diesel::insert_into(packages)
        .values(&package)
        .execute(&conn.conn)
        .expect("Error saving new package");
}
