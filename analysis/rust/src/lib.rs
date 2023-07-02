use diesel::QueryableByName;
use postgres_db::{
    connection::{DbConnection, QueryRunner},
    diff_analysis::{DiffAnalysis, DiffAnalysisJobResult},
};

use serde::{Deserialize, Serialize};

pub mod relational_db_accessor;

pub fn process_diff_analysis(
    mut conn: DbConnection,
    chunk_size: i64,
    writer: fn(
        conn: &mut DbConnection,
        diffs: &Vec<DiffAnalysis>,
    ) -> Result<(), diesel::result::Error>,
) {
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

        writer(&mut conn, &table).expect("Failed to write to file");

        println!("Wrote {} rows in {:?}!", len_table, time.elapsed());
    }
}
