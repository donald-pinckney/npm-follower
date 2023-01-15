use std::collections::HashMap;

use diesel::QueryableByName;
use postgres_db::{
    connection::{DbConnection, QueryRunner},
    custom_types::Semver,
    diff_analysis::{DiffAnalysis, DiffAnalysisJobResult},
};
use serde::{Deserialize, Serialize};

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
    process_diff_all_updates(conn, chunk_size);
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

        println!("Writing {} rows to the file...", table.len());
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
    let total_count = postgres_db::diff_analysis::count_diff_analysis(&mut conn).unwrap();

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
            FROM analysis.all_updates
            ORDER BY from_id, to_id{}
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
        println!(
            "Progress: {:.2}%",
            num_processed as f64 / total_count as f64 * 100.0
        );
        if table.is_empty() {
            break;
        }
        last = table.last().map(|d| (d.from_id, d.to_id));

        println!("Writing {} rows to the file...", table.len());
        let time = std::time::Instant::now();
        let len_table = table.len();

        // insert here queries to write

        println!("Wrote {} rows in {:?}!", len_table, time.elapsed());
    }
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
