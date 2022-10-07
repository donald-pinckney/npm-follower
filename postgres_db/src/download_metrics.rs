use chrono::NaiveDate;

use super::schema::download_metrics;
use crate::{custom_types::DownloadCount, DbConnection};
use diesel::prelude::*;

#[derive(Insertable, Clone, Queryable, Debug)]
#[diesel(table_name = download_metrics)]
pub struct DownloadMetric {
    pub package_id: i64,
    pub download_counts: Vec<DownloadCount>,
    pub total_downloads: i64,
    pub latest_date: NaiveDate,
}

impl DownloadMetric {
    pub fn new(
        package_id: i64,
        download_counts: Vec<DownloadCount>,
        total_downloads: i64,
        latest_date: NaiveDate,
    ) -> DownloadMetric {
        DownloadMetric {
            package_id,
            download_counts,
            total_downloads,
            latest_date,
        }
    }
}

#[derive(Debug, Clone)]
pub struct QueriedDownloadMetric {
    pub id: i64,
    pub package_id: i64,
    pub download_counts: Vec<DownloadCount>,
    pub total_downloads: i64,
    pub latest_date: NaiveDate,
}

impl<ST, SB: diesel::backend::Backend> Queryable<ST, SB> for QueriedDownloadMetric
where
    (i64, i64, Vec<DownloadCount>, i64, NaiveDate): diesel::deserialize::FromSqlRow<ST, SB>,
{
    type Row = (i64, i64, Vec<DownloadCount>, i64, NaiveDate);

    fn build(row: Self::Row) -> Self {
        QueriedDownloadMetric {
            id: row.0,
            package_id: row.1,
            download_counts: row.2,
            total_downloads: row.3,
            latest_date: row.4,
        }
    }
}

impl From<QueriedDownloadMetric> for DownloadMetric {
    fn from(qdm: QueriedDownloadMetric) -> Self {
        DownloadMetric {
            package_id: qdm.package_id,
            download_counts: qdm.download_counts,
            total_downloads: qdm.total_downloads,
            latest_date: qdm.latest_date,
        }
    }
}

pub fn insert_download_metric(conn: &DbConnection, metrics: DownloadMetric) -> i64 {
    use super::schema::download_metrics::dsl::*;

    diesel::insert_into(download_metrics)
        .values(metrics)
        .on_conflict_do_nothing()
        .get_result::<QueriedDownloadMetric>(&conn.conn)
        .unwrap_or_else(|e| panic!("Error inserting download metric, {:?}", e))
        .id
}

pub fn update_metric_by_id(conn: &DbConnection, metric_id: i64, metric: DownloadMetric) {
    use super::schema::download_metrics::dsl::*;

    diesel::update(download_metrics.find(metric_id))
        .set((
            package_id.eq(metric.package_id),
            download_counts.eq(metric.download_counts),
            total_downloads.eq(metric.total_downloads),
            latest_date.eq(metric.latest_date),
        ))
        .execute(&conn.conn)
        .unwrap_or_else(|e| panic!("Error updating download metric, {:?}", e));
}

/// Queries all download metrics with latest date being less than or equal the given date.
/// The query is limited to 1000 results
pub fn query_metric_latest_less_than(
    conn: &DbConnection,
    date: NaiveDate,
) -> Vec<QueriedDownloadMetric> {
    use super::schema::download_metrics::dsl::*;

    download_metrics
        .filter(latest_date.le(date))
        .limit(1000)
        .load::<QueriedDownloadMetric>(&conn.conn)
        .unwrap_or_else(|e| panic!("Error querying download metrics, {:?}", e))
}
