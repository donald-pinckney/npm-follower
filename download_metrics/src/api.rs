use chrono::NaiveDate;
use postgres_db::{download_metrics::DownloadMetric, packages::Package};
use serde::Deserialize;
use std::{collections::HashMap, sync::Arc};
use tokio::{
    sync::{Mutex, Semaphore},
    task::JoinHandle,
};

use crate::make_download_metric;

/// API wrapper that handles the rate limiting
#[derive(Debug, Clone)]
pub struct API {
    pub pool: Arc<Semaphore>,
    pub pool_size: u32,
    pub rl_lock: Arc<Mutex<()>>,
    pub client: reqwest::Client,
}

pub type QueryTaskHandle = JoinHandle<(Result<DownloadMetric, ApiError>, Package)>;
pub type BulkQueryTaskHandle = JoinHandle<(Result<Vec<DownloadMetric>, ApiError>, Vec<Package>)>;

impl API {
    pub fn new(pool_size: u32) -> API {
        API {
            pool: Arc::new(Semaphore::new(pool_size as usize)),
            pool_size,
            rl_lock: Arc::new(Mutex::new(())),
            client: reqwest::Client::new(),
        }
    }

    pub fn spawn_bulk_query_task(
        self,
        pkgs: Vec<Package>,
        lbound: chrono::NaiveDate,
        rbound: chrono::NaiveDate,
    ) -> BulkQueryTaskHandle {
        tokio::spawn(async move {
            let thunk = async {
                let api_result = self.bulkquery_npm_metrics(&pkgs, &lbound, &rbound).await?;
                let mut metrics = Vec::new();
                for (pkg_name, result) in api_result {
                    if let Some(result) = result {
                        let pkg = pkgs.iter().find(|p| p.name == pkg_name).unwrap(); // yeah, this is bad
                        metrics.push(make_download_metric(pkg, &result).await?);
                    }
                }
                Ok(metrics)
            };
            (thunk.await, pkgs)
        })
    }

    pub fn spawn_query_task(
        self,
        pkg: Package,
        lbound: chrono::NaiveDate,
        rbound: chrono::NaiveDate,
    ) -> QueryTaskHandle {
        tokio::spawn(async move {
            let thunk = async {
                let api_result = self.query_npm_metrics(&pkg, &lbound, &rbound).await?;
                make_download_metric(&pkg, &api_result).await
            };
            (thunk.await, pkg)
        })
    }

    /// Sends a query with reqwest, taking account of the rate limit pool.
    async fn send_query(&self, query: &str) -> Result<reqwest::Response, ApiError> {
        {
            self.rl_lock.lock().await;
        }
        let permit = self.pool.acquire().await.unwrap();
        let resp = self.client.get(query).send().await?;
        drop(permit);
        Ok(resp)
    }

    /// When we are getting rate limited, this function is called, and it handles the sleep.
    /// The idea is to get only one thread to sleep and all the others to return.
    async fn handle_rate_limit(&self) {
        let lock = match self.rl_lock.try_lock() {
            Ok(l) => l,
            Err(_) => return,
        };
        let time = std::time::Duration::from_secs(1200);
        println!(
            "Rate-limit hit, sleeping for {} minutes",
            (time.as_secs() as f64) / 60.0
        );
        tokio::time::sleep(time).await;
        drop(lock);
    }

    /// Abstraction to remove duplicate code between the bulk and single query functions.
    async fn query_abstraction<T, R: for<'a> Deserialize<'a>, M>(
        self,
        thing_to_query: &T,
        formatter: fn(&T) -> String,
        merger: M,
        lbound: &NaiveDate,
        rbound: &NaiveDate,
    ) -> Result<R, ApiError>
    where
        M: Fn(&mut R, R),
    {
        let delta = chronoutil::RelativeDuration::years(1);
        // we are going to merge the results of multiple queries into one
        let mut api_result = None;
        // we can only query 365 days at a time, so we must split the query into multiple requests
        let mut rel_lbound = *lbound;
        let rule = chronoutil::DateRule::new(rel_lbound + delta, delta);
        for mut rel_rbound in rule {
            if rel_lbound > *rbound {
                break;
            }

            if rel_rbound > *rbound {
                // we must not query past the rbound
                rel_rbound = *rbound;
            }

            let query_thing = formatter(thing_to_query);

            println!(
                "Querying {} from {} to {}",
                query_thing, rel_lbound, rel_rbound
            );

            let query = format!(
                "https://api.npmjs.org/downloads/range/{}:{}/{}",
                rel_lbound, rel_rbound, query_thing
            );

            let resp = self.send_query(&query).await?;

            if resp.status() == 429 {
                self.handle_rate_limit().await;
                return Err(ApiError::RateLimit);
            }
            let text = resp.text().await?;

            let result: R = parse_resp(text)?;

            if api_result.is_none() {
                api_result = Some(result);
            } else {
                let mut new_api_result = api_result.unwrap();
                merger(&mut new_api_result, result);
                api_result = Some(new_api_result);
            }

            rel_lbound = rel_rbound + chronoutil::RelativeDuration::days(1);
        }
        api_result.ok_or_else(|| panic!("api_result is None"))
    }

