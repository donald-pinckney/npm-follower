mod relational_db_accessor;

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Duration, Utc};
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
use historic_solver_job_server::{JobResult, SolveResult, SolveResultSql};
use postgres_db::{
    connection::{DbConnection, QueryRunner},
    custom_types::{ParsedSpec, Semver},
    dependencies::Dependency,
    diff_analysis::{DiffAnalysis, DiffAnalysisJobResult},
    versions::Version,
};
use relational_db_accessor::RelationalDbAccessor;
use serde::{Deserialize, Serialize};
use serde_json::Value;

fn print_usage_exit(argv0: &str) -> ! {
    eprintln!("Usage: {} chunk_size", argv0);
    std::process::exit(1);
}

fn main() {
    utils::check_no_concurrent_processes("diff_queries");
    dotenvy::dotenv().ok();
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() < 2 {
        print_usage_exit(args[0].as_str());
    }
    let chunk_size = args[1].parse::<i64>().unwrap();
    let conn: DbConnection = DbConnection::connect();
    // process_diff_all_updates(conn, chunk_size);
    process_historic_solver(conn, chunk_size);
}

fn process_diff_analysis(mut conn: DbConnection, chunk_size: i64) {
    let mut last = None;
    let mut num_processed = 0;
    let total_count = postgres_db::diff_analysis::count_diff_analysis(&mut conn).unwrap();

    loop {
        println!("Loading {} rows from the table...", chunk_size);
        let time = std::time::Instant::now();
        let table =
            postgres_db::diff_analysis::query_table(&mut conn, Some(chunk_size), last).unwrap();
        let table_len = table.len();
        println!("Loaded {} rows in {:?}!", table_len, time.elapsed());
        num_processed += table_len;
        println!(
            "Progress: {:.2}%",
            num_processed as f64 / total_count as f64 * 100.0
        );
        if table.is_empty() {
            break;
        }
        last = table.last().map(|d| (d.from_id, d.to_id));

        println!("Writing {} rows to the table...", table.len());
        let time = std::time::Instant::now();
        let len_table = table.len();

        // insert here queries to write
        changed_file(&mut conn, &table).expect("Failed to write to file");

        println!("Wrote {} rows in {:?}!", len_table, time.elapsed());
    }
}

//  package_id | from_group_base_semver | to_group_base_semver | from_id |  to_id  | from_semver | to_semver |        from_created        |         to_created         |  ty

#[derive(Serialize, Deserialize, QueryableByName, Debug, Clone)]
struct Update {
    #[sql_type = "diesel::sql_types::BigInt"]
    package_id: i64,
    #[sql_type = "postgres_db::schema::sql_types::SemverStruct"]
    from_group_base_semver: Semver,
    #[sql_type = "postgres_db::schema::sql_types::SemverStruct"]
    to_group_base_semver: Semver,
    #[sql_type = "diesel::sql_types::BigInt"]
    from_id: i64,
    #[sql_type = "diesel::sql_types::BigInt"]
    to_id: i64,
    #[sql_type = "postgres_db::schema::sql_types::SemverStruct"]
    from_semver: Semver,
    #[sql_type = "postgres_db::schema::sql_types::SemverStruct"]
    to_semver: Semver,
    #[sql_type = "diesel::sql_types::Timestamp"]
    from_created: chrono::NaiveDateTime,
    #[sql_type = "diesel::sql_types::Timestamp"]
    to_created: chrono::NaiveDateTime,
    #[sql_type = "diesel::sql_types::Text"]
    ty: String,
}

