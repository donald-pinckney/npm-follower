use std::collections::HashSet;

use chrono::NaiveDate;
use redis::Commands;

use super::schema::download_metrics;
use crate::{connection::QueryRunner, custom_types::DownloadCount, packages::Package};
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

#[derive(Debug, Clone, Queryable)]
pub struct QueriedDownloadMetric {
    pub id: i64,
    pub package_id: i64,
    pub download_counts: Vec<DownloadCount>,
    pub total_downloads: i64,
    pub latest_date: NaiveDate,
}

// impl<ST, SB: diesel::backend::Backend> Queryable<ST, SB> for QueriedDownloadMetric
// where
//     (i64, i64, Vec<DownloadCount>, i64, NaiveDate): diesel::deserialize::FromSqlRow<ST, SB>,
// {
//     type Row = (i64, i64, Vec<DownloadCount>, i64, NaiveDate);

//     fn build(row: Self::Row) -> Self {
//         QueriedDownloadMetric {
//             id: row.0,
//             package_id: row.1,
//             download_counts: row.2,
//             total_downloads: row.3,
//             latest_date: row.4,
//         }
//     }
// }

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

pub fn insert_download_metric<R: QueryRunner>(conn: &mut R, metrics: DownloadMetric) -> i64 {
    use super::schema::download_metrics::dsl::*;

    conn.get_result::<_, QueriedDownloadMetric>(
        diesel::insert_into(download_metrics)
            .values(metrics)
            .on_conflict_do_nothing(),
    )
    .unwrap_or_else(|e| panic!("Error inserting download metric, {:?}", e))
    .id
}

pub fn update_metric_by_id<R: QueryRunner>(conn: &mut R, metric_id: i64, metric: DownloadMetric) {
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
/// The query is limited to the given limit.
pub fn query_metric_latest_less_than<R: QueryRunner>(
    conn: &mut R,
    date: NaiveDate,
    limit: i64,
) -> Vec<QueriedDownloadMetric> {
    use super::schema::download_metrics::dsl::*;

    conn.load::<_, QueriedDownloadMetric>(
        download_metrics.filter(latest_date.le(date)).limit(limit),
    )
    .unwrap_or_else(|e| panic!("Error querying download metrics, {:?}", e))
}

/// Queries redis for the list of packages that have been rate-limited
pub fn query_rate_limited_packages<R: QueryRunner>(conn: &mut R) -> Vec<Package> {
    let mut con = conn.get_redis();

    let data: HashSet<String> = con.smembers("rate-limited").unwrap();

    data.into_iter()
        .map(|s| serde_json::from_str::<Package>(&s).unwrap())
        .collect()
}

/// Removes a package from the rate-limited packages set in redis
pub fn remove_rate_limited_package<R: QueryRunner>(conn: &mut R, package: &Package) {
    let mut con = conn.get_redis();

    let _: () = con
        .srem("rate-limited", serde_json::to_string(package).unwrap())
        .unwrap();
}

/// Removes a list of packages from the rate-limited packages set in redis
pub fn remove_rate_limited_packages<R: QueryRunner>(conn: &mut R, package: &Vec<Package>) {
    let mut con = conn.get_redis();

    for p in package {
        let _: () = con
            .srem("rate-limited", serde_json::to_string(p).unwrap())
            .unwrap();
    }
}

/// Adds a package to the set of rate-limited packages
pub fn add_rate_limited_package<R: QueryRunner>(conn: &mut R, package: &Package) {
    let mut con = conn.get_redis();

    let data = serde_json::to_string(&package).unwrap();

    let _: () = con.sadd("rate-limited", data).unwrap();
}

/// Adds a list of packages to the set of rate-limited packages
pub fn add_rate_limited_packages<R: QueryRunner>(conn: &mut R, packages: &[Package]) {
    let mut con = conn.get_redis();

    let data: Vec<String> = packages
        .iter()
        .map(|p| serde_json::to_string(&p).unwrap())
        .collect();

    for d in data {
        let _: () = con.sadd("rate-limited", d).unwrap();
    }
}
