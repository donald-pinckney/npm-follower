use rusqlite::Connection;
use serde_json::Value;
use serde_json::Map;
use crate::PackageReference;
use crate::packument::Dependencies;
use chrono::Utc;
use chrono::DateTime;
use crate::VersionPackument;
use crate::version::Version;
use crate::Packument;
use std::collections::HashMap;
use std::collections::HashSet;
use std::convert::TryFrom;
use crate::sql_data;
use crate::sql_insertable::SqlInsertable;

pub struct Inserter<'pkgs> {
  pkgs_not_processed: HashSet<&'pkgs String>,
  downloads: HashMap<&'pkgs String, u64>,
  // pkg_id_to_name: HashMap<u64, &'pkgs String>,
  pkg_name_to_id: HashMap<&'pkgs String, u64>,
  version_id_counter: u64,
  dep_id_counter: u64,
  connection: Connection
}

impl<'pkgs> Inserter<'pkgs> {
  pub fn new(pkg_names: &'pkgs HashSet<String>, downloads: HashMap<&'pkgs String, u64>) -> Inserter<'pkgs> {
    let pkgs_not_processed: HashSet<_> = pkg_names.iter().collect();
    let pkg_id_to_name: HashMap<_, _> = pkg_names.iter().enumerate().map(|(i, p)| (u64::try_from(i).unwrap(), p)).collect();
    let pkg_name_to_id: HashMap<_, _> = pkg_id_to_name.iter().map(|(i, p)| (*p, *i)).collect();

    let conn = Connection::open("npm_db.sqlite3").unwrap();