fn process_diff_all_updates(mut conn: DbConnection, chunk_size: i64) {
    let mut last = None;
    let mut num_processed = 0;

    loop {
        println!("Loading {} rows from the table...", chunk_size);
        let time = std::time::Instant::now();
        let query = diesel::sql_query(format!(
            "
            SELECT
                package_id,
                from_group_base_semver,
                to_group_base_semver,
                from_id,
                to_id,
                from_semver,
                to_semver,
                from_created,
                to_created,
                ty
            FROM analysis.all_updates{}
            ORDER BY from_id, to_id
            LIMIT {}",
            {
                if let Some((from_id, to_id)) = last {
                    format!(
                        "
                WHERE (from_id, to_id) > ({}, {})",
                        from_id, to_id
                    )
                } else {
                    "".to_string()
                }
            },
            chunk_size,
        ));

        let table: Vec<Update> = conn
            .get_results(query)
            .expect("Failed to load the table from the database");
        let table_len = table.len();
        println!("Loaded {} rows in {:?}!", table_len, time.elapsed());
        num_processed += table_len;
        if table.is_empty() {
            break;
        }
        last = table.last().map(|d| (d.from_id, d.to_id));

        println!("Progress: {} rows", num_processed);
        println!("Writing {} rows to the file...", table.len());
        let time = std::time::Instant::now();
        let len_table = table.len();

        // insert here queries to write
        dep_update_changes(&mut conn, table).unwrap();

        println!("Wrote {} rows in {:?}!", len_table, time.elapsed());
    }
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

        let dep_name_start_idx = dep_path.rfind("node_modules/").ok_or(())? + 13;
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
    if versions_pkg_id.1.is_none() {
        return None;
    }

    let versions = &versions_pkg_id.0;
    let dep_pkg_id = versions_pkg_id.1.unwrap();

    let solved_version = versions
        .binary_search_by(|probe| probe.0.cmp(&solved_version))
        .ok()?;
    let solved_version = &versions[solved_version];

    let most_recent_before_solve_time = versions
        .iter()
        .filter(|v| v.1 <= solve_time)
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

fn dep_update_changes(
    conn: &mut DbConnection,
    table: Vec<Update>,
) -> Result<(), diesel::result::Error> {
    struct UpdateDep {
        from_id: i64,
        to_id: i64,
        did_add_dep: bool,
        did_remove_dep: bool,
        did_change_dep_constraint: bool,
    }
    let mut rows: Vec<UpdateDep> = Vec::new();
    for update in table {
        let from_ver = postgres_db::versions::get_version_by_id(conn, update.from_id);
        let to_ver = postgres_db::versions::get_version_by_id(conn, update.to_id);
        let mut from_deps = HashMap::new();
        let mut to_deps = HashMap::new();
        let mut fn_add_deps = |deps: &mut HashMap<String, ParsedSpec>, ver: &Version| {
            for dep_id in ver
                .prod_dependencies
                .iter()
                .chain(ver.dev_dependencies.iter())
                .chain(ver.peer_dependencies.iter())
                .chain(ver.optional_dependencies.iter())
            {
                let dep = postgres_db::dependencies::get_dependency_by_id(conn, *dep_id);
                deps.insert(dep.dst_package_name, dep.spec);
            }
        };
        fn_add_deps(&mut from_deps, &from_ver);
        fn_add_deps(&mut to_deps, &to_ver);

        let mut did_add_dep = false;
        let mut did_remove_dep = false;
        let mut did_change_dep_constraint = false;
        for (dep_name, from_spec) in from_deps {
            if let Some(to_spec) = to_deps.remove(&dep_name) {
                if from_spec != to_spec {
                    did_change_dep_constraint = true;
                }
            } else {
                did_remove_dep = true;
            }
        }
        if !to_deps.is_empty() {
            did_add_dep = true;
        }

        rows.push(UpdateDep {
            from_id: update.from_id,
            to_id: update.to_id,
            did_add_dep,
            did_remove_dep,
            did_change_dep_constraint,
        });
    }

    // insert into analysis.update_dep_changes
    let query = r#"
        INSERT INTO analysis.update_dep_changes (from_id, to_id, did_add_dep, did_remove_dep, did_change_dep_constraint)
        VALUES
    "#;
    let mut query = query.to_string();
    for (i, nf) in rows.iter().enumerate() {
        if i > 0 {
            query.push_str(", ");
        }
        query.push_str(&format!(
            "({}, {}, {}, {}, {})",
            nf.from_id, nf.to_id, nf.did_add_dep, nf.did_remove_dep, nf.did_change_dep_constraint
        ));
    }
    query.push_str(" ON CONFLICT (from_id, to_id) DO NOTHING");
    println!("Inserting {} rows into the table...", rows.len());
    let diesel_query = diesel::sql_query(query);
    conn.execute(diesel_query)?;
    Ok(())
}

#[derive(Serialize, Deserialize, QueryableByName, Debug, Clone)]
struct NumFiles {
    #[sql_type = "diesel::sql_types::BigInt"]
    from_id: i64,
    #[sql_type = "diesel::sql_types::BigInt"]
    to_id: i64,
    #[sql_type = "diesel::sql_types::BigInt"]
    num_files_added: i64,
    #[sql_type = "diesel::sql_types::BigInt"]
    num_files_modified: i64,
    #[sql_type = "diesel::sql_types::BigInt"]
    num_files_deleted: i64,
}

fn num_files(
    conn: &mut DbConnection,
    diffs: &Vec<DiffAnalysis>,
) -> Result<(), diesel::result::Error> {
    let mut num_files: Vec<NumFiles> = vec![];
    for diff in diffs {
        let mut num_files_added = 0;
        let mut num_files_deleted = 0;
        let mut num_files_modified = 0;
        match &diff.job_result {
            DiffAnalysisJobResult::Diff(map) => {
                for diff in map.values() {
                    if diff.total_old.is_some()
                        && diff.total_new.is_some()
                        && diff.added > 0
                        && diff.removed > 0
                    {
                        num_files_modified += 1;
                    } else if diff.total_new.is_none() {
                        num_files_deleted += 1;
                    } else if diff.total_old.is_none() {
                        num_files_added += 1;
                    }
                }
            }
            _ => continue,
        }
        num_files.push(NumFiles {
            from_id: diff.from_id,
            to_id: diff.to_id,
            num_files_added,
            num_files_modified,
            num_files_deleted,
        });
    }

    if num_files.is_empty() {
        return Ok(());
    }

    // insert into analysis.diff_num_files
    let query = r#"
        INSERT INTO analysis.diff_num_files (from_id, to_id, num_files_added, num_files_modified, num_files_deleted)
        VALUES
        "#;
    let mut query = query.to_string();
    for (i, nf) in num_files.iter().enumerate() {
        if i > 0 {
            query.push_str(", ");
        }
        query.push_str(&format!(
            "({}, {}, {}, {}, {})",
            nf.from_id, nf.to_id, nf.num_files_added, nf.num_files_modified, nf.num_files_deleted,
        ));
    }
    query.push_str(" ON CONFLICT (from_id, to_id) DO NOTHING");
    println!("Inserting {} rows into diff_num_files...", num_files.len());
    let diesel_query = diesel::sql_query(query);
    conn.execute(diesel_query)?;
    Ok(())
}

#[derive(Serialize, Deserialize, QueryableByName, Debug, Clone)]
struct NumLines {
    #[sql_type = "diesel::sql_types::BigInt"]
    from_id: i64,
    #[sql_type = "diesel::sql_types::BigInt"]
    to_id: i64,
    #[sql_type = "diesel::sql_types::BigInt"]
    num_lines_added: i64,
    #[sql_type = "diesel::sql_types::BigInt"]
    num_lines_deleted: i64,
}

fn num_lines(
    conn: &mut DbConnection,
    diffs: &Vec<DiffAnalysis>,
) -> Result<(), diesel::result::Error> {
    let mut num_lines: Vec<NumLines> = vec![];
    for diff in diffs {
        let mut num_lines_added = 0;
        let mut num_lines_deleted = 0;
        match &diff.job_result {
            DiffAnalysisJobResult::Diff(map) => {
                for diff in map.values() {
                    num_lines_added += diff.added as i64;
                    num_lines_deleted += diff.removed as i64;
                }
            }
            _ => continue,
        }
        num_lines.push(NumLines {
            from_id: diff.from_id,
            to_id: diff.to_id,
            num_lines_added,
            num_lines_deleted,
        });
    }

    if num_lines.is_empty() {
        return Ok(());
    }

    // insert into analysis.diff_num_lines
    let query = r#"
        INSERT INTO analysis.diff_num_lines (from_id, to_id, num_lines_added, num_lines_deleted)
        VALUES
        "#;
    let mut query = query.to_string();
    for (i, nl) in num_lines.iter().enumerate() {
        if i > 0 {
            query.push_str(", ");
        }
        query.push_str(&format!(
            "({}, {}, {}, {})",
            nl.from_id, nl.to_id, nl.num_lines_added, nl.num_lines_deleted,
        ));
    }
    query.push_str(" ON CONFLICT (from_id, to_id) DO NOTHING");
    println!("Inserting {} rows into diff_num_lines...", num_lines.len());
    let diesel_query = diesel::sql_query(query);
    conn.execute(diesel_query)?;
    Ok(())
}

#[derive(Serialize, Deserialize, QueryableByName, Debug, Clone)]
struct ExtCount {
    #[sql_type = "diesel::sql_types::Text"]
    ext: String,
    #[sql_type = "diesel::sql_types::BigInt"]
    count: i64,
}

fn get_extension(path: &str) -> Option<&str> {
    let split: Vec<&str> = path.split('.').collect();
    if split.len() > 1 {
        Some(split[split.len() - 1])
    } else {
        None
    }
}

fn ext_count(
    conn: &mut DbConnection,
    diffs: &Vec<DiffAnalysis>,
) -> Result<(), diesel::result::Error> {
    let mut ext_counts: HashMap<String, i64> = HashMap::new();
    for diff in diffs {
        match &diff.job_result {
            DiffAnalysisJobResult::Diff(map) => {
                for diff in map.keys() {
                    let ext = match get_extension(diff) {
                        Some(e) => e,
                        None => continue,
                    };
                    let count = ext_counts.entry(ext.to_string()).or_insert(0);
                    *count += 1;
                }
            }
            _ => continue,
        }
    }

    if ext_counts.is_empty() {
        return Ok(());
    }

    // insert into analysis.diff_ext_count
    let query = r#"
        INSERT INTO analysis.diff_ext_count (ext, count)
        VALUES
        "#;
    let mut query = query.to_string();
    for (i, (ext, count)) in ext_counts.iter().enumerate() {
        if i > 0 {
            query.push_str(", ");
        }
        query.push_str(&format!("('{}', {})", ext, count));
    }
    // on conflict, update count += excluded.count
    query.push_str(
        " ON CONFLICT (ext) DO UPDATE SET count = analysis.diff_ext_count.count + excluded.count",
    );
    println!("Inserting {} rows into diff_ext_count...", ext_counts.len());
    let diesel_query = diesel::sql_query(query);
    conn.execute(diesel_query)?;
    Ok(())
}

#[derive(Serialize, Deserialize, QueryableByName, Debug, Clone)]
struct ChangedFile {
    #[sql_type = "diesel::sql_types::BigInt"]
    from_id: i64,
    #[sql_type = "diesel::sql_types::BigInt"]
    to_id: i64,
    #[sql_type = "diesel::sql_types::Bool"]
    did_change_types: bool,
    #[sql_type = "diesel::sql_types::Bool"]
    did_change_code: bool,
}

fn changed_file(
    conn: &mut DbConnection,
    diffs: &Vec<DiffAnalysis>,
) -> Result<(), diesel::result::Error> {
    let mut changed_files: Vec<ChangedFile> = vec![];
    for diff in diffs {
        match &diff.job_result {
            DiffAnalysisJobResult::Diff(map) => {
                let mut did_change_types = false;
                let mut did_change_code = false;
                for (path, diff) in map.iter() {
                    // if file ends with .d.ts and it has been changed, then it's a type change
                    // if file ends with .js, .jsx, .ts, .tsx and it has been changed, then it's a code change
                    let ext = match get_extension(path) {
                        Some(e) => e,
                        None => continue,
                    };

                    // NOTE: can't use ext for .d.ts
                    if (path.ends_with(".d.ts") || path.ends_with(".d.tsx"))
                        && (diff.added > 0 || diff.removed > 0)
                    {
                        did_change_types = true;
                    } else if (ext == "js" || ext == "jsx" || ext == "ts" || ext == "tsx")
                        && (diff.added > 0 || diff.removed > 0)
                    {
                        did_change_code = true;
                    }

                    if did_change_types && did_change_code {
                        break;
                    }
                }
                changed_files.push(ChangedFile {
                    from_id: diff.from_id,
                    to_id: diff.to_id,
                    did_change_types,
                    did_change_code,
                });
            }
            _ => continue,
        }
    }

    if changed_files.is_empty() {
        return Ok(());
    }

    // insert into analysis.diff_changed_files
    let query = r#"
        INSERT INTO analysis.diff_changed_files (from_id, to_id, did_change_types, did_change_code)
        VALUES
        "#;
    let mut query = query.to_string();
    for (i, cf) in changed_files.iter().enumerate() {
        if i > 0 {
            query.push_str(", ");
        }
        query.push_str(&format!(
            "({}, {}, {}, {})",
            cf.from_id, cf.to_id, cf.did_change_types, cf.did_change_code,
        ));
    }
    query.push_str(" ON CONFLICT (from_id, to_id) DO NOTHING");
    println!(
        "Inserting {} rows into diff_changed_files...",
        changed_files.len()
    );
    let diesel_query = diesel::sql_query(query);
    conn.execute(diesel_query)?;
    Ok(())
}
