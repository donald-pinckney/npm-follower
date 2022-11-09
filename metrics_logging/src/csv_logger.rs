use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
};

use serde::Serialize;

use chrono::{DateTime, Utc};

use crate::{
    DiffLogBatchCompleteMetrics, DiffLogEndSessionMetrics, DiffLogPanicMetrics,
    DiffLogStartSessionMetrics, MetricsLoggerTrait, RelationalDbBatchCompleteMetrics,
    RelationalDbEndSessionMetrics, RelationalDbPanicMetrics, RelationalDbStartSessionMetrics,
};

pub struct CsvLogger {
    writers: HashMap<String, csv::Writer<File>>,
}

impl CsvLogger {
    pub(crate) fn new() -> CsvLogger {
        CsvLogger {
            writers: HashMap::new(),
        }
    }
}

fn open_csv_file<P: AsRef<Path>>(path: P) -> csv::Writer<File> {
    if path.as_ref().exists() {
        let append_file = OpenOptions::new().append(true).open(path).unwrap();
        csv::WriterBuilder::new()
            .has_headers(false)
            .from_writer(append_file)
    } else {
        let parent_dir = path.as_ref().parent().unwrap();
        if !parent_dir.exists() {
            std::fs::create_dir_all(parent_dir).unwrap();
        }

        let new_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap();
        csv::WriterBuilder::new()
            .has_headers(true)
            .from_writer(new_file)
    }
}

impl CsvLogger {
    fn get_csv_file(&mut self, name: &'static str) -> &mut csv::Writer<File> {
        self.writers.entry(name.to_string()).or_insert_with(|| {
            let mut path: PathBuf = ["logs", name].iter().collect();
            path.set_extension("csv");
            open_csv_file(path)
        })
    }
}

#[derive(Serialize)]
struct DiffLogBatchCompleteMetricsCsv {
    batch_start_time: DateTime<Utc>,
    batch_end_time: DateTime<Utc>,
    batch_start_seq_inclusive: i64,
    batch_end_seq_inclusive: i64,
    batch_num_processed_seqs: i64,
    batch_bytes_read: i64,
    batch_bytes_written: i64,
    batch_reading_duration_secs: f64,
    batch_writing_duration_secs: f64,
    batch_total_duration_secs: f64,
    session_num_seqs: i64,
    session_num_seqs_processed_so_far: i64,
    session_start_time: DateTime<Utc>,
}

#[derive(Serialize)]
struct DiffLogSessionMetricsCsv {
    event_type: String,
    session_start_time: DateTime<Utc>,
    session_start_seq_exclusive: i64,
    session_num_seqs: i64,
    session_end_time: Option<DateTime<Utc>>,
    session_end_seq_inclusive: Option<i64>,
    session_total_duration_secs: Option<f64>,
}

impl MetricsLoggerTrait for CsvLogger {
    fn log_diff_log_builder_batch_complete_metrics(
        &mut self,
        metrics_tmp: DiffLogBatchCompleteMetrics,
    ) {
        let time_now = Utc::now();
        let metrics = DiffLogBatchCompleteMetricsCsv {
            batch_start_time: metrics_tmp.batch_start_time,
            batch_end_time: time_now,
            batch_start_seq_inclusive: metrics_tmp.batch_start_seq_inclusive,
            batch_end_seq_inclusive: metrics_tmp.batch_end_seq_inclusive,
            batch_num_processed_seqs: metrics_tmp.batch_num_processed_seqs,
            batch_bytes_read: metrics_tmp.batch_bytes_read,
            batch_bytes_written: metrics_tmp.batch_bytes_written,
            batch_reading_duration_secs: metrics_tmp
                .batch_reading_duration
                .to_std()
                .unwrap()
                .as_secs_f64(),
            batch_writing_duration_secs: metrics_tmp
                .batch_writing_duration
                .to_std()
                .unwrap()
                .as_secs_f64(),
            batch_total_duration_secs: metrics_tmp
                .batch_total_duration
                .to_std()
                .unwrap()
                .as_secs_f64(),
            session_num_seqs: metrics_tmp.session_num_seqs,
            session_num_seqs_processed_so_far: metrics_tmp.session_num_seqs_processed_so_far,
            session_start_time: metrics_tmp.session_start_time,
        };

        let csv = self.get_csv_file("diff_log_builder_batch_metrics");
        csv.serialize(metrics).unwrap();
        csv.flush().unwrap();
    }

    fn log_diff_log_builder_start_session(&mut self, start_metrics: DiffLogStartSessionMetrics) {
        let metrics = DiffLogSessionMetricsCsv {
            event_type: "start_session".to_owned(),
            session_start_time: start_metrics.session_start_time,
            session_start_seq_exclusive: start_metrics.session_start_seq_exclusive,
            session_num_seqs: start_metrics.session_num_seqs,
            session_end_time: None,
            session_end_seq_inclusive: None,
            session_total_duration_secs: None,
        };

        let csv = self.get_csv_file("diff_log_builder_session_metrics");
        csv.serialize(metrics).unwrap();
        csv.flush().unwrap();
    }

    fn log_diff_log_builder_end_session(&mut self, end_metrics: DiffLogEndSessionMetrics) {
        let metrics = DiffLogSessionMetricsCsv {
            event_type: "end_session".to_owned(),
            session_start_time: end_metrics.session_start_time,
            session_start_seq_exclusive: end_metrics.session_start_seq_exclusive,
            session_num_seqs: end_metrics.session_num_seqs,
            session_end_time: Some(end_metrics.session_end_time),
            session_end_seq_inclusive: Some(end_metrics.session_end_seq_inclusive),
            session_total_duration_secs: Some(
                end_metrics
                    .session_total_duration
                    .to_std()
                    .unwrap()
                    .as_secs_f64(),
            ),
        };

        let csv = self.get_csv_file("diff_log_builder_session_metrics");
        csv.serialize(metrics).unwrap();
        csv.flush().unwrap();
    }

    fn log_diff_log_builder_panic(&mut self, metrics: DiffLogPanicMetrics) {
        let csv = &mut self.get_csv_file("diff_log_builder_panic_metrics");
        csv.serialize(metrics).unwrap();
        csv.flush().unwrap();
    }

    fn log_relational_db_builder_batch_complete_metrics(
        &mut self,
        metrics: RelationalDbBatchCompleteMetrics,
    ) {
        todo!()
    }

    fn log_relational_db_builder_start_session(
        &mut self,
        metrics: RelationalDbStartSessionMetrics,
    ) {
        todo!()
    }

    fn log_relational_db_builder_end_session(&mut self, metrics: RelationalDbEndSessionMetrics) {
        todo!()
    }

    fn log_relational_db_builder_panic(&mut self, metrics: RelationalDbPanicMetrics) {
        todo!()
    }
}
