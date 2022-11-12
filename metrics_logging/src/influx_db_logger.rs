use std::{
    sync::mpsc::{self, Sender},
    thread::JoinHandle,
};

use chrono::{Local, Utc};
use influxdb2::{models::DataPoint, Client};
use tokio::runtime::Runtime;

use crate::{
    DiffLogBatchCompleteMetrics, DiffLogEndSessionMetrics, DiffLogPanicMetrics,
    DiffLogStartSessionMetrics, MetricsLoggerTrait, RelationalDbBatchCompleteMetrics,
    RelationalDbEndSessionMetrics, RelationalDbPanicMetrics, RelationalDbStartSessionMetrics,
};

pub(crate) struct InfluxDbLogger {
    write_thread: Option<JoinHandle<()>>,
    write_sender: Option<Sender<DataPoint>>,
    initial_write_count: i64,
}

use futures::stream;

fn sync_write_queue(
    bucket: &str,
    points: Vec<DataPoint>,
    rt: &Runtime,
    conn: &Client,
) -> Vec<DataPoint> {
    let to_write = stream::iter(points.clone());
    match rt.block_on(conn.write(bucket, to_write)) {
        Ok(_) => vec![],
        Err(_) => points,
    }
}

impl InfluxDbLogger {
    pub(crate) fn new(host: String, org: String, token: String, bucket: String) -> InfluxDbLogger {
        let (write_sender, write_receiver) = mpsc::channel();

        let write_thread = std::thread::spawn(move || {
            let conn = Client::new(host, org, token);

            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            let mut write_queue: Vec<DataPoint> = Vec::new();

            loop {
                let msg = write_receiver.recv();
                match msg {
                    Err(_) => {
                        sync_write_queue(&bucket, write_queue, &rt, &conn);
                        break;
                    }
                    Ok(to_write) => {
                        write_queue.push(to_write);

                        if write_queue.len() > 1024 {
                            eprintln!(
                                "[{}] Dropping {} metrics due to error writing to InfluxDB.",
                                write_queue.len() - 1024,
                                Local::now()
                            );
                            write_queue.drain(0..(write_queue.len() - 1024));
                        }
                        write_queue = sync_write_queue(&bucket, write_queue, &rt, &conn);
                    }
                }
            }
        });

        InfluxDbLogger {
            write_thread: Some(write_thread),
            write_sender: Some(write_sender),
            initial_write_count: 0,
        }
    }

    fn write_data_point(&mut self, point: DataPoint) {
        if self.initial_write_count < 5 {
            println!("Sending to InfluxDB: {:?}", point);
            self.initial_write_count += 1;

            if self.initial_write_count == 5 {
                println!("\ngoing silent now, will now only write to InfluxDB...");
            }
        }
        self.write_sender.as_ref().unwrap().send(point).unwrap()
    }
}

impl Drop for InfluxDbLogger {
    fn drop(&mut self) {
        drop(self.write_sender.take());

        if let Some(thread) = self.write_thread.take() {
            thread.join().unwrap();
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

        let p = DataPoint::builder("diff_log_builder_metrics")
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
            .unwrap();

        self.write_data_point(p);
    }

    fn log_diff_log_builder_start_session(&mut self, metrics: DiffLogStartSessionMetrics) {
        let p = DataPoint::builder("diff_log_builder_metrics")
            .tag("event_type", "start_session")
            .field("session_start_time", metrics.session_start_time.to_string())
            .field(
                "session_start_seq_exclusive",
                metrics.session_start_seq_exclusive,
            )
            .field("session_num_seqs", metrics.session_num_seqs)
            .timestamp(metrics.session_start_time.timestamp_nanos())
            .build()
            .unwrap();

        self.write_data_point(p);
    }

    fn log_diff_log_builder_end_session(&mut self, metrics: DiffLogEndSessionMetrics) {
        let p = DataPoint::builder("diff_log_builder_metrics")
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
            .unwrap();

        self.write_data_point(p);
    }

    fn log_diff_log_builder_panic(&mut self, metrics: DiffLogPanicMetrics) {
        let p = DataPoint::builder("diff_log_builder_metrics")
            .tag("event_type", "panic")
            .field("panic_time", metrics.panic_time.to_string())
            .field("panic_on_seq_id", metrics.panic_on_seq_id)
            .field("panic_message", metrics.panic_message)
            .timestamp(metrics.panic_time.timestamp_nanos())
            .build()
            .unwrap();

        self.write_data_point(p);
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
