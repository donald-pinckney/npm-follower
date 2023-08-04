use std::collections::HashMap;

use chrono::NaiveDate;
use download_metrics::api::ApiError;
use download_metrics::api::QueryTaskHandle;
use download_metrics::api::API;
use download_metrics::LOWER_BOUND_DATE;
use download_metrics::UPPER_BOUND_DATE;
use postgres_db::download_metrics::QueriedDownloadMetric;
use postgres_db::{download_metrics::DownloadMetric, packages::QueriedPackage, DbConnection};
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
        "update" => update_from_packages(&conn).await,
        _ => {
            eprintln!("Usage: {} <insert|update>", args[0]);
            std::process::exit(1);
        }
    }
}

async fn update_from_packages(conn: &DbConnection) {
    let mut metrics = query_metrics_older_than_a_week(conn);
    let api = API::default();

    while !metrics.is_empty() {
        let mut handles: Vec<(i64, QueryTaskHandle)> = Vec::new();

        // map of [metric id] -> [metric]
        let mut lookup_table: HashMap<i64, DownloadMetric> = HashMap::new();

        for metric in metrics {
            let pkg = postgres_db::packages::query_pkg_by_id(conn, metric.package_id)
                .expect("coulnd't find package from metric's package_id");

            let lower_bound_date = metric.latest_date;

            let metric_id = metric.id;
            lookup_table.insert(metric_id, metric.into());

            let api = api.clone();
            handles.push((
                metric_id,
                api.spawn_query_task(pkg, lower_bound_date, *UPPER_BOUND_DATE),
            ));
        }

        // where i64 is the id of the metric
        let mut metrics_to_upd: Vec<(i64, DownloadMetric)> = Vec::new();

        for (id, handle) in handles {
            let mut metric = match handle.await.unwrap() {
                (Ok(metric), _) => metric,
                (Err(ApiError::RateLimit), _) => {
                    eprintln!("Rate limited! Exiting!");
                    std::process::exit(1);
                }
                (Err(e), _) => {
                    println!("Error: {}", e);
                    continue;
                }
            };

            pretty_print_metric(&metric);

            let mut older_metric = lookup_table.get(&id).unwrap().clone();

            // we check if we got a newer latest_date for the older metric
            if let Some(older_last) = older_metric.download_counts.last() {
                if !metric.download_counts.is_empty()
                    && older_last.date == metric.download_counts[0].date
                {
                    // we remove the last element of the older metric, since it's the same as the
                    // first element of the newer metric, except that the newer metric may have
                    // different counts for that download
                    older_metric.download_counts.pop();
                }
            }

            metric.download_counts = older_metric
                .download_counts
                .into_iter()
                .chain(metric.download_counts.into_iter())
                .collect();
            metric.total_downloads += older_metric.total_downloads;
            metrics_to_upd.push((id, metric));
        }

        conn.run_psql_transaction(|| {
            for (id, metric) in metrics_to_upd {
                postgres_db::download_metrics::update_metric_by_id(conn, id, metric);
            }
            Ok(())
        })
        .expect("couldn't run transaction");

        metrics = query_metrics_older_than_a_week(conn);
    }

    println!("Done updating metrics");
}

/// Queries all metrics older than a week. The query is limited to 128 results.
fn query_metrics_older_than_a_week(conn: &DbConnection) -> Vec<QueriedDownloadMetric> {
    let week_ago = get_a_week_ago(&LOWER_BOUND_DATE, &UPPER_BOUND_DATE) - chrono::Duration::days(7);
    println!("querying metrics older than {}", week_ago);
    postgres_db::download_metrics::query_metric_latest_less_than(conn, week_ago, 128)
}

