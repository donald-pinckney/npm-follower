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
    utils::check_no_concurrent_processes("process_num_files");
    dotenvy::dotenv().ok();
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() < 2 {
        print_usage_exit(args[0].as_str());
    }
    let chunk_size = args[1].parse::<i64>().unwrap();
    let conn: DbConnection = DbConnection::connect();
    process_diff_analysis(conn, chunk_size, num_files);
}

#[derive(Serialize, Deserialize, QueryableByName, Debug, Clone)]
struct NumFiles {
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    from_id: i64,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    to_id: i64,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    num_files_added: i64,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    num_files_modified: i64,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
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
