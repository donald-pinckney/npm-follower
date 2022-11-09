use chrono::Utc;
use influxdb2::{models::DataPoint, Client};
use tokio::runtime::Runtime;

use crate::{
    DiffLogBatchCompleteMetrics, DiffLogEndSessionMetrics, DiffLogPanicMetrics,
    DiffLogStartSessionMetrics, MetricsLoggerTrait, RelationalDbBatchCompleteMetrics,
    RelationalDbEndSessionMetrics, RelationalDbPanicMetrics, RelationalDbStartSessionMetrics,
};

pub(crate) struct InfluxDbLogger {
    conn: Client,
    bucket: String,
    rt: Runtime,
}

use futures::stream;

impl InfluxDbLogger {
    pub(crate) fn new(host: String, org: String, token: String, bucket: String) -> InfluxDbLogger {
        InfluxDbLogger {
            conn: Client::new(host, org, token),
            bucket,
            rt: tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap(),
        }
    }
}

struct DiffLogBatchCompleteMetricsInflux {
    batch_start_time: String,
    batch_end_time: String,
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
    session_start_time: String,
}

impl MetricsLoggerTrait for InfluxDbLogger {
    fn log_diff_log_builder_batch_complete_metrics(
        &mut self,
        metrics_tmp: DiffLogBatchCompleteMetrics,
    ) {
        let time_now = Utc::now();

        let metrics = DiffLogBatchCompleteMetricsInflux {
            batch_start_time: metrics_tmp.batch_start_time.to_string(),
            batch_end_time: time_now.to_string(),
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
            session_start_time: metrics_tmp.session_start_time.to_string(),
        };

        let p = vec![DataPoint::builder("diff_log_builder_metrics")
            .tag("event_type", "batch_complete")
            .field("batch_start_time", metrics.batch_start_time)
            .field("batch_end_time", metrics.batch_end_time)
            .field(
                "batch_start_seq_inclusive",
                metrics.batch_start_seq_inclusive,
            )
            .field("batch_end_seq_inclusive", metrics.batch_end_seq_inclusive)
            .field("batch_num_processed_seqs", metrics.batch_num_processed_seqs)
            .field("batch_bytes_read", metrics.batch_bytes_read)
            .field("batch_bytes_written", metrics.batch_bytes_written)
            .field(
                "batch_reading_duration_secs",
                metrics.batch_reading_duration_secs,
            )
            .field(
                "batch_writing_duration_secs",
                metrics.batch_writing_duration_secs,
            )
            .field(
                "batch_total_duration_secs",
                metrics.batch_total_duration_secs,
            )
            .field("session_num_seqs", metrics.session_num_seqs)
            .field(
                "session_num_seqs_processed_so_far",
                metrics.session_num_seqs_processed_so_far,
            )
            .field("session_start_time", metrics.session_start_time)
            .timestamp(time_now.timestamp_nanos())
            .build()
            .unwrap()];

        self.rt
            .block_on(self.conn.write(&self.bucket, stream::iter(p)))
            .unwrap()
    }

    fn log_diff_log_builder_start_session(&mut self, metrics: DiffLogStartSessionMetrics) {
        let p = vec![DataPoint::builder("diff_log_builder_metrics")
            .tag("event_type", "start_session")
            .field("session_start_time", metrics.session_start_time.to_string())
            .field(
                "session_start_seq_exclusive",
                metrics.session_start_seq_exclusive,
            )
            .field("session_num_seqs", metrics.session_num_seqs)
            .timestamp(metrics.session_start_time.timestamp_nanos())
            .build()
            .unwrap()];

        self.rt
            .block_on(self.conn.write(&self.bucket, stream::iter(p)))
            .unwrap()
    }

    fn log_diff_log_builder_end_session(&mut self, metrics: DiffLogEndSessionMetrics) {
        let p = vec![DataPoint::builder("diff_log_builder_metrics")
            .tag("event_type", "start_session")
            .field("session_start_time", metrics.session_start_time.to_string())
            .field(
                "session_start_seq_exclusive",
                metrics.session_start_seq_exclusive,
            )
            .field("session_num_seqs", metrics.session_num_seqs)
            .field("session_end_time", metrics.session_end_time.to_string())
            .field(
                "session_end_seq_inclusive",
                metrics.session_end_seq_inclusive,
            )
            .field(
                "session_total_duration",
                metrics
                    .session_total_duration
                    .to_std()
                    .unwrap()
                    .as_secs_f64(),
            )
            .timestamp(metrics.session_end_time.timestamp_nanos())
            .build()
            .unwrap()];

        self.rt
            .block_on(self.conn.write(&self.bucket, stream::iter(p)))
            .unwrap()
    }

    fn log_diff_log_builder_panic(&mut self, metrics: DiffLogPanicMetrics) {
        let p = vec![DataPoint::builder("diff_log_builder_metrics")
            .tag("event_type", "panic")
            .field("panic_time", metrics.panic_time.to_string())
            .field("panic_on_seq_id", metrics.panic_on_seq_id)
            .field("panic_message", metrics.panic_message)
            .timestamp(metrics.panic_time.timestamp_nanos())
            .build()
            .unwrap()];

        self.rt
            .block_on(self.conn.write(&self.bucket, stream::iter(p)))
            .unwrap()
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
