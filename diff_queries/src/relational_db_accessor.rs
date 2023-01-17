use std::{collections::HashSet, num::NonZeroUsize, rc::Rc};

use chrono::{DateTime, Utc};
use diesel::backend::sql_dialect::returning_clause::PgLikeReturningClause;
use lru::LruCache;
use postgres_db::{
    connection::QueryRunner,
    custom_types::Semver,
    dependencies::{Dependency, NewDependency},
    packages::{NewPackage, Package, PackageUpdate},
    versions::{NewVersion, Version},
};
use serde_json::Value;

pub struct RelationalDbAccessor {
    versions_cache: LruCache<String, Rc<(Vec<(Semver, DateTime<Utc>)>, Option<i64>)>>,
}

impl RelationalDbAccessor {
    pub fn new() -> Self {
        Self {
            versions_cache: LruCache::new(NonZeroUsize::new(0x100000).unwrap()), // about 1 million entries
        }
    }
}

impl Default for RelationalDbAccessor {
    fn default() -> Self {
        RelationalDbAccessor::new()
    }
}

impl RelationalDbAccessor {
    pub fn get_package_version_times<R: QueryRunner>(
        &mut self,
        conn: &mut R,
        package: &str,
    ) -> Rc<(Vec<(Semver, DateTime<Utc>)>, Option<i64>)> {
        if let Some(versions) = self.versions_cache.get(package) {
            return versions.clone();
        }

        let package_id = postgres_db::packages::maybe_get_package_id_by_name(conn, package);
        if package_id.is_none() {
            let versions = Rc::new((vec![], None));
            self.versions_cache
                .put(package.to_string(), versions.clone());
            return versions;
        }

        let package_id = package_id.unwrap();

        let versions = postgres_db::versions::get_version_times(conn, package_id);

        let mut versions = versions
            .into_iter()
            .filter(|v| v.0.prerelease.is_empty() && v.0.build.is_empty())
            .collect::<Vec<_>>();

        versions.sort_by(|a, b| a.0.cmp(&b.0));

        let versions = Rc::new((versions, Some(package_id)));
        self.versions_cache
            .put(package.to_string(), versions.clone());
        versions
    }
}
