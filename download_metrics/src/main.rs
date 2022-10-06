use std::collections::HashMap;

use chrono::NaiveDate;
use postgres_db::{
    custom_types::DownloadCount, download_metrics::DownloadMetric, packages::QueriedPackage,
    DbConnection,
};
use serde::Deserialize;
use utils::check_no_concurrent_processes;

#[tokio::main]
async fn main() {
    check_no_concurrent_processes("download_metrics");
    let conn = postgres_db::connect();
    insert_from_packages(&conn).await;
}

/// Performs a regular query in npm download metrics api for each package given
async fn query_npm_metrics(
    pkg: &QueriedPackage,
    lbound: &NaiveDate,
    rbound: &NaiveDate,
) -> Result<ApiResult, Box<dyn std::error::Error>> {
    let delta = chronoutil::RelativeDuration::years(1);
    // we are going to merge the results of multiple queries into one
    let mut api_result = None;
    // we can only query 365 days at a time, so we must split the query into multiple requests
    let mut rel_lbound = *lbound;
    let rule = chronoutil::DateRule::new(rel_lbound + delta, delta);
    for rel_rbound in rule {
        if rel_rbound > *rbound {
            break;
        }

        let query = format!(
            "https://api.npmjs.org/downloads/range/{}:{}/{}",
            rel_lbound, rel_rbound, pkg.name
        );

        // TODO: handle errors
        let resp = reqwest::get(&query).await?.text().await?;
        println!("body: {}", resp);
        let result: ApiResult = serde_json::from_str(&resp)?;

        if api_result.is_none() {
            api_result = Some(result);
        } else {
            // merge the results
            let mut new_api_result = api_result.unwrap();
            for dl in result.downloads {
                new_api_result.downloads.push(dl);
            }
            new_api_result.end = result.end;
            api_result = Some(new_api_result);
        }

        rel_lbound = rel_rbound + chronoutil::RelativeDuration::days(1);
    }
    Ok(api_result.unwrap())
}

/// Performs a bulk query in npm download metrics api for each package given
async fn bulkquery_npm_metrics(
    pkgs: &Vec<QueriedPackage>,
) -> Result<BulkApiResult, Box<dyn std::error::Error>> {
    todo!("bulkquery_npm_metrics")
}

/// Inserts new download metric rows by using the `packages` table and querying npm
async fn insert_from_packages(conn: &DbConnection) {
    let mut pkg_id = postgres_db::internal_state::query_download_metrics_pkg_seq(conn).unwrap_or(1);

    let lower_bound_date = chrono::NaiveDate::from_ymd(2015, 1, 10);
    let upper_bound_date = chrono::Utc::now().date().naive_utc();

    println!("starting inserting metrics from pkg_id: {}", pkg_id);

    // NOTE: Bulk queries are limited to at most 128 packages at a time and at most 365 days of data.
    //       however, we can't bulk query scoped packages.

    // therefore we run in chunks of 128 packages (+ scoped packages, max 128 too for consistency)

    let mut finished = false; // we break the loop if we have no more packages to query
    while !finished {
        let mut chunk_pkg_id = pkg_id;
        let mut normal_packages = Vec::new();
        let mut scoped_packages = Vec::new(); // TODO: do scoped packages.

        while normal_packages.len() < 128 && scoped_packages.len() < 128 {
            let pkg = postgres_db::packages::query_pkg_by_id(conn, chunk_pkg_id);
            if pkg.is_none() {
                finished = true;
                break;
            }
            let pkg = pkg.unwrap();
            if !pkg.secret && has_normal_metadata(&pkg) {
                // TODO: i think? ping donald about it
                if pkg.name.starts_with('@') {
                    scoped_packages.push(pkg);
                } else {
                    normal_packages.push(pkg);
                }
            }
            chunk_pkg_id += 1;
        }

        let mut download_metrics: Vec<DownloadMetric> = Vec::new();

        // TODO: bulk query, remove chain
        for pkg in normal_packages
            .into_iter()
            .chain(scoped_packages.into_iter())
        {
            let result = match query_npm_metrics(&pkg, &lower_bound_date, &upper_bound_date).await {
                Ok(result) => result,
                Err(e) => {
                    eprintln!("Error querying npm api: {}", e);
                    continue;
                }
            };
            // we need to convert the results into DownloadMetric, merging daily results
            // into weekly results

            let mut weekly_results: Vec<DownloadCount> = Vec::new();

            let mut i = 0;
            while i < result.downloads.len() {
                let mut weekly_count = result.downloads[i].downloads;
                let mut j = i + 1;
                while j < result.downloads.len() && j < i + 7 {
                    weekly_count += result.downloads[j].downloads;
                    j += 1;
                }
                i = j;

                let date = chrono::NaiveDate::parse_from_str(
                    &result.downloads.last().unwrap().day,
                    "%Y-%m-%d",
                )
                .unwrap();

                // we don't insert zero counts
                if weekly_count > 0 {
                    weekly_results.push(DownloadCount {
                        date,
                        count: weekly_count,
                    });
                }
            }

            let latest = weekly_results.last().unwrap().date;
            let metric = DownloadMetric::new(pkg.id, weekly_results, Some(latest));
            download_metrics.push(metric);
        }

        for metric in download_metrics {
            println!("inserting metric for pkg_id: {}", metric.package_id);
            println!("latest: {:?}", metric.latest_date);
            println!("metrics: {:?}", metric.download_counts);
        }
    }

    println!("Done, at pkg_id {}", pkg_id);
}

type BulkApiResult = HashMap<String, ApiResult>;

#[derive(Deserialize, Debug, Clone)]
pub struct ApiResult {
    downloads: Vec<ApiResultDownload>,
    end: String,
    package: String,
    start: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ApiResultDownload {
    day: String,
    downloads: i64,
}

/// Helper to check if a package has normal metadata
fn has_normal_metadata(pkg: &QueriedPackage) -> bool {
    use postgres_db::custom_types::PackageMetadata;
    matches!(
        pkg.metadata,
        PackageMetadata::Normal {
            dist_tag_latest_version: _,
            created: _,
            modified: _,
            other_dist_tags: _,
        }
    )
}
