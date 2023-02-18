use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use diesel::deserialize;
use diesel::deserialize::FromSql;
use diesel::pg::Pg;
use diesel::pg::PgValue;
use diesel::prelude::*;
use diesel::serialize;
use diesel::serialize::Output;
use diesel::serialize::ToSql;
use diesel::serialize::WriteTuple;
use diesel::sql_types::Int8;
use diesel::sql_types::Record;
use diesel::QueryableByName;
use historic_solver_job::{SolveResult, SolveResultSql};
use postgres_db::{
    connection::{DbConnection, QueryRunner},
    custom_types::Semver,
};
use rust_sql_analysis::relational_db_accessor::RelationalDbAccessor;
use serde::{Deserialize, Serialize};
use serde_json::Value;

fn print_usage_exit(argv0: &str) -> ! {
    eprintln!("Usage: {} chunk_size", argv0);
    std::process::exit(1);
}

fn main() {
    utils::check_no_concurrent_processes("process_historic_solver");
    dotenvy::dotenv().ok();
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() < 2 {
        print_usage_exit(args[0].as_str());
    }
    let chunk_size = args[1].parse::<i64>().unwrap();
    let conn: DbConnection = DbConnection::connect();
    process_historic_solver(conn, chunk_size);
}

#[derive(Serialize, Deserialize, QueryableByName, Debug)]
struct HistoricResultRow {
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    update_from_id: i64,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    update_to_id: i64,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    downstream_package_id: i64,
    #[diesel(sql_type = diesel::sql_types::Array<SolveResultSql>)]
    solve_history: Vec<SolveResult>,
}

fn process_historic_solver(mut conn: DbConnection, chunk_size: i64) {
    // use diesel::prelude::*;

    let mut last = None;
    let mut num_processed = 0;

    let mut db = RelationalDbAccessor::new();

    loop {
        println!("Loading {} rows from the table...", chunk_size);
        let time = std::time::Instant::now();
        let query = diesel::sql_query(format!(
            "
            SELECT
                update_from_id, 
                update_to_id, 
                downstream_package_id, 
                solve_history
            FROM historic_solver_job_results
            WHERE ROW(update_from_id, update_to_id, downstream_package_id) NOT IN (SELECT update_from_id, update_to_id, downstream_package_id FROM historic_solver_job_results_oldnesses){}
            ORDER BY update_from_id, update_to_id, downstream_package_id
            LIMIT {}",
            {
                if let Some((update_from_id, update_to_id, downstream_package_id)) = last {
                    format!(
                        "
                AND (update_from_id, update_to_id, downstream_package_id) > ({}, {}, {})",
                        update_from_id, update_to_id, downstream_package_id
                    )
                } else {
                    "".to_string()
                }
            },
            chunk_size,
        ));

        let table: Vec<HistoricResultRow> = conn
            .get_results(query)
            .expect("Failed to load the table from the database");
        let table_len = table.len();
        println!("Loaded {} rows in {:?}!", table_len, time.elapsed());
        num_processed += table_len;
        if table.is_empty() {
            break;
        }
        last = table
            .last()
            .map(|d| (d.update_from_id, d.update_to_id, d.downstream_package_id));

        println!("Progress: {} rows", num_processed);
        println!("Computing {} rows...", table.len());
        let time = std::time::Instant::now();
        let len_table = table.len();

        solve_oldness(&mut conn, &mut db, table).unwrap();

        println!("Wrote {} rows in {:?}!", len_table, time.elapsed());
    }
}

#[derive(Debug)]
struct SolveSolutionMetrics {
    downstream_v: Semver,
    solve_time: DateTime<Utc>,
    deps: HashMap<String, HashSet<Semver>>,
    full_package_lock: Value,
}

impl SolveSolutionMetrics {
    fn new(downstream_v: Semver, solve_time: DateTime<Utc>, full_package_lock: Value) -> Self {
        Self {
            downstream_v,
            solve_time,
            deps: HashMap::new(),
            full_package_lock,
        }
    }

    fn push_dep(&mut self, package: String, version: Semver) {
        self.deps.entry(package).or_default().insert(version);
    }

    fn contains(&self, package: &str, version: &Semver) -> bool {
        self.deps
            .get(package)
            .map(|versions| versions.contains(version))
            .unwrap_or(false)
    }

    fn to_solve_result(&self, update_package: &str) -> SolveResult {
        let mut versions: Vec<Semver> = self
            .deps
            .get(update_package)
            .map(|versions| versions.iter().cloned().collect())
            .unwrap_or_default();

        versions.sort();

        SolveResult {
            solve_time: self.solve_time,
            downstream_version: self.downstream_v.clone(),
            update_versions: versions,
            full_package_lock: self.full_package_lock.clone(),
        }
    }

    fn into_all_deps(self) -> Vec<(String, Semver)> {
        self.deps
            .into_iter()
            .flat_map(|(package, versions)| {
                versions
                    .into_iter()
                    .map(move |version| (package.clone(), version))
            })
            .collect()
    }

    fn all_old_gone(&self, package: &str, old_version: &Semver) -> bool {
        self.deps
            .get(package)
            .unwrap()
            .iter()
            .all(|v| v > old_version)
    }

    fn are_deps_removed(&self, package: &str) -> bool {
        !self.deps.contains_key(package)
    }
}