/// Inserts new download metric rows by using the `packages` table and querying npm
async fn insert_from_packages(conn: &DbConnection) {
    let mut pkg_id = postgres_db::internal_state::query_download_metrics_pkg_seq(conn).unwrap_or(1);

    println!("starting inserting metrics from pkg_id: {}", pkg_id);

    // NOTE: Bulk queries are limited to at most 128 packages at a time and at most 365 days of data.
    //       however, we can't bulk query scoped packages.

    // therefore we run in chunks of 128 packages (+ scoped packages, max 128 too for consistency)

    let api = API::new(6);
    let mut finished = false; // we break the loop if we have no more packages to query
    let mut redo_rl = postgres_db::download_metrics::query_rate_limited_packages(conn);
    while !finished {
        let mut chunk_pkg_id = pkg_id;
        let mut normal_packages = Vec::new();
        // NOTE: for simplicity, we pool rate-limited packages into the scoped packages
        let mut scoped_packages = redo_rl;
        redo_rl = Vec::new();

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
                    if has_normal_metadata(&pkg) {
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

        let mut download_metrics: Vec<(DownloadMetric, QueriedPackage)> = Vec::new();
        let mut scoped_handles: Vec<QueryTaskHandle> = Vec::new();

        // normal packages can be queried in bulk
        let maybe_bulk_handle = {
            if !normal_packages.is_empty() {
                let api = api.clone();
                Some(api.spawn_bulk_query_task(
                    normal_packages,
                    *LOWER_BOUND_DATE,
                    *UPPER_BOUND_DATE,
                ))
            } else {
                None
            }
        };

        // scoped packages need to be handled separately one-by-one
        for pkg in scoped_packages {
            let api = api.clone();
            scoped_handles.push(api.spawn_query_task(pkg, *LOWER_BOUND_DATE, *UPPER_BOUND_DATE));
        }

        for handle in scoped_handles {
            match handle.await.unwrap() {
                (Ok(metric), pkg) => {
                    pretty_print_metric(&metric);
                    download_metrics.push((metric, pkg));
                }
                (Err(ApiError::RateLimit), pkg) => {
                    println!("Rate-limited!");
                    postgres_db::download_metrics::add_rate_limited_package(conn, &pkg);
                    redo_rl.push(pkg);
                    println!(
                        "Retrying {} packages",
                        redo_rl
                            .iter()
                            .map(|p| &p.name)
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
                (Err(ApiError::DoesNotExist), pkg) => {
                    println!("Error: {} does not exist", pkg.name);
                    postgres_db::download_metrics::remove_rate_limited_package(conn, &pkg);
                }
                (Err(e), pkg) => panic!("Error: {} with pkg: {}", e, pkg.name),
            };
        }

        if let Some(bulk_handle) = maybe_bulk_handle {
            match bulk_handle.await.unwrap() {
                (Ok(metrics), pkgs) => {
                    // NOTE: we have to do this whole thing because the result may not have
                    //       the same packages as the query (some packages don't exist)
                    for pkg in pkgs {
                        for metric in &metrics {
                            if metric.package_id == pkg.id {
                                pretty_print_metric(metric);
                                download_metrics.push((metric.clone(), pkg));
                                break;
                            }
                        }
                    }
                }
                (Err(ApiError::RateLimit), pkgs) => {
                    println!("Rate-limited!");
                    postgres_db::download_metrics::add_rate_limited_packages(conn, &pkgs);
                    redo_rl.extend(pkgs);
                    println!(
                        "Retrying {} packages",
                        redo_rl
                            .iter()
                            .map(|p| &p.name)
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
                (Err(e), pkgs) => panic!("Error: {} with pkgs: {:?}", e, pkgs),
            };
        }

        conn.run_psql_transaction(|| {
            let rate_limited = postgres_db::download_metrics::query_rate_limited_packages(conn);
            for (metric, pkg) in download_metrics {
                if !rate_limited.is_empty() && rate_limited.contains(&pkg) {
                    postgres_db::download_metrics::remove_rate_limited_package(conn, &pkg);
                }
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

/// Returns the earliest date that matches a week given the epoch, using the same logic as npm
/// queries.
fn get_a_week_ago(lbound: &chrono::NaiveDate, rbound: &chrono::NaiveDate) -> NaiveDate {
    let delta = chronoutil::RelativeDuration::years(1);
    let mut rel_lbound = *lbound;
    let mut res = *lbound;
    let rule = chronoutil::DateRule::new(rel_lbound + delta, delta);
    for mut rel_rbound in rule {
        if rel_lbound > *rbound {
            break;
        }

        if rel_rbound > *rbound {
            rel_rbound = *rbound;
        }

        res = rel_rbound;
        rel_lbound = rel_rbound + chronoutil::RelativeDuration::days(1);
    }

    // now traverse weeks, until we get a week less than `res`
    let delta = chrono::Duration::weeks(1);
    let mut rel_lbound = *lbound;
    let rbound = res;

    while rel_lbound < rbound {
        rel_lbound += delta;
    }

    rel_lbound - delta
}

fn pretty_print_metric(metric: &DownloadMetric) {
    println!("id: {}", metric.package_id);
    println!("latest: {:?}", metric.latest_date);
    println!("counts:");
    for dl in &metric.download_counts {
        print!("{}: {}, ", dl.date, dl.count);
    }
    println!();
}
