use super::schema;
use super::schema::packages;
use crate::connection::DbConnection;
use crate::connection::QueryRunner;
use crate::custom_types::DiffTypeEnum;
use crate::custom_types::PackageStateTimePoint;
use crate::custom_types::PackageStateType;
use crate::custom_types::Semver;
use crate::packument::PackageOnlyPackument;
use crate::packument::VersionOnlyPackument;
use chrono::DateTime;
use chrono::Utc;
use deepsize::DeepSizeOf;
use diesel::insert_into;
use diesel::prelude::*;
use diesel::Insertable;
use diesel::Queryable;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

#[derive(Queryable, Debug, DeepSizeOf)]
#[diesel(table_name = packages)]
pub struct Package {
    pub id: i64,
    pub name: String,
    pub current_package_state_type: PackageStateType,
    pub package_state_history: Vec<PackageStateTimePoint>,
    pub dist_tag_latest_version: Option<i64>,
    pub created: Option<DateTime<Utc>>,
    pub modified: Option<DateTime<Utc>>,
    pub other_dist_tags: Option<Value>,
    pub other_time_data: Option<Value>,
    pub unpublished_data: Option<Value>,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = packages)]
pub struct NewPackage {
    pub name: String,
    pub current_package_state_type: PackageStateType,
    pub package_state_history: Vec<PackageStateTimePoint>,
    pub dist_tag_latest_version: Option<i64>,
    pub created: Option<DateTime<Utc>>,
    pub modified: Option<DateTime<Utc>>,
    pub other_dist_tags: Option<Value>,
    pub other_time_data: Option<Value>,
    pub unpublished_data: Option<Value>,
}

#[derive(Debug, AsChangeset)]
#[diesel(table_name = packages)]
pub struct PackageUpdate {
    name: Option<String>, // but we ALWAYS set this to None
    current_package_state_type: Option<PackageStateType>,
    package_state_history: Option<Vec<PackageStateTimePoint>>,
    dist_tag_latest_version: Option<Option<i64>>,
    created: Option<Option<DateTime<Utc>>>,
    modified: Option<Option<DateTime<Utc>>>,
    other_dist_tags: Option<Option<Value>>,
    other_time_data: Option<Option<Value>>,
    unpublished_data: Option<Option<Value>>,
}

impl Package {
    pub fn diff(&self, new: NewPackage) -> PackageUpdate {
        let mut update = PackageUpdate {
            name: None,
            current_package_state_type: None,
            package_state_history: None,
            dist_tag_latest_version: None,
            created: None,
            modified: None,
            other_dist_tags: None,
            other_time_data: None,
            unpublished_data: None,
        };

        if self.current_package_state_type != new.current_package_state_type {
            update.current_package_state_type = Some(new.current_package_state_type);
        }

        if self.package_state_history != new.package_state_history {
            update.package_state_history = Some(new.package_state_history);
        }

        if self.dist_tag_latest_version != new.dist_tag_latest_version {
            update.dist_tag_latest_version = Some(new.dist_tag_latest_version);
        }

        if self.created != new.created {
            update.created = Some(new.created);
        }

        if self.modified != new.modified {
            update.modified = Some(new.modified);
        }

        if self.other_dist_tags != new.other_dist_tags {
            update.other_dist_tags = Some(new.other_dist_tags);
        }

        if self.other_time_data != new.other_time_data {
            update.other_time_data = Some(new.other_time_data);
        }

        if self.unpublished_data != new.unpublished_data {
            update.unpublished_data = Some(new.unpublished_data);
        }

        update
    }

    pub fn apply_diff(&mut self, diff: &PackageUpdate) {
        assert!(diff.name.is_none());

        if let Some(current_package_state_type) = &diff.current_package_state_type {
            self.current_package_state_type = current_package_state_type.clone();
        }

        if let Some(package_state_history) = &diff.package_state_history {
            self.package_state_history = package_state_history.clone();
        }

        if let Some(dist_tag_latest_version) = diff.dist_tag_latest_version {
            self.dist_tag_latest_version = dist_tag_latest_version;
        }

        if let Some(created) = diff.created {
            self.created = created;
        }

        if let Some(modified) = diff.modified {
            self.modified = modified;
        }

        if let Some(other_dist_tags) = &diff.other_dist_tags {
            self.other_dist_tags = other_dist_tags.clone();
        }

        if let Some(other_time_data) = &diff.other_time_data {
            self.other_time_data = other_time_data.clone();
        }

        if let Some(unpublished_data) = &diff.unpublished_data {
            self.unpublished_data = unpublished_data.clone();
        }
    }
}

