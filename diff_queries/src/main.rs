use diesel::QueryableByName;
use postgres_db::{
    connection::{DbConnection, QueryRunner},
    diff_analysis::{DiffAnalysis, DiffAnalysisJobResult},
};
use serde::{Deserialize, Serialize};

fn print_usage_exit(argv0: &str) -> ! {
    eprintln!("Usage: {} [num_files]", argv0);
    std::process::exit(1);
}

fn main() {
    utils::check_no_concurrent_processes("diff_queries");
    dotenvy::dotenv().ok();
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() < 2 {
        print_usage_exit(args[0].as_str());
    }
    let mut conn: DbConnection = DbConnection::connect();
    let table = postgres_db::diff_analysis::query_table(&mut conn, None).unwrap();
    match args[1].as_str() {
        "num_files" => num_files(conn, table).unwrap(),
        _ => {
            print_usage_exit(args[0].as_str());
        }
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
    mut conn: DbConnection,
    diffs: Vec<DiffAnalysis>,
) -> Result<(), diesel::result::Error> {
    for diffs in diffs.chunks(1000) {
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

        // insert into analysis.diff_num_files
        let query = r#"
        INSERT INTO analysis.diff_num_files (from_id, to_id, num_files_added, num_files_modified, num_files_deleted)
        VALUES
        "#;
        let mut query = query.to_string();
        for (i, _) in num_files.iter().enumerate() {
            if i > 0 {
                query.push_str(", ");
            }
            query.push_str(&format!(
                "({}, {}, {}, {}, {})",
                &num_files[i].from_id,
                &num_files[i].to_id,
                &num_files[i].num_files_added,
                &num_files[i].num_files_modified,
                &num_files[i].num_files_deleted,
            ));
        }
        query.push_str(" ON CONFLICT (from_id, to_id) DO NOTHING");
        println!("Inserting {} rows", num_files.len());
        let diesel_query = diesel::sql_query(query);
        conn.execute(diesel_query)?;
    }
    Ok(())
}
