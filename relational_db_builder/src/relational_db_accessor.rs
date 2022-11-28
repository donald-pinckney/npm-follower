use std::num::NonZeroUsize;

use deepsize::DeepSizeOf;
use lru::LruCache;
use postgres_db::{
    connection::QueryRunner,
    custom_types::Semver,
    packages::{NewPackage, Package, PackageUpdate},
    versions::NewVersion,
};

pub struct RelationalDbAccessor {
    package_id_cache: LruCache<String, i64>,
    package_data_cache: LruCache<String, Package>,
}

impl RelationalDbAccessor {
    pub fn new() -> Self {
        Self {
            package_id_cache: LruCache::new(NonZeroUsize::new(0x100000).unwrap()), // about 1 million entries
            package_data_cache: LruCache::new(NonZeroUsize::new(1_073_741_824).unwrap()), // 1GB max memory usage
        }
    }
}

impl Default for RelationalDbAccessor {
    fn default() -> Self {
        RelationalDbAccessor::new()
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
        let inserted = postgres_db::packages::insert_new_package(conn, new_package);

        let cache_entry_size = inserted.deep_size_of() + inserted.name.deep_size_of();
        self.package_id_cache
            .put(inserted.name.clone(), inserted.id);
        self.package_data_cache
            .put_with_cost(inserted.name.clone(), inserted, cache_entry_size);
    }

    pub fn update_package<R: QueryRunner>(
        &mut self,
        conn: &mut R,
        package_id: i64,
        package_name: &str,
        diff: PackageUpdate,
    ) {
        // NB: If the cache doesn't already have the package, we don't add it to the cache, since that would involve
        // potentially a lot more Postgres I/O
        let old_entry = self.package_data_cache.pop(package_name);
        let new_entry = old_entry.map(|mut e| {
            e.apply_diff(&diff);
            e
        });
        if let Some(new_entry) = new_entry {
            let cache_entry_size = new_entry.deep_size_of() + new_entry.name.deep_size_of();
            self.package_data_cache.put_with_cost(
                new_entry.name.clone(),
                new_entry,
                cache_entry_size,
            );
        }
        postgres_db::packages::update_package(conn, package_id, diff);
    }

    pub fn update_deps_missing_pack<R: QueryRunner>(
        &mut self,
        conn: &mut R,
        package_name: &str,
        package_id: i64,
    ) {
        self.package_id_cache
            .put(package_name.to_string(), package_id);
        self.package_data_cache.promote(package_name);
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
