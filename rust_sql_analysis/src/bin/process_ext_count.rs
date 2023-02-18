use std::collections::HashMap;

use postgres_db::{
    connection::{DbConnection, QueryRunner},
    diff_analysis::{DiffAnalysis, DiffAnalysisJobResult},
};

fn print_usage_exit(argv0: &str) -> ! {
    eprintln!("Usage: {} chunk_size", argv0);
    std::process::exit(1);
}

fn main() {
    utils::check_no_concurrent_processes("process_ext_count");
    dotenvy::dotenv().ok();
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() < 2 {
        print_usage_exit(args[0].as_str());
    }
    let chunk_size = args[1].parse::<i64>().unwrap();
    let conn: DbConnection = DbConnection::connect();
    process_ext_count(conn, chunk_size);
}

fn process_ext_count(mut conn: DbConnection, chunk_size: i64) {
    todo!(); // Should this be like the process_diff_all_updates skeleton, or the process_diff_analysis skeleton?
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
