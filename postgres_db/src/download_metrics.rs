use chrono::NaiveDate;

use super::schema::download_metrics;
use crate::{custom_types::DownloadCount, DbConnection};
use diesel::prelude::*;

#[derive(Insertable, Clone, Queryable, Debug)]
#[diesel(table_name = download_metrics)]
pub struct DownloadMetric {
    pub package_id: i64,
    pub download_counts: Vec<DownloadCount>,
    pub latest_date: Option<NaiveDate>,
    pub total_downloads: i64,
}

impl DownloadMetric {
    pub fn new(
        package_id: i64,
        download_counts: Vec<DownloadCount>,
        latest_date: Option<NaiveDate>,
        total_downloads: i64,
    ) -> DownloadMetric {
        DownloadMetric {
            package_id,
            download_counts,
            latest_date,
            total_downloads,
        }
    }
}

#[derive(Debug, Clone)]
pub struct QueriedDownloadMetric {
    pub id: i64,
    pub package_id: i64,
    pub download_counts: Vec<DownloadCount>,
    pub latest_date: Option<NaiveDate>,
    pub total_downloads: i64,
}

impl<ST, SB: diesel::backend::Backend> Queryable<ST, SB> for QueriedDownloadMetric
where
    (i64, i64, Vec<DownloadCount>, Option<NaiveDate>, i64): diesel::deserialize::FromSqlRow<ST, SB>,
{
    type Row = (i64, i64, Vec<DownloadCount>, Option<NaiveDate>, i64);

    fn build(row: Self::Row) -> Self {
        QueriedDownloadMetric {
            id: row.0,
            package_id: row.1,
            download_counts: row.2,
            latest_date: row.3,
            total_downloads: row.4,
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
