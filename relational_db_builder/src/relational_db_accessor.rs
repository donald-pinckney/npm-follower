use postgres_db::{
    connection::QueryRunner,
    custom_types::Semver,
    packages::{NewPackage, Package},
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

    pub fn get_package_id_by_name<R: QueryRunner>(&mut self, conn: &mut R, package: &str) -> i64 {
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
}