    /// Performs a regular query in npm download metrics api for each package given
    async fn query_npm_metrics(
        self,
        pkg: &Package,
        lbound: &NaiveDate,
        rbound: &NaiveDate,
    ) -> Result<ApiResult, ApiError> {
        fn formatter(pkg: &Package) -> String {
            pkg.name.clone()
        }
        fn merger(api_result: &mut ApiResult, result: ApiResult) {
            for dl in result.downloads {
                api_result.downloads.push(dl);
            }
            api_result.end = result.end;
        }
        self.query_abstraction(pkg, formatter, merger, lbound, rbound)
            .await
    }

    /// Performs a bulk query in npm download metrics api for each package given
    /// Can only handle 128 packages at a time, and can't query scoped packages
    async fn bulkquery_npm_metrics(
        self,
        pkgs: &Vec<Package>,
        lbound: &NaiveDate,
        rbound: &NaiveDate,
    ) -> Result<BulkApiResult, ApiError> {
        assert!(pkgs.len() <= 128);

        #[allow(clippy::ptr_arg)]
        fn formatter(pkgs: &Vec<Package>) -> String {
            pkgs.iter()
                .map(|pkg| pkg.name.to_string())
                .collect::<Vec<String>>()
                .join(",")
        }

        let merger = |api_result: &mut BulkApiResult, result: BulkApiResult| {
            // not_founds is just for printing
            let mut not_founds = String::new();
            for pkg in pkgs {
                let pkg_name = pkg.name.to_string();
                let pkg_res = match result.get(&pkg_name) {
                    Some(Some(p)) => p,
                    _ => {
                        not_founds.push_str(&pkg_name);
                        not_founds.push_str(", ");
                        continue;
                    }
                };

                // here we handle the merging, which is a bit more complicated
                // than the regular merging due to the Optional type
                let new_pkg_res_opt = api_result.get_mut(&pkg_name).unwrap();
                let mut new_pkg_res = new_pkg_res_opt.take().unwrap();

                for dl in &pkg_res.downloads {
                    new_pkg_res.downloads.push(dl.clone());
                }

                new_pkg_res.end = pkg_res.end.clone();
                *new_pkg_res_opt = Some(new_pkg_res);
            }
            if !not_founds.is_empty() {
                println!("Not found: {}", not_founds);
            }
        };
        self.query_abstraction(pkgs, formatter, merger, lbound, rbound)
            .await
    }
}

impl Default for API {
    fn default() -> Self {
        Self::new(3)
    }
}

fn parse_resp<T: for<'a> Deserialize<'a>>(text: String) -> Result<T, ApiError> {
    Ok(match serde_json::from_str(&text) {
        Ok(result) => result,
        Err(e) => {
            // get the error message from the response
            let json_map = serde_json::from_str::<HashMap<String, String>>(&text)?;
            let error = match json_map.get("error") {
                Some(error) => error,
                None => return Err(ApiError::Other(e.to_string())),
            };
            if error.contains("not found") {
                return Err(ApiError::DoesNotExist);
            }
            return Err(ApiError::Other(error.to_string()));
        }
    })
}

pub type BulkApiResult = HashMap<String, Option<ApiResult>>;

#[derive(Deserialize, Debug, Clone)]
pub struct ApiResult {
    pub downloads: Vec<ApiResultDownload>,
    pub end: String,
    pub package: String,
    pub start: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ApiResultDownload {
    pub day: String,
    pub downloads: i64,
}

#[derive(Debug)]
pub enum ApiError {
    Reqwest(reqwest::Error),
    Serde(serde_json::Error),
    Io(std::io::Error),
    DoesNotExist,
    Other(String), // where String is the error message
    RateLimit,
}

impl From<reqwest::Error> for ApiError {
    fn from(err: reqwest::Error) -> Self {
        ApiError::Reqwest(err)
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(err: serde_json::Error) -> Self {
        ApiError::Serde(err)
    }
}

impl From<std::io::Error> for ApiError {
    fn from(err: std::io::Error) -> Self {
        ApiError::Io(err)
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::Reqwest(err) => write!(f, "reqwest error: {}", err),
            ApiError::Serde(err) => write!(f, "serde error: {}", err),
            ApiError::Io(err) => write!(f, "io error: {}", err),
            ApiError::RateLimit => write!(f, "rate-limited"),
            ApiError::DoesNotExist => write!(f, "package does not exist"),
            ApiError::Other(msg) => write!(f, "other error: {}", msg),
        }
    }
}

impl std::error::Error for ApiError {}
