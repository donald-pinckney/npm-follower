mod csv_logger;
mod influx_db_logger;
mod null_logger;

use chrono::{DateTime, Duration, Utc};
use dotenv::dotenv;
use serde::Serialize;
use std::env;

pub struct DiffLogBatchCompleteMetrics {
    pub batch_start_time: DateTime<Utc>,
    pub batch_start_seq_inclusive: i64,
    pub batch_end_seq_inclusive: i64,
    pub batch_num_processed_seqs: i64,
    pub batch_bytes_read: i64,
    pub batch_bytes_written: i64,
    pub batch_reading_duration: Duration,
    pub batch_writing_duration: Duration,
    pub batch_total_duration: Duration,
    pub session_num_seqs: i64,
    pub session_num_seqs_processed_so_far: i64,
    pub session_start_time: DateTime<Utc>,
}

pub struct DiffLogStartSessionMetrics {
    pub session_start_time: DateTime<Utc>,
    pub session_start_seq_exclusive: i64,
    pub session_num_seqs: i64,
}

pub struct DiffLogEndSessionMetrics {
    pub session_start_time: DateTime<Utc>,
    pub session_start_seq_exclusive: i64,
    pub session_num_seqs: i64,
    pub session_end_time: DateTime<Utc>,
    pub session_end_seq_inclusive: i64,
    pub session_total_duration: Duration,
}

#[derive(Serialize)]
pub struct DiffLogPanicMetrics {
    pub panic_time: DateTime<Utc>,
    pub panic_on_seq_id: i64,
    pub panic_message: String,
}

pub struct RelationalDbBatchCompleteMetrics {
    pub batch_start_time: DateTime<Utc>,
    pub batch_start_seq_inclusive: i64,
    pub batch_end_seq_inclusive: i64,
    pub batch_num_processed_seqs: i64,
    pub batch_num_processed_diff_entries: i64,
    pub batch_bytes_read: i64,
    pub batch_bytes_written: i64,
    pub batch_reading_duration: Duration,
    pub batch_writing_duration: Duration,
    pub batch_total_duration: Duration,
    pub session_num_seqs: i64,
    pub session_num_diff_entries: i64,
    pub session_num_seqs_processed_so_far: i64,
    pub session_num_diff_entries_processed_so_far: i64,
    pub session_start_time: DateTime<Utc>,
}

pub struct RelationalDbStartSessionMetrics {
    pub session_start_time: DateTime<Utc>,
    pub session_start_seq_exclusive: i64,
    pub session_num_seqs: i64,
    pub session_num_diff_entries: i64,
}

pub struct RelationalDbEndSessionMetrics {
    pub session_start_time: DateTime<Utc>,
    pub session_start_seq_exclusive: i64,
    pub session_num_seqs: i64,
    pub session_num_diff_entries: i64,
    pub session_end_time: DateTime<Utc>,
    pub session_end_seq_inclusive: i64,
    pub session_total_duration: Duration,
}

#[derive(Serialize)]
pub struct RelationalDbPanicMetrics {
    pub panic_time: DateTime<Utc>,
    pub panic_on_seq_id: i64,
    pub panic_on_diff_entry_id: i64,
    pub panic_message: String,
}

pub trait MetricsLoggerTrait {
    fn log_diff_log_builder_batch_complete_metrics(&mut self, metrics: DiffLogBatchCompleteMetrics);
    fn log_diff_log_builder_start_session(&mut self, metrics: DiffLogStartSessionMetrics);
    fn log_diff_log_builder_end_session(&mut self, metrics: DiffLogEndSessionMetrics);
    fn log_diff_log_builder_panic(&mut self, metrics: DiffLogPanicMetrics);

    fn log_relational_db_builder_batch_complete_metrics(
        &mut self,
        metrics: RelationalDbBatchCompleteMetrics,
    );
    fn log_relational_db_builder_start_session(&mut self, metrics: RelationalDbStartSessionMetrics);
    fn log_relational_db_builder_end_session(&mut self, metrics: RelationalDbEndSessionMetrics);
    fn log_relational_db_builder_panic(&mut self, metrics: RelationalDbPanicMetrics);
}

pub struct MetricsLogger(Box<dyn MetricsLoggerTrait + Send>);

impl MetricsLoggerTrait for MetricsLogger {
    fn log_diff_log_builder_batch_complete_metrics(
        &mut self,
        metrics: DiffLogBatchCompleteMetrics,
    ) {
        self.0.log_diff_log_builder_batch_complete_metrics(metrics)
    }

    fn log_diff_log_builder_start_session(&mut self, metrics: DiffLogStartSessionMetrics) {
        self.0.log_diff_log_builder_start_session(metrics)
    }

    fn log_diff_log_builder_end_session(&mut self, metrics: DiffLogEndSessionMetrics) {
        self.0.log_diff_log_builder_end_session(metrics)
    }

    fn log_diff_log_builder_panic(&mut self, metrics: DiffLogPanicMetrics) {
        self.0.log_diff_log_builder_panic(metrics)
    }

    fn log_relational_db_builder_batch_complete_metrics(
        &mut self,
        metrics: RelationalDbBatchCompleteMetrics,
    ) {
        self.0
            .log_relational_db_builder_batch_complete_metrics(metrics)
    }

    fn log_relational_db_builder_start_session(
        &mut self,
        metrics: RelationalDbStartSessionMetrics,
    ) {
        self.0.log_relational_db_builder_start_session(metrics)
    }

    fn log_relational_db_builder_end_session(&mut self, metrics: RelationalDbEndSessionMetrics) {
        self.0.log_relational_db_builder_end_session(metrics)
    }

    fn log_relational_db_builder_panic(&mut self, metrics: RelationalDbPanicMetrics) {
        self.0.log_relational_db_builder_panic(metrics)
    }
}

pub fn new_metrics_logger(testing_mode: bool) -> MetricsLogger {
    if testing_mode {
        MetricsLogger(Box::new(null_logger::NullLogger::new()))
    } else {
        dotenv().expect("failed to load .env");

        let enable_influx: bool = env::var("ENABLE_INFLUX_DB_LOGGING")
            .unwrap()
            .parse()
            .unwrap();

        if enable_influx {
            let host = env::var("INFLUX_DB_HOST").unwrap();
            let org = env::var("INFLUX_DB_ORG").unwrap();
            let bucket = env::var("INFLUX_DB_BUCKET").unwrap();

            dotenv::from_filename(".secret.env").expect("failed to load .secret.env. To setup InfluxDB logging, run:\necho \"export INFLUX_DB_TOKEN=<TYPE API TOKEN HERE>\" > .secret.env\n\nOr to disable InfluxDB, set ENABLE_INFLUX_DB_LOGGING=false in .env");

            let token = env::var("INFLUX_DB_TOKEN").unwrap();
            MetricsLogger(Box::new(influx_db_logger::InfluxDbLogger::new(
                host, org, token, bucket,
            )))
        } else {
            MetricsLogger(Box::new(csv_logger::CsvLogger::new()))
        }
    }
}
