use postgres_db::{
    connection::QueryRunner,
    custom_types::Semver,
    packages::{NewPackage, Package, PackageUpdate},
    versions::NewVersion,
};

pub struct RelationalDbAccessor {}

impl RelationalDbAccessor {
    pub fn new() -> Self {
        Self {}
    }
}

impl RelationalDbAccessor {
    pub fn get_package_by_name<R: QueryRunner>(&mut self, conn: &mut R, package: &str) -> Package {
        todo!()
    }

    pub fn maybe_get_package_id_by_name<R: QueryRunner>(
        &mut self,
        conn: &mut R,
        package: &str,
    ) -> Option<i64> {
        todo!()
    }

    pub fn get_version_id_by_semver<R: QueryRunner>(
        &mut self,
        conn: &mut R,
        package_id: i64,
        v: Semver,
    ) -> i64 {
        todo!()
    }

    pub fn insert_new_package<R: QueryRunner>(&mut self, conn: &mut R, new_package: NewPackage) {
        todo!()
        // postgres_db::packages::insert_new_package(conn, new_package);
    }

    pub fn update_package<R: QueryRunner>(
        &mut self,
        conn: &mut R,
        package_id: i64,
        diff: PackageUpdate,
    ) {
        //         postgres_db::packages::update_package(conn, &package, diff);
        todo!()
    }

    pub fn update_deps_missing_pack<R: QueryRunner>(
        &mut self,
        conn: &mut R,
        package_name: &str,
        package_id: i64,
    ) {
        postgres_db::dependencies::update_deps_missing_pack(conn, package_name, package_id)
    }

    pub fn insert_new_version<R: QueryRunner>(&mut self, conn: &mut R, new_version: NewVersion) {
        todo!()
        // postgres_db::packages::insert_new_package(conn, new_package);
    }
}

impl RelationalDbAccessor {
    pub fn get_package_id_by_name<R: QueryRunner>(&mut self, conn: &mut R, package: &str) -> i64 {
        self.maybe_get_package_id_by_name(conn, package)
            .expect("The package should exist")
    }
}
