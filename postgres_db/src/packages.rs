use crate::custom_types::PackageMetadata;

use super::schema::packages;
use diesel::pg::upsert::excluded;
use diesel::Queryable;

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

// TODO [perf]: could consider memoizing this
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
// Also returns a bool that is true if the package already existed.
pub fn insert_package(conn: &DbConnection, package: Package) -> (i64, bool) {
    use super::schema::packages::dsl::*;

    // check if the package already exists
    // TODO [perf]: can we do this as one query?
    let already_existed = packages
        .filter(name.eq(&package.name))
        .first::<(i64, String, PackageMetadata, bool)>(&conn.conn)
        .optional()
        .expect("Error loading package")
        .is_some();

    let pkg_id = diesel::insert_into(packages)
        .values(&package)
        .on_conflict(name)
        .do_update()
        .set(metadata.eq(excluded(metadata)))
        .returning(id)
        .get_result::<i64>(&conn.conn)
        .expect("Error saving new package");
    
    (pkg_id, already_existed)
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
