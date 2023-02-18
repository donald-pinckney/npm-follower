use diesel::prelude::*;
use rust_sql_analysis::process_diff_analysis;
use serde::{Deserialize, Serialize};

use postgres_db::{
    connection::{DbConnection, QueryRunner},
    diff_analysis::{DiffAnalysis, DiffAnalysisJobResult},
};

fn print_usage_exit(argv0: &str) -> ! {
    eprintln!("Usage: {} chunk_size", argv0);
    std::process::exit(1);
}

fn main() {
    utils::check_no_concurrent_processes("process_num_lines");
    dotenvy::dotenv().ok();
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() < 2 {
        print_usage_exit(args[0].as_str());
    }
    let chunk_size = args[1].parse::<i64>().unwrap();
    let conn: DbConnection = DbConnection::connect();
    process_diff_analysis(conn, chunk_size, num_lines);
}

#[derive(Serialize, Deserialize, QueryableByName, Debug, Clone)]
struct NumLines {
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    from_id: i64,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    to_id: i64,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    num_lines_added: i64,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
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
