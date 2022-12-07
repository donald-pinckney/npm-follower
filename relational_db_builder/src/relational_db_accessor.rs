use std::{num::NonZeroUsize, rc::Rc};

use deepsize::DeepSizeOf;
use lru::LruCache;
use postgres_db::{
    connection::QueryRunner,
    custom_types::Semver,
    dependencies::NewDependency,
    packages::{NewPackage, Package, PackageUpdate},
    versions::NewVersion,
};

pub struct RelationalDbAccessor {
    package_id_cache: LruCache<String, Option<i64>>,
    package_data_cache: LruCache<String, Rc<Package>>,
    version_id_cache: LruCache<(i64, Semver), i64>,
}

impl RelationalDbAccessor {
    pub fn new() -> Self {
        Self {
            package_id_cache: LruCache::new(NonZeroUsize::new(0x100000).unwrap()), // about 1 million entries
            package_data_cache: LruCache::new(NonZeroUsize::new(1_073_741_824).unwrap()), // 1GB max memory usage
            version_id_cache: LruCache::new(NonZeroUsize::new(0x100000).unwrap()), // about 1 million entries
        }
    }
}

impl Default for RelationalDbAccessor {
    fn default() -> Self {
        RelationalDbAccessor::new()
    }
}

impl RelationalDbAccessor {
    pub fn get_package_by_name<R: QueryRunner>(
        &mut self,
        conn: &mut R,
        package_name: &str,
    ) -> Rc<Package> {
        if let Some(package) = self.package_data_cache.get(package_name) {
            self.package_id_cache
                .put(package_name.to_string(), Some(package.id));
            return Rc::clone(package);
        }

        if let Some(&package_id) = self.package_id_cache.get(package_name) {
            let package_id = package_id.expect("Package does not exist!");
            let package = postgres_db::packages::get_package(conn, package_id);
            let cache_entry_size = package.deep_size_of() + package.name.deep_size_of();

            let package = Rc::new(package);
            self.package_data_cache.put_with_cost(
                package.name.clone(),
                Rc::clone(&package),
                cache_entry_size,
            );
            return package;
        }

        let package = postgres_db::packages::get_package_by_name(conn, package_name);
        let cache_entry_size = package.deep_size_of() + package.name.deep_size_of();

        let package = Rc::new(package);
        self.package_data_cache.put_with_cost(
            package.name.clone(),
            Rc::clone(&package),
            cache_entry_size,
        );
        self.package_id_cache
            .put(package_name.to_string(), Some(package.id));
        package
    }

    pub fn maybe_get_package_id_by_name<R: QueryRunner>(
        &mut self,
        conn: &mut R,
        package: &str,
    ) -> Option<i64> {
        if let Some(&package_id) = self.package_id_cache.get(package) {
            return package_id;
        }

        let package_id = postgres_db::packages::maybe_get_package_id_by_name(conn, package);
        self.package_id_cache.put(package.to_string(), package_id);
        package_id
    }

    pub fn get_version_id_by_semver<R: QueryRunner>(
        &mut self,
        conn: &mut R,
        package_id: i64,
        v: Semver,
    ) -> i64 {
        if let Some(&version_id) = self.version_id_cache.get(&(package_id, v.clone())) {
            version_id
        } else {
            let version_id =
                postgres_db::versions::get_version_id_by_semver(conn, package_id, v.clone());
            self.version_id_cache.put((package_id, v), version_id);
            version_id
        }
    }

    pub fn insert_new_package<R: QueryRunner>(&mut self, conn: &mut R, new_package: NewPackage) {
        let inserted = postgres_db::packages::insert_new_package(conn, new_package);

        let cache_entry_size = inserted.deep_size_of() + inserted.name.deep_size_of();
        self.package_id_cache
            .put(inserted.name.clone(), Some(inserted.id));
        self.package_data_cache.put_with_cost(
            inserted.name.clone(),
            Rc::new(inserted),
            cache_entry_size,
        );
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
            Rc::get_mut(&mut e)
                .expect("there should be no other references to the cache entry")
                .apply_diff(&diff);
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
            .put(package_name.to_string(), Some(package_id));
        self.package_data_cache.promote(package_name);
        postgres_db::dependencies::update_deps_missing_pack(conn, package_name, package_id)
    }

    pub fn insert_new_version<R: QueryRunner>(&mut self, conn: &mut R, new_version: NewVersion) {
        let package_id = new_version.package_id;
        let semver = new_version.semver.clone();
        let id = postgres_db::versions::insert_new_version(conn, new_version);
        self.version_id_cache.put((package_id, semver), id);
    }

    fn insert_or_inc_dependency<R>(&mut self, conn: &mut R, dep: NewDependency) -> i64
    where
        R: QueryRunner,
    {
        todo!()
    }

    pub fn insert_or_inc_dependencies<R>(
        &mut self,
        conn: &mut R,
        deps: Vec<NewDependency>,
    ) -> Vec<i64>
    where
        R: QueryRunner,
    {
        postgres_db::dependencies::insert_dependencies(conn, deps)
    }
}

impl RelationalDbAccessor {
    pub fn get_package_id_by_name<R: QueryRunner>(&mut self, conn: &mut R, package: &str) -> i64 {
        self.maybe_get_package_id_by_name(conn, package)
            .expect("The package should exist")
    }

    // pub fn insert_or_inc_dependencies<R>(
    //     &mut self,
    //     conn: &mut R,
    //     deps: Vec<NewDependency>,
    // ) -> Vec<i64>
    // where
    //     R: QueryRunner,
    // {
    //     deps.into_iter()
    //         .map(|d| self.insert_or_inc_dependency(conn, d))
    //         .collect()
    // }
}
