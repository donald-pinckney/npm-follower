use std::io::{self, Write};
use std::time::Instant;

use diff_log_builder::process_changes;
use postgres_db::change_log;

use postgres_db::internal_state;

use utils::check_no_concurrent_processes;

const PAGE_SIZE: i64 = 1024;

fn main() {
    check_no_concurrent_processes("diff_log_builder");

    let conn = postgres_db::connect();

    let mut processed_up_to = internal_state::query_diff_log_processed_seq(&conn).unwrap_or(0);

    let num_changes_total = change_log::query_num_changes_after_seq(processed_up_to, &conn);
    let mut num_changes_so_far = 0;

    // TODO: Extract this into function (duplicated in download_queuer/src/main.rs)
    loop {
        print!(
            "Fetching seq > {}, page size = {} ({:.1}%)",
            processed_up_to,
            PAGE_SIZE,
            100.0 * (num_changes_so_far as f64) / (num_changes_total as f64)
        );
        io::stdout().flush().unwrap();
        let start = Instant::now();

        let changes = change_log::query_changes_after_seq(processed_up_to, PAGE_SIZE, &conn);
        let num_changes = changes.len() as i64;
        num_changes_so_far += num_changes;
        if num_changes == 0 {
            break;
        }

        let last_seq_in_page = changes.last().unwrap().seq;

        let (read_bytes, write_bytes) = conn
            .run_psql_transaction(|| {
                let (read_bytes, write_bytes) = process_changes(&conn, changes);
                internal_state::set_diff_log_processed_seq(last_seq_in_page, &conn);
                Ok((read_bytes, write_bytes))
            })
            .unwrap();

        processed_up_to = last_seq_in_page;

        let duration = start.elapsed();
        println!(
            "  [{} ms]  [{} read]  [{} wrote]",
            duration.as_millis(),
            read_bytes,
            write_bytes
        );

        if num_changes < PAGE_SIZE {
            break;
        }
    }
}
