use kdam::{tqdm, BarExt};
use std::any::Any;
use std::collections::{HashSet, VecDeque};
use std::panic::{self, AssertUnwindSafe};
use std::time::{Duration, Instant};

use chrono::Utc;
use metrics_logging::{
    MetricsLoggerTrait, RelationalDbBatchCompleteMetrics, RelationalDbEndSessionMetrics,
    RelationalDbPanicMetrics, RelationalDbStartSessionMetrics,
};
use postgres_db::connection::{DbConnection, QueryRunner};
use postgres_db::diff_log::DiffLogEntry;
use postgres_db::diff_log::{self, DiffLogInstruction};
use postgres_db::internal_state;

use relational_db_builder::EntryProcessor;
use utils::check_no_concurrent_processes;

const TARGET_PAGE_SIZE_NUM_ENTRIES: i64 = 32000;

fn main() {
    check_no_concurrent_processes("relational_db_builder");

    let mut conn = DbConnection::connect();
    let mut metrics_logger = metrics_logging::new_metrics_logger(false);

    let mut processed_up_to_seq =
        internal_state::query_relational_processed_seq(&mut conn).unwrap_or(0);

    println!("Initial queries:");
    println!("query_num_changes_after_seq");
    let num_changes_total = postgres_db::diff_log::query_num_changes_after_seq_in_diff_log(
        processed_up_to_seq,
        &mut conn,
    );
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

    let mut entry_processor = EntryProcessor::new();

    let mut batches_pb = tqdm!(
        total = num_entries_total.try_into().unwrap(),
        desc = "All entries",
        position = 0
    );

    // Initially we start the page size at 256
    let mut current_page_size_seqs = 256;
    let history_size = 4;
    let mut batch_size_history: VecDeque<(i64, i64)> = VecDeque::new();

    let mut num_loops = 0;

    // TODO: Extract this into function (duplicated in download_queuer/src/main.rs)
    loop {
        let batch_start = Instant::now();
        let batch_start_time = Utc::now();

        if num_loops <= 3 {
            println!(
                "Starting read from diff_log, page size = {}",
                current_page_size_seqs
            );
        }

        let entries = diff_log::query_diff_entries_after_seq(
            processed_up_to_seq,
            current_page_size_seqs,
            &mut conn,
        );

        if num_loops <= 3 {
            println!(
                "Completed read from diff_log, page size = {}",
                current_page_size_seqs
            );
        }

        let unique_seqs: HashSet<_> = entries.iter().map(|entry| entry.seq).collect();
        let num_changes = unique_seqs.len() as i64;

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
                match process_entries(&mut entry_processor, &mut trans_conn, entries) {
                    Ok(res) => {
                        internal_state::set_relational_processed_seq(
                            last_seq_in_page,
                            &mut trans_conn,
                        );
                        Ok((res, true))
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

        batches_pb.update(num_entries.try_into().unwrap());

        batch_size_history.push_front((num_entries, num_changes));
        if batch_size_history.len() == history_size + 1 {
            batch_size_history.pop_back();
        }

        if batch_size_history.len() == history_size {
            let entries_sum = batch_size_history
                .iter()
                .fold(0, |acc, (n_entries, _n_changes)| acc + n_entries);

            let changes_sum = batch_size_history
                .iter()
                .fold(0, |acc, (_n_entries, n_changes)| acc + n_changes);

            let ratio_est = (entries_sum as f64) / (changes_sum as f64);
            let page_size_seq_est = ((TARGET_PAGE_SIZE_NUM_ENTRIES as f64) / ratio_est) as i64;
            current_page_size_seqs = page_size_seq_est.max(16).min(16384);
        }

        num_loops += 1;
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

pub fn process_entries<R>(
    processor: &mut EntryProcessor,
    conn: &mut R,
    entries: Vec<DiffLogEntry>,
) -> Result<ProcessEntrySuccessMetrics, ProcessEntryError>
where
    R: QueryRunner,
{
    let mut read_bytes = 0;

    let entries_iter = tqdm!(entries.into_iter(), desc = "Current batch", position = 1);
    // let entries_iter = entries.into_iter();
    for e in entries_iter {
        read_bytes += entry_num_bytes(&e);
        let seq = e.seq;
        let entry_id = e.id;
        let package = e.package_name;
        let instr = e.instr;
        panic::catch_unwind(AssertUnwindSafe(|| {
            processor.process_entry(conn, package, instr, seq, entry_id)
        }))
        .map_err(|err| {
            let message = panic_as_string(&err);
            ProcessEntryError {
                seq,
                entry_id,
                message,
                err,
            }
        })?;
    }

    processor.flush_caches(conn);

    Ok(ProcessEntrySuccessMetrics {
        read_bytes,
        write_bytes: 0,
        write_duration: Duration::from_secs(0),
    })
}

fn entry_num_bytes(e: &DiffLogEntry) -> usize {
    match &e.instr {
        DiffLogInstruction::CreatePackage(pack) | DiffLogInstruction::UpdatePackage(pack) => {
            serde_json::to_vec(pack).unwrap().len()
        }
        DiffLogInstruction::CreateVersion(_, vpack)
        | DiffLogInstruction::UpdateVersion(_, vpack) => serde_json::to_vec(vpack).unwrap().len(),
        _ => 0,
    }
}

#[derive(Debug)]
pub struct ProcessEntryError {
    pub seq: i64,
    pub entry_id: i64,
    pub message: String,
    pub err: Box<dyn Any + Send>,
}

pub struct ProcessEntrySuccessMetrics {
    pub read_bytes: usize,
    pub write_bytes: usize,
    pub write_duration: Duration,
}

fn panic_as_string(p: &Box<dyn Any + Send>) -> String {
    fn swap<A, B>(x: Result<A, B>) -> Result<B, A> {
        match x {
            Ok(a) => Err(a),
            Err(b) => Ok(b),
        }
    }

    fn panic_as_string_result_err(p: &Box<dyn Any + Send>) -> Result<(), String> {
        swap(
            p.as_ref()
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .ok_or(p),
        )?;

        swap(
            p.as_ref()
                .downcast_ref::<String>()
                .map(|s| s.to_string())
                .ok_or(p),
        )?;

        Err("Unknown panic type".to_string())
    }

    panic_as_string_result_err(p).unwrap_err()
}
