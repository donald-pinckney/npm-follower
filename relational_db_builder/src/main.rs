use std::any::Any;
use std::time::{Duration, Instant};

use chrono::Utc;
use metrics_logging::{
    MetricsLoggerTrait, RelationalDbBatchCompleteMetrics, RelationalDbEndSessionMetrics,
    RelationalDbPanicMetrics, RelationalDbStartSessionMetrics,
};
use postgres_db::connection::{DbConnection, DbConnectionInTransaction};
use postgres_db::diff_log;
use postgres_db::diff_log::DiffLogEntry;
use postgres_db::internal_state;

use utils::check_no_concurrent_processes;

const PAGE_SIZE: i64 = 1024;

fn main() {
    check_no_concurrent_processes("relational_db_builder");

    let mut conn = DbConnection::connect();
    let mut metrics_logger = metrics_logging::new_metrics_logger(false);

    let mut processed_up_to_seq =
        internal_state::query_relational_processed_seq(&mut conn).unwrap_or(0);

    println!("Initial queries:");
    println!("query_num_changes_after_seq");
    let num_changes_total =
        postgres_db::change_log::query_num_changes_after_seq(processed_up_to_seq, &mut conn);
    println!("query_num_diff_entries_after_seq");
    let num_entries_total =
        diff_log::query_num_diff_entries_after_seq(processed_up_to_seq, &mut conn);
    println!("Initial queries DONE!");

    let mut num_changes_so_far = 0;
    let mut num_entries_so_far = 0;

    let session_start_time = Utc::now();
    let session_start_seq_exclusive = processed_up_to_seq;

    metrics_logger.log_relational_db_builder_start_session(RelationalDbStartSessionMetrics {
        session_start_time,
        session_start_seq_exclusive,
        session_num_seqs: num_changes_total,
        session_num_diff_entries: num_entries_total,
    });

    // TODO: Extract this into function (duplicated in download_queuer/src/main.rs)
    loop {
        let batch_start = Instant::now();
        let batch_start_time = Utc::now();

        let entries =
            diff_log::query_diff_entries_after_seq(processed_up_to_seq, PAGE_SIZE, &mut conn);

        let read_duration = batch_start.elapsed();

        let num_entries = entries.len() as i64;
        num_entries_so_far += num_entries;
        if num_entries == 0 {
            break;
        }

        let first_seq_in_page = entries.first().unwrap().seq;
        let last_seq_in_page = entries.last().unwrap().seq;

        let process_entries_metrics = conn
            .run_psql_transaction(|mut trans_conn| {
                match process_entries(&mut trans_conn, entries) {
                    Ok(res) => {
                        internal_state::set_relational_processed_seq(
                            last_seq_in_page,
                            &mut trans_conn,
                        );
                        Ok(res)
                    }
                    Err(err) => {
                        metrics_logger.log_relational_db_builder_panic(RelationalDbPanicMetrics {
                            panic_time: Utc::now(),
                            panic_on_seq_id: err.seq,
                            panic_on_diff_entry_id: err.entry_id,
                            panic_message: err.message,
                        });
                        std::panic::resume_unwind(err.err);
                    }
                }
            })
            .unwrap();

        let num_changes = process_entries_metrics.num_changes;
        num_changes_so_far += num_changes;

        processed_up_to_seq = last_seq_in_page;

        let batch_total_duration = batch_start.elapsed();

        metrics_logger.log_relational_db_builder_batch_complete_metrics(
            RelationalDbBatchCompleteMetrics {
                batch_start_time,
                batch_start_seq_inclusive: first_seq_in_page,
                batch_end_seq_inclusive: last_seq_in_page,
                batch_num_processed_seqs: num_changes,
                batch_num_processed_diff_entries: num_entries,
                batch_bytes_read: process_entries_metrics.read_bytes as i64,
                batch_bytes_written: process_entries_metrics.write_bytes as i64,
                batch_reading_duration: chrono::Duration::from_std(read_duration).unwrap(),
                batch_writing_duration: chrono::Duration::from_std(
                    process_entries_metrics.write_duration,
                )
                .unwrap(),
                batch_total_duration: chrono::Duration::from_std(batch_total_duration).unwrap(),
                session_num_seqs: num_changes_total,
                session_num_diff_entries: num_entries_total,
                session_num_seqs_processed_so_far: num_changes_so_far,
                session_num_diff_entries_processed_so_far: num_entries_so_far,
                session_start_time,
            },
        );

        if num_changes < PAGE_SIZE {
            break;
        }
    }

    let session_end_time = Utc::now();
    let session_total_duration = session_end_time - session_start_time;

    metrics_logger.log_relational_db_builder_end_session(RelationalDbEndSessionMetrics {
        session_start_time,
        session_start_seq_exclusive,
        session_num_seqs: num_changes_so_far,
        session_num_diff_entries: num_entries_so_far,
        session_end_time,
        session_end_seq_inclusive: processed_up_to_seq,
        session_total_duration,
    })
}

pub fn process_entries(
    conn: &mut DbConnectionInTransaction,
    entries: Vec<DiffLogEntry>,
) -> Result<ProcessEntrySuccessMetrics, ProcessEntryError> {
    Ok(ProcessEntrySuccessMetrics {
        num_changes: 0,
        read_bytes: 0,
        write_bytes: 0,
        write_duration: Duration::from_secs(0),
    })
    // todo!()

    // let mut state_manager = DiffStateManager::new();
    // let mut new_diff_entries: Vec<NewDiffLogEntryWithHash> = Vec::new();

    // let mut read_bytes = 0;
    // let mut write_bytes = 0;

    // for c in changes {
    //     let seq = c.seq;

    //     let result = panic::catch_unwind(AssertUnwindSafe(|| {
    //         process_change(conn, c, &mut state_manager, &mut new_diff_entries)
    //     }));

    //     let (rb, wb) = match result {
    //         Err(err) => {
    //             let err_message = format!("Failed on seq: {}.\n:{:?}", seq, err);
    //             return Result::Err(ProcessChangeError {
    //                 seq,
    //                 message: err_message,
    //                 err,
    //             });
    //         }
    //         Ok(r) => r,
    //     };

    //     read_bytes += rb;
    //     write_bytes += wb;
    // }

    // let write_start = Instant::now();

    // state_manager.flush_to_db(conn);
    // diff_log::insert_diff_log_entries(
    //     new_diff_entries.into_iter().map(|x| x.entry).collect(),
    //     conn,
    // );

    // let write_duration = write_start.elapsed();

    // Ok(ProcessChangeSuccessMetrics {
    //     read_bytes,
    //     write_bytes,
    //     write_duration,
    // })
}

#[derive(Debug)]
pub struct ProcessEntryError {
    pub seq: i64,
    pub entry_id: i64,
    pub message: String,
    pub err: Box<dyn Any + Send>,
}

pub struct ProcessEntrySuccessMetrics {
    pub num_changes: i64,
    pub read_bytes: usize,
    pub write_bytes: usize,
    pub write_duration: Duration,
}