fn parse_lockfile_json(mut lock_json: Value) -> Result<SolveSolutionMetrics, ()> {
    let deps = lock_json
        .as_object_mut()
        .ok_or(())?
        .remove("packages")
        .ok_or(())?;

    let mut solution = SolveSolutionMetrics::new(
        Semver {
            major: 0,
            minor: 0,
            bug: 0,
            prerelease: vec![],
            build: vec![],
        },
        DateTime::<Utc>::MAX_UTC,
        Value::Null,
    );

    for (dep_path, dep_info) in deps.as_object().unwrap().iter() {
        if dep_path.is_empty() {
            continue;
        }

        let dep_info = dep_info.as_object().ok_or(())?;
        if dep_info.contains_key("link") {
            continue;
        }

        if !dep_path.contains("node_modules/") {
            continue;
        }

        let dep_name_start_idx = dep_path.rfind("node_modules/").ok_or(()).unwrap() + 13;
        let dep_name = &dep_path[dep_name_start_idx..];

        let version = dep_info.get("version").ok_or(())?.as_str().ok_or(())?;
        let version = semver_spec_serialization::parse_semver(version).map_err(|_| ())?;
        solution.push_dep(dep_name.to_string(), version);
    }

    Ok(solution)
}

fn how_old(
    conn: &mut DbConnection,
    db: &mut RelationalDbAccessor,
    package: &str,
    solved_version: Semver,
    solve_time: DateTime<Utc>,
) -> Option<OldnessPair> {
    let versions_pkg_id = db.get_package_version_times(conn, package);
    versions_pkg_id.1?;

    let versions = &versions_pkg_id.0;
    let dep_pkg_id = versions_pkg_id.1.unwrap();

    let solved_version = versions
        .binary_search_by(|probe| probe.0.cmp(&solved_version))
        .ok()?;
    let solved_version = &versions[solved_version];

    let most_recent_before_solve_time = versions
        .iter()
        .filter(|v| v.1 <= solve_time && v.0 >= solved_version.0)
        .max_by_key(|v| v.1)?;

    Some(OldnessPair {
        old_secs: (most_recent_before_solve_time.1 - solved_version.1).num_seconds(),
        dep_pkg_id,
    })
}

diesel::table! {
    use diesel::sql_types::*;

    historic_solver_job_results_oldnesses (update_from_id, update_to_id, downstream_package_id) {
        update_from_id -> Int8,
        update_to_id -> Int8,
        downstream_package_id -> Int8,
        oldnesses -> Array<crate::OldnessPairSql>,
    }
}

#[derive(diesel::sql_types::SqlType)]
#[diesel(postgres_type(name = "oldness_pair"))]
pub struct OldnessPairSql;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct OldnessPair {
    old_secs: i64,
    dep_pkg_id: i64,
}

type OldnessPairRecordSql = (Int8, Int8);

impl ToSql<OldnessPairSql, Pg> for OldnessPair {
    fn to_sql(&self, out: &mut Output<Pg>) -> serialize::Result {
        let record: (i64, i64) = (self.old_secs, self.dep_pkg_id);
        WriteTuple::<OldnessPairRecordSql>::write_tuple(&record, out)
    }
}

impl FromSql<OldnessPairSql, Pg> for OldnessPair {
    fn from_sql(bytes: PgValue) -> deserialize::Result<Self> {
        let (old_secs, dep_pkg_id): (i64, i64) =
            FromSql::<Record<(Int8, Int8)>, Pg>::from_sql(bytes)?;
        Ok(OldnessPair {
            old_secs,
            dep_pkg_id,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Insertable, Queryable, QueryableByName)]
#[diesel(table_name = historic_solver_job_results_oldnesses)]
struct OldnessRow {
    update_from_id: i64,
    update_to_id: i64,
    downstream_package_id: i64,
    oldnesses: Vec<OldnessPair>,
}

fn solve_oldness(
    conn: &mut DbConnection,
    db: &mut RelationalDbAccessor,
    table: Vec<HistoricResultRow>,
) -> Result<(), ()> {
    let mut oldness_rows: Vec<OldnessRow> = Vec::with_capacity(table.len());

    for row in table {
        if let Some(first_solve) = row.solve_history.first() {
            let solve_time = first_solve.solve_time;
            // println!("{},{},{}", row.update_from_id, row.update_to_id, row.downstream_package_id);
            let all_deps =
                parse_lockfile_json(first_solve.full_package_lock.clone())?.into_all_deps();
            let oldnesses = all_deps
                .into_iter()
                .filter_map(|(package, version)| how_old(conn, db, &package, version, solve_time))
                .collect::<Vec<_>>();

            oldness_rows.push(OldnessRow {
                update_from_id: row.update_from_id,
                update_to_id: row.update_to_id,
                downstream_package_id: row.downstream_package_id,
                oldnesses,
            });
        } else {
            oldness_rows.push(OldnessRow {
                update_from_id: row.update_from_id,
                update_to_id: row.update_to_id,
                downstream_package_id: row.downstream_package_id,
                oldnesses: vec![],
            });
        }
    }

    println!("Inserting {} rows to the table...", oldness_rows.len());

    let insert_query =
        diesel::insert_into(historic_solver_job_results_oldnesses::table).values(oldness_rows);
    // .on_conflict((
    //     historic_solver_job_results_oldnesses::update_from_id,
    //     historic_solver_job_results_oldnesses::update_to_id,
    //     historic_solver_job_results_oldnesses::downstream_package_id,
    // )).do_update()
    conn.execute(insert_query).unwrap();

    Ok(())
}
