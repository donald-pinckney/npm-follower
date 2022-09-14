use std::collections::HashMap;

use crate::custom_types::PackageMetadata;
use crate::custom_types::Semver;
use crate::versions::Version;

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
    pub fn create(name: String, metadata: PackageMetadata, secret: bool) -> Package {
        Package {
            name,
            metadata,
            secret,
        }
    }
}

pub fn package_transaction(
    conn: &DbConnection,
    package: Package,
) {
    conn.conn
        .transaction::<(), _, _>(|| {
            let pkg_id = insert_package(conn, package);
            Ok::<(), diesel::result::Error>(())
        })
        .expect("Failed to insert package");
}

// Inserts package into the database and returns the id of the row that was just inserted.
pub fn insert_package(conn: &DbConnection, package: Package) -> i64 {
    use super::schema::packages::dsl::*;

    diesel::insert_into(packages)
        .values(&package)
        .get_result::<(i64, String, PackageMetadata, bool)>(&conn.conn)
        .expect("Error saving new package")
        .0
}
