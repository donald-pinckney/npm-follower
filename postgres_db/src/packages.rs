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

pub fn query_pkg_id(conn: &DbConnection, pkg_name: &str) -> Option<i64> {
    use super::schema::packages::dsl::*;

    let result = packages
        .filter(name.eq(pkg_name))
        .first::<(i64, String, PackageMetadata, bool)>(&conn.conn)
        .optional()
        .expect("Error loading package");

    result.map(|x| x.0)
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

// Patches the missing latest version id of the package, for packages with Normal package metadata.
pub fn patch_latest_version_id(conn: &DbConnection, package_id: i64, version_id: i64) {
    use super::schema::packages::dsl::*;

    // get the package
    let pkg = packages
        .find(package_id)
        .get_result::<(i64, String, PackageMetadata, bool)>(&conn.conn)
        .expect("Error finding package");

    if let PackageMetadata::Normal {
        dist_tag_latest_version: _,
        created,
        modified,
        other_dist_tags,
    } = pkg.2
    {
        let new_package_metadata = PackageMetadata::Normal {
            dist_tag_latest_version: Some(version_id),
            created,
            modified,
            other_dist_tags,
        };

        diesel::update(packages.find(package_id))
            .set(metadata.eq(new_package_metadata))
            .execute(&conn.conn)
            .expect("Error updating package");
    }
}
