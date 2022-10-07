use std::sync::Arc;

use chrono::NaiveDate;
use download_metrics::api::query_npm_metrics;
use download_metrics::api::ApiError;
use postgres_db::{
    custom_types::DownloadCount, download_metrics::DownloadMetric, packages::QueriedPackage,
    DbConnection,
};
use tokio::sync::Semaphore;
use utils::check_no_concurrent_processes;

#[tokio::main]
async fn main() {
    check_no_concurrent_processes("download_metrics");
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() != 2 {
        eprintln!("Usage: {} <insert|update>", args[0]);
        std::process::exit(1);
    }
    let conn = postgres_db::connect();

    match args[1].as_str() {
        "insert" => insert_from_packages(&conn).await,
        "update" => todo!("update"),
        _ => {
            eprintln!("Usage: {} <insert|update>", args[0]);
            std::process::exit(1);
        }
    }
}

async fn make_download_metric(
    pkg: &QueriedPackage,
    sem: Arc<Semaphore>,
    lower_bound_date: &NaiveDate,
    upper_bound_date: &NaiveDate,
) -> Result<DownloadMetric, ApiError> {
    let permit = sem.acquire().await.unwrap();
    let result = query_npm_metrics(pkg, lower_bound_date, upper_bound_date).await?;
    drop(permit);

    // we need to convert the results into DownloadMetric, merging daily results
    // into weekly results
    let mut weekly_results: Vec<DownloadCount> = Vec::new();
    let mut i = 0;
    let mut total_downloads = 0;

    loop {
        let mut weekly_count = result.downloads[i].downloads;
        let mut j = i + 1;
        while j < result.downloads.len() && j < i + 7 {
            weekly_count += result.downloads[j].downloads;
            j += 1;
        }

        let date = chrono::NaiveDate::parse_from_str(&result.downloads[i].day, "%Y-%m-%d").unwrap();

        // we set i to j so that we skip the days we already counted
        i = j;

        total_downloads += weekly_count;

        let count = DownloadCount {
            date,
            count: weekly_count,
        };

        // we don't insert zero counts
        if weekly_count > 0 {
            weekly_results.push(count);
        }

        if i >= result.downloads.len() {
            // we still want to know the latest, even if it's zero and we didn't insert it
            let latest = Some(date);
            println!("did package {}", pkg.name);
            return Ok(DownloadMetric::new(
                pkg.id,
                weekly_results,
                total_downloads,
                latest,
            ));
        }
    }
}

/// Inserts new download metric rows by using the `packages` table and querying npm
async fn insert_from_packages(conn: &DbConnection) {
    let mut pkg_id = postgres_db::internal_state::query_download_metrics_pkg_seq(conn).unwrap_or(1);

    let lower_bound_date = chrono::NaiveDate::from_ymd(2015, 1, 10);
    // NOTE: we remove three days because:
    //  1. we remove 1 day beacuse of time zones
    //  2. we remove 1 day because the data of "today" is not yet complete
    //  3. we remove 1 other day because NPM's api only publishes data for "today" the day after
    let upper_bound_date = chrono::Utc::now().date().naive_utc() - chrono::Duration::days(3);

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
            match pkg {
                None => {
                    // could be that ids are not contiguous, so we need to get the next id
                    let next_pkg_id = postgres_db::packages::query_next_pkg_id(conn, chunk_pkg_id);
                    match next_pkg_id {
                        None => {
                            // no more packages to query
                            finished = true;
                            break;
                        }
                        Some(next_pkg_id) => {
                            println!(
                                "No package with id {}, skipping to next id {}",
                                chunk_pkg_id, next_pkg_id
                            );
                            chunk_pkg_id = next_pkg_id;
                        }
                    }
                }
                Some(pkg) => {
                    // TODO: i think? ping donald about it
                    if !pkg.secret && has_normal_metadata(&pkg) {
                        if pkg.name.starts_with('@') {
                            scoped_packages.push(pkg);
                        } else {
                            normal_packages.push(pkg);
                        }
                    }
                    chunk_pkg_id += 1;
                }
            }
        }

        let mut download_metrics: Vec<DownloadMetric> = Vec::new();
        let mut handles = Vec::new();
        let sem = Arc::new(Semaphore::new(3)); // limiting to 3 requests at a time

        // TODO: bulk query, remove chain
        for pkg in normal_packages
            .into_iter()
            .chain(scoped_packages.into_iter())
        {
            let sem = sem.clone();
            handles.push(tokio::spawn(async move {
                make_download_metric(&pkg, sem, &lower_bound_date, &upper_bound_date).await
            }));
        }

        for handle in handles {
            let metric = match handle.await.unwrap() {
                Ok(metric) => metric,
                Err(ApiError::RateLimit) => {
                    eprintln!("Rate limited! Exiting!");
                    std::process::exit(1);
                }
                Err(e) => {
                    println!("Error: {}", e);
                    continue;
                }
            };

            println!("latest: {:?}", metric.latest_date);
            println!("counts:");
            for dl in &metric.download_counts {
                print!("{}: {}, ", dl.date, dl.count);
            }
            println!();
            download_metrics.push(metric);
        }

        conn.run_psql_transaction(|| {
            for metric in download_metrics {
                postgres_db::download_metrics::insert_download_metric(conn, metric);
            }
            postgres_db::internal_state::set_download_metrics_pkg_seq(chunk_pkg_id, conn);
            Ok(())
        })
        .expect("failed to insert download metrics");
        pkg_id = chunk_pkg_id;
    }

    println!("Done, at pkg_id {}", pkg_id);
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

/// Returns true if the given date is a week ago basing ourselves on the current time and the
/// given 0 epoch date
fn is_a_week_ago(date: &chrono::NaiveDate, epoch: &chrono::NaiveDate) -> bool {
    todo!("is_a_week_ago")
}
