use diesel::QueryableByName;
use postgres_db::{
    connection::{DbConnection, QueryRunner},
    diff_analysis::{DiffAnalysis, DiffAnalysisJobResult},
};
use serde::{Deserialize, Serialize};

fn print_usage_exit(argv0: &str) -> ! {
    eprintln!("Usage: {} [num_files] chunk_size", argv0);
    std::process::exit(1);
}

fn main() {
    utils::check_no_concurrent_processes("diff_queries");
    dotenvy::dotenv().ok();
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() < 3 {
        print_usage_exit(args[0].as_str());
    }
    let chunk_size = args[2].parse::<i64>().unwrap();
    let mut conn: DbConnection = DbConnection::connect();
    let write_func = match args[1].as_str() {
        "num_files" => num_files,
        _ => {
            print_usage_exit(args[0].as_str());
        }
    };

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
        write_func(&mut conn, table).unwrap();
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
    diffs: Vec<DiffAnalysis>,
) -> Result<(), diesel::result::Error> {
    let mut num_files: Vec<NumFiles> = vec![];
    for diff in diffs {
        let mut num_files_added = 0;
        let mut num_files_removed = 0;
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
                        num_files_removed += 1;
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
            num_files_deleted: num_files_removed,
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
    println!("Inserting {} rows", num_files.len());
    let diesel_query = diesel::sql_query(query);
    conn.execute(diesel_query)?;
    Ok(())
}