pub fn insert_new_package<R: QueryRunner>(conn: &mut R, package: NewPackage) -> Package {
    let query = insert_into(packages::table)
        .values(&package)
        .returning(packages::id);
    let new_id = conn.get_result(query).expect("Error inserting new package");

    Package {
        id: new_id,
        name: package.name,
        current_package_state_type: package.current_package_state_type,
        package_state_history: package.package_state_history,
        dist_tag_latest_version: package.dist_tag_latest_version,
        created: package.created,
        modified: package.modified,
        other_dist_tags: package.other_dist_tags,
        other_time_data: package.other_time_data,
        unpublished_data: package.unpublished_data,
    }
}

pub fn update_package<R: QueryRunner>(conn: &mut R, package_id: i64, update: PackageUpdate) {
    let query = diesel::update(packages::table.filter(packages::id.eq(package_id))).set(update);
    conn.execute(query).expect("Error updating package");
}

pub fn get_package<R: QueryRunner>(conn: &mut R, package_id: i64) -> Package {
    let query = packages::table.filter(packages::id.eq(package_id));
    conn.get_result(query).expect("Error getting package")
}

pub fn get_package_by_name<R: QueryRunner>(conn: &mut R, package_name: &str) -> Package {
    let query = packages::table.filter(packages::name.eq(package_name));
    conn.get_result(query)
        .expect("Error getting package by name")
}

pub fn maybe_get_package_id_by_name<R: QueryRunner>(
    conn: &mut R,
    package_name: &str,
) -> Option<i64> {
    let query = packages::table
        .filter(packages::name.eq(package_name))
        .select(packages::id);
    conn.first(query)
        .optional()
        .expect("Error getting package id")
}

// #[derive(Queryable, Insertable, Debug)]
// #[diesel(table_name = packages)]
// pub struct Package {
//     pub name: String,
//     pub metadata: PackageMetadata,
//     pub secret: bool,
// }

// impl Package {
//     pub fn create(name: String, metadata: PackageMetadata, secret: bool) -> Package {
//         Package {
//             name,
//             metadata,
//             secret,
//         }
//     }
// }

// // TODO [perf]: could consider memoizing this
// pub fn query_pkg_id(conn: &mut DbConnection, pkg_name: &str) -> Option<i64> {
//     use super::schema::packages::dsl::*;

//     let result = packages
//         .filter(name.eq(pkg_name))
//         .first::<(i64, String, PackageMetadata, bool)>(&conn.conn)
//         .optional()
//         .expect("Error loading package");

//     result.map(|x| x.0)
// }

// // Inserts package into the database and returns the id of the row that was just inserted.
// // Also returns a bool that is true if the package already existed.
// pub fn insert_package(conn: &mut DbConnection, package: Package) -> (i64, bool) {
//     use super::schema::packages::dsl::*;

//     // check if the package already exists
//     // TODO [perf]: can we do this as one query?
//     let already_existed = packages
//         .filter(name.eq(&package.name))
//         .first::<(i64, String, PackageMetadata, bool)>(&conn.conn)
//         .optional()
//         .expect("Error loading package")
//         .is_some();

//     let pkg_id = diesel::insert_into(packages)
//         .values(&package)
//         .on_conflict(name)
//         .do_update()
//         .set(metadata.eq(excluded(metadata)))
//         .returning(id)
//         .get_result::<i64>(&conn.conn)
//         .expect("Error saving new package");

//     (pkg_id, already_existed)
// }

// // Patches the missing latest version id of the package, for packages with Normal package metadata.
// pub fn patch_latest_version_id(conn: &mut DbConnection, package_id: i64, version_id: i64) {
//     use super::schema::packages::dsl::*;

//     // get the package
//     let pkg = packages
//         .find(package_id)
//         .get_result::<(i64, String, PackageMetadata, bool)>(&conn.conn)
//         .expect("Error finding package");

//     if let PackageMetadata::Normal {
//         dist_tag_latest_version: _,
//         created,
//         modified,
//         other_dist_tags,
//     } = pkg.2
//     {
//         let new_package_metadata = PackageMetadata::Normal {
//             dist_tag_latest_version: Some(version_id),
//             created,
//             modified,
//             other_dist_tags,
//         };

//         diesel::update(packages.find(package_id))
//             .set(metadata.eq(new_package_metadata))
//             .execute(&conn.conn)
//             .expect("Error updating package");
//     }
// }