    conn.execute_batch(r"
      PRAGMA journal_mode = OFF;
      PRAGMA synchronous = 0;
      PRAGMA cache_size = 1000000;
      PRAGMA locking_mode = EXCLUSIVE;
      PRAGMA temp_store = MEMORY;
    ").expect("PRAGMA");

    conn.execute_batch(r"
    CREATE TABLE `package` (
        `id` INTEGER NOT NULL PRIMARY KEY,
        `name` TEXT UNIQUE NOT NULL,
        `downloads` INTEGER COMMENT 'Number of downloads in August 2021',
        `latest_version` INTEGER,
        `created` TEXT NOT NULL,
        `modified` TEXT NOT NULL,
        `other_dist_tags` TEXT
      );
      CREATE TABLE `version` (
        `id` INTEGER NOT NULL PRIMARY KEY,
        `package_id` INTEGER NOT NULL,
        `description` TEXT,
        `shasum` TEXT NOT NULL,
        `tarball` TEXT NOT NULL,
        `major` INTEGER NOT NULL,
        `minor` INTEGER NOT NULL,
        `bug` INTEGER NOT NULL,
        `prerelease` TEXT,
        `build` TEXT,
        `created` TEXT NOT NULL,
        `extra_metadata` TEXT NOT NULL
      );
      CREATE TABLE `dependency` (
        `id` INTEGER NOT NULL PRIMARY KEY,
        `package_raw` TEXT,
        `package_id` INTEGER,
        `spec_raw` TEXT
      );
      CREATE TABLE `version_dependencies` (
        `version_id` INTEGER NOT NULL,
        `dependency_id` INTEGER NOT NULL,
        `type` INTEGER NOT NULL, COMMENT '0 = prod, 1 = dev, 2 = peer, 3 = optional',
        `dependency_index` INTEGER NOT NULL,
        PRIMARY KEY (`version_id`, `dependency_id`, `type`)
      );
    ").unwrap();

    Inserter {
      pkgs_not_processed: pkgs_not_processed,
      downloads: downloads,
      // pkg_id_to_name: pkg_id_to_name,
      pkg_name_to_id: pkg_name_to_id,
      version_id_counter: 0,
      dep_id_counter: 0,
      connection: conn
    }
  }

  pub fn build_indexes(&self) {
    self.connection.execute_batch(r"
      CREATE INDEX idx_version_package_id ON version(package_id);
      CREATE INDEX idx_dependency_package_id ON dependency(package_id);
    ").unwrap();
  }


  pub fn insert_packument(&mut self, pkg_name: &'pkgs String, pack: Packument<'pkgs>) {
    let pkg_id = self.pkg_name_to_id[pkg_name];
    let d = self.downloads.get(pkg_name).map(|d| *d);
    let other_dist_tags: Option<HashMap<_, _>> = pack.other_dist_tags.map(|dts| dts.into_iter().map(|(t, v)| (t, format!("{}", v))).collect());
    let other_dist_tags_json = other_dist_tags.map(|odt| {
      let the_map: Map<_, _> = odt.into_iter().map(|(k, v)| (k, Value::String(v))).collect();
      Value::Object(the_map)
    });
    let pack_latest = pack.latest;

    let mut package = sql_data::Package {
      id: pkg_id,
      name: pkg_name,
      downloads: d,
      latest_version: None, // TEMP VALUE, fill in later with a version ID
      created: pack.created,
      modified: pack.modified,
      other_dist_tags: other_dist_tags_json,
    };

    let mut latest_version_id = None;

    let mut version_rows = Vec::new();
    let mut dep_rows = Vec::new();
    let mut rel_rows = Vec::new();

    for (v, v_pack) in pack.versions {
      let is_latest = match &pack_latest {
        Some(l) => *l == v,
        None => false
      };
      
      //pack_latest.map(|l| l == v).unwrap_or(false);

      if let Some((v_row, v_deps)) = self.build_version_row(pkg_id, v, v_pack, &pack.version_times) {
        if is_latest {
          latest_version_id = Some(v_row.id);
        }

        self.build_dependency_rows(v_row.id, v_deps, &mut dep_rows, &mut rel_rows);
        version_rows.push(v_row);
      }
    }

    package.latest_version = latest_version_id;

    
    // START TRANSACTION

    // self.connection.start_transaction();
    let tr = self.connection.transaction().unwrap();


    tr.execute(sql_data::Package::INSERT_TEMPLATE, &package.params()[..]).unwrap();


    for v_row in version_rows {
      // self.connection.insert(v_row);
      tr.execute(sql_data::Version::INSERT_TEMPLATE, &v_row.params()[..]).unwrap();

    }

    for dep_row in dep_rows {
      // self.connection.insert(dep_row);
      tr.execute(sql_data::Dependency::INSERT_TEMPLATE, &dep_row.params()[..]).unwrap();
    }

    for rel_row in rel_rows {
      // self.connection.insert(rel_row);
      tr.execute(sql_data::VersionDependencyRelation::INSERT_TEMPLATE, &rel_row.params()[..]).unwrap();

    }

    // FINISH TRANSACTION

    // self.connection.commit_transaction();
    tr.commit().unwrap();

    self.pkgs_not_processed.remove(pkg_name);
  }

  fn build_dependency_hash_rows(&mut self, 
    v_id: u64, 
    deps: HashMap<PackageReference<'pkgs>, (u64, Option<String>)>, 
    dep_type: i32, 
    into_dep_rows: &mut Vec<sql_data::Dependency>, 
    into_rel_rows: &mut Vec<sql_data::VersionDependencyRelation>) {

    for (dst_pkg, (dep_idx, spec)) in deps {
      let dep_id = self.dep_id_counter;
      self.dep_id_counter += 1;

      let dep_row = match dst_pkg {
        PackageReference::Known(dst_pkg_name) => sql_data::Dependency {
          id: dep_id,
          package_raw: None,
          package_id: Some(self.pkg_name_to_id[dst_pkg_name]),
          spec_raw: spec
        },
        PackageReference::Unknown(dst_pkg_name) => sql_data::Dependency {
          id: dep_id,
          package_raw: Some(dst_pkg_name),
          package_id: None,
          spec_raw: spec
        }
      };

      let rel_row = sql_data::VersionDependencyRelation {
        version_id: v_id,
        dependency_id: dep_row.id,
        dep_type: dep_type,
        dependency_index: dep_idx
      };

      into_dep_rows.push(dep_row);
      into_rel_rows.push(rel_row);
    }
  }

  fn build_dependency_rows(&mut self, 
    v_id: u64, 
    deps: Dependencies<'pkgs>, 
    into_dep_rows: &mut Vec<sql_data::Dependency>, 
    into_rel_rows: &mut Vec<sql_data::VersionDependencyRelation>) {
    
    self.build_dependency_hash_rows(v_id, deps.prod_dependencies, sql_data::DEPENDENCY_TYPE_PROD, into_dep_rows, into_rel_rows);
    self.build_dependency_hash_rows(v_id, deps.dev_dependencies, sql_data::DEPENDENCY_TYPE_DEV, into_dep_rows, into_rel_rows);
    self.build_dependency_hash_rows(v_id, deps.peer_dependencies, sql_data::DEPENDENCY_TYPE_PEER, into_dep_rows, into_rel_rows);
    self.build_dependency_hash_rows(v_id, deps.optional_dependencies, sql_data::DEPENDENCY_TYPE_OPTIONAL, into_dep_rows, into_rel_rows);
  }

  fn build_version_row(&mut self, pkg_id: u64, v: Version, v_pack: VersionPackument<'pkgs>, v_times: &HashMap<Version, DateTime<Utc>>) 
  -> Option<(sql_data::Version, Dependencies<'pkgs>)> {
    let created_time = *v_times.get(&v).or_else(|| {
      println!("Didn't have time: {:#?}", v_pack);
      None
    })?;

    let id = self.version_id_counter;
    self.version_id_counter += 1;

    let other_dist_tags_json = Value::Object(v_pack.extra_metadata.into_iter().collect());

    let v_row = sql_data::Version {
      id: id,
      package_id: pkg_id,
      description: v_pack.description,
      shasum: v_pack.shasum,
      tarball: v_pack.tarball,
      major: v.major,
      minor: v.minor,
      bug: v.bug,
      prerelease: v.prerelease,
      build: v.build,
      created: created_time,
      extra_metadata: other_dist_tags_json
    };
    let deps = v_pack.dependencies;
    Some((v_row, deps))
  }
}