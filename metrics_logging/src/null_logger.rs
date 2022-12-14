use crate::{
    DiffLogBatchCompleteMetrics, DiffLogEndSessionMetrics, DiffLogPanicMetrics,
    DiffLogStartSessionMetrics, MetricsLoggerTrait, RelationalDbBatchCompleteMetrics,
    RelationalDbEndSessionMetrics, RelationalDbPanicMetrics, RelationalDbStartSessionMetrics,
};

pub(crate) struct NullLogger;

impl NullLogger {
    pub(crate) fn new() -> NullLogger {
        NullLogger
    }
}

impl MetricsLoggerTrait for NullLogger {
    fn log_diff_log_builder_batch_complete_metrics(
        &mut self,
        _metrics: DiffLogBatchCompleteMetrics,
    ) {
    }

    fn log_diff_log_builder_start_session(&mut self, _metrics: DiffLogStartSessionMetrics) {}

    fn log_diff_log_builder_end_session(&mut self, _metrics: DiffLogEndSessionMetrics) {}

    fn log_diff_log_builder_panic(&mut self, _metrics: DiffLogPanicMetrics) {}

    fn log_relational_db_builder_batch_complete_metrics(
        &mut self,
        _metrics: RelationalDbBatchCompleteMetrics,
    ) {
    }

    fn log_relational_db_builder_start_session(
        &mut self,
        _metrics: RelationalDbStartSessionMetrics,
    ) {
    }

    fn log_relational_db_builder_end_session(&mut self, _metrics: RelationalDbEndSessionMetrics) {}

    fn log_relational_db_builder_panic(&mut self, _metrics: RelationalDbPanicMetrics) {}
}
