use std::{collections::HashSet, num::NonZeroUsize, rc::Rc};

use chrono::{DateTime, Utc};
use deepsize::DeepSizeOf;
use lru::LruCache;
use postgres_db::{
    connection::QueryRunner,
    custom_types::Semver,
    dependencies::{Dependency, NewDependency},
    packages::{NewPackage, Package, PackageUpdate},
    versions::{NewVersion, Version},
};
use serde_json::Value;

struct DependencyState {
    id: i64,
    prod_freq_count: i64,
    dev_freq_count: i64,
    peer_freq_count: i64,
    optional_freq_count: i64,
}

impl DependencyState {
    fn add_counts(&mut self, other: NewDependency) {
        self.prod_freq_count += other.get_prod_freq_count();
        self.dev_freq_count += other.get_dev_freq_count();
        self.peer_freq_count += other.get_peer_freq_count();
        self.optional_freq_count += other.get_optional_freq_count();
    }
}

pub struct RelationalDbAccessor {
    package_id_cache: LruCache<String, Option<i64>>,
    package_data_cache: LruCache<String, Rc<Package>>,
    version_id_cache: LruCache<(i64, Semver), i64>,

    // hash --> state
    dependency_states_cache: LruCache<String, DependencyState>,
    need_flush_set: HashSet<String>,
}

impl RelationalDbAccessor {
    pub fn new() -> Self {
        Self {
            package_id_cache: LruCache::new(NonZeroUsize::new(0x10000).unwrap()), // about 65 K entries
            package_data_cache: LruCache::new(NonZeroUsize::new(62_500_000).unwrap()), // 62.5 MB max memory usage
            version_id_cache: LruCache::new(NonZeroUsize::new(0x20000).unwrap()), // about 131 K entries
            dependency_states_cache: LruCache::new(NonZeroUsize::new(0x100000).unwrap()), // about 1 M entries
            need_flush_set: HashSet::with_capacity(0x100000),
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
        // If the dep exists in the cache, we just increment the cache only,
        // and mark it for flushing later
        if let Some(mem_state) = self
            .dependency_states_cache
            .get_mut(dep.get_md5digest_with_version())
        {
            // println!("dep cache hit");
            self.need_flush_set
                .insert(dep.get_md5digest_with_version().to_string());
            mem_state.add_counts(dep);
            return mem_state.id;
        }
        // println!("dep cache miss");

        let copy_of_hash = dep.get_md5digest_with_version().to_string();

        // If the dep isn't in the cache, that's because either:
        // a) it doesn't exist in the DB, and needs to be created, or
        // b) it is in the DB but not in the cache,
        //
        // We deal with both cases at once with an INSERT ON CONFLICT statement
        // in which we increment counts, and then get back ID and all the updated counts
        let (id, prod_freq_count, dev_freq_count, peer_freq_count, optional_freq_count) =
            postgres_db::dependencies::insert_dependency_inc_counts(conn, dep);
        let new_state = DependencyState {
            id,
            prod_freq_count,
            dev_freq_count,
            peer_freq_count,
            optional_freq_count,
        };
        // initially not marked as needing flush, since hasn't received mem only writes yet,
        // so we don't add it to self.need_flush_set

        if let Some((evicted_hash, evicted_state)) =
            self.dependency_states_cache.push(copy_of_hash, new_state)
        {
            // println!("Evicting cache entry");

            // In the case that we had to evict an entry, we MUST flush it to the DB (if needed)
            if self.need_flush_set.contains(&evicted_hash) {
                self.need_flush_set.remove(&evicted_hash);

                // println!("Flushing evicted cache entry");

                postgres_db::dependencies::set_dependency_counts(
                    conn,
                    evicted_state.id,
                    (
                        evicted_state.prod_freq_count,
                        evicted_state.dev_freq_count,
                        evicted_state.peer_freq_count,
                        evicted_state.optional_freq_count,
                    ),
                )
            }
        }

        id
    }

    pub fn get_version_by_id<R: QueryRunner>(&self, conn: &mut R, version_id: i64) -> Version {
        postgres_db::versions::get_version_by_id(conn, version_id)
    }

    pub fn get_dependency_by_id<R: QueryRunner>(
        &self,
        conn: &mut R,
        dependency_id: i64,
    ) -> Dependency {
        postgres_db::dependencies::get_dependency_by_id(conn, dependency_id)
    }

    pub fn set_version_extra_metadata<R>(
        &self,
        conn: &mut R,
        version_id: i64,
        new_extra_metadata: Value,
    ) where
        R: QueryRunner,
    {
        postgres_db::versions::set_version_extra_metadata(conn, version_id, new_extra_metadata)
    }

    pub fn delete_version<R: QueryRunner>(
        &mut self,
        conn: &mut R,
        version_id: i64,
        seq: i64,
        diff_entry_id: i64,
        delete_time: Option<DateTime<Utc>>,
    ) {
        postgres_db::versions::delete_version(conn, version_id, seq, diff_entry_id, delete_time);
    }

    pub fn flush_caches<R>(&mut self, conn: &mut R)
    where
        R: QueryRunner,
    {
        // println!("flushing caches");

        // Flush everything, don't clear the cache, and mark everything as flushed

        self.need_flush_set.iter().for_each(|hash_needs_flush| {
            let state = self
                .dependency_states_cache
                .peek(hash_needs_flush)
                .unwrap_or_else(|| {
                    panic!(
                        "BUG: {} needs to be flushed, but isn't in the cache",
                        hash_needs_flush
                    )
                });
            postgres_db::dependencies::set_dependency_counts(
                conn,
                state.id,
                (
                    state.prod_freq_count,
                    state.dev_freq_count,
                    state.peer_freq_count,
                    state.optional_freq_count,
                ),
            );
        });
        self.need_flush_set.clear();
    }
}

impl RelationalDbAccessor {
    pub fn get_package_id_by_name<R: QueryRunner>(&mut self, conn: &mut R, package: &str) -> i64 {
        self.maybe_get_package_id_by_name(conn, package)
            .expect("The package should exist")
    }

    pub fn insert_or_inc_dependencies<R>(
        &mut self,
        conn: &mut R,
        deps: Vec<NewDependency>,
    ) -> Vec<i64>
    where
        R: QueryRunner,
    {
        deps.into_iter()
            .map(|d| self.insert_or_inc_dependency(conn, d))
            .collect()
    }
}
