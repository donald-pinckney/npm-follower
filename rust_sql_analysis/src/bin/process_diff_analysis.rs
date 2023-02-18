use diesel::QueryableByName;
use postgres_db::{
    connection::{DbConnection, QueryRunner},
    diff_analysis::{DiffAnalysis, DiffAnalysisJobResult},
};
use serde::{Deserialize, Serialize};

fn print_usage_exit(argv0: &str) -> ! {
    eprintln!("Usage: {} chunk_size", argv0);
    std::process::exit(1);
}

fn main() {
    utils::check_no_concurrent_processes("process_diff_analysis");
    dotenvy::dotenv().ok();
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() < 2 {
        print_usage_exit(args[0].as_str());
    }
    let chunk_size = args[1].parse::<i64>().unwrap();
    let conn: DbConnection = DbConnection::connect();
    process_diff_analysis(conn, chunk_size);
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

fn get_extension(path: &str) -> Option<&str> {
    let split: Vec<&str> = path.split('.').collect();
    if split.len() > 1 {
        Some(split[split.len() - 1])
    } else {
        None
    }
}

#[derive(Serialize, Deserialize, QueryableByName, Debug, Clone)]
struct ChangedFile {
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    from_id: i64,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    to_id: i64,
    #[diesel(sql_type = diesel::sql_types::Bool)]
    did_change_types: bool,
    #[diesel(sql_type = diesel::sql_types::Bool)]
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
