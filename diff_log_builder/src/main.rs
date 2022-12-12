use std::time::Instant;

use chrono::Utc;
use diff_log_builder::process_changes;
use metrics_logging::{
    DiffLogBatchCompleteMetrics, DiffLogEndSessionMetrics, DiffLogPanicMetrics,
    DiffLogStartSessionMetrics, MetricsLoggerTrait,
};
use postgres_db::change_log;

use postgres_db::connection::DbConnection;
use postgres_db::internal_state;

use utils::check_no_concurrent_processes;

const PAGE_SIZE: i64 = 1024;

fn main() {
    check_no_concurrent_processes("diff_log_builder");

    let mut conn = DbConnection::connect();
    let mut metrics_logger = metrics_logging::new_metrics_logger(false);

    let mut processed_up_to = internal_state::query_diff_log_processed_seq(&mut conn).unwrap_or(0);

    let num_changes_total = change_log::query_num_changes_after_seq(processed_up_to, &mut conn);
    let mut num_changes_so_far = 0;

    let session_start_time = Utc::now();
    let session_start_seq_exclusive = processed_up_to;

    metrics_logger.log_diff_log_builder_start_session(DiffLogStartSessionMetrics {
        session_start_time,
        session_start_seq_exclusive,
        session_num_seqs: num_changes_total,
    });

    // TODO: Extract this into function (duplicated in download_queuer/src/main.rs)
    loop {
        let batch_start = Instant::now();
        let batch_start_time = Utc::now();

        let changes = change_log::query_changes_after_seq(processed_up_to, PAGE_SIZE, &mut conn);

        let read_duration = batch_start.elapsed();

        let num_changes = changes.len() as i64;
        num_changes_so_far += num_changes;
        if num_changes == 0 {
            break;
        }

        let first_seq_in_page = changes.first().unwrap().seq;
        let last_seq_in_page = changes.last().unwrap().seq;

        let process_changes_metrics = conn
            .run_psql_transaction(|mut trans_conn| {
                match process_changes(&mut trans_conn, changes) {
                    Ok(res) => {
                        internal_state::set_diff_log_processed_seq(
                            last_seq_in_page,
                            &mut trans_conn,
                        );
                        Ok((res, true))
                    }
                    Err(err) => {
                        metrics_logger.log_diff_log_builder_panic(DiffLogPanicMetrics {
                            panic_time: Utc::now(),
                            panic_on_seq_id: err.seq,
                            panic_message: err.message,
                        });
                        std::panic::resume_unwind(err.err);
                    }
                }
            })
            .unwrap();

        processed_up_to = last_seq_in_page;

        let batch_total_duration = batch_start.elapsed();

        metrics_logger.log_diff_log_builder_batch_complete_metrics(DiffLogBatchCompleteMetrics {
            batch_start_time,
            batch_start_seq_inclusive: first_seq_in_page,
            batch_end_seq_inclusive: last_seq_in_page,
            batch_num_processed_seqs: num_changes,
            batch_bytes_read: process_changes_metrics.read_bytes as i64,
            batch_bytes_written: process_changes_metrics.write_bytes as i64,
            batch_reading_duration: chrono::Duration::from_std(read_duration).unwrap(),
            batch_writing_duration: chrono::Duration::from_std(
                process_changes_metrics.write_duration,
            )
            .unwrap(),
            batch_total_duration: chrono::Duration::from_std(batch_total_duration).unwrap(),
            session_num_seqs: num_changes_total,
            session_num_seqs_processed_so_far: num_changes_so_far,
            session_start_time,
        });

        if num_changes < PAGE_SIZE {
            break;
        }
    }

    let session_end_time = Utc::now();
    let session_total_duration = session_end_time - session_start_time;

    metrics_logger.log_diff_log_builder_end_session(DiffLogEndSessionMetrics {
        session_start_time,
        session_end_time,
        session_start_seq_exclusive,
        session_end_seq_inclusive: processed_up_to,
        session_num_seqs: num_changes_so_far,
        session_total_duration,
    })
}
