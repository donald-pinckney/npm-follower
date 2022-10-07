use chrono::NaiveDate;
use postgres_db::packages::QueriedPackage;
use serde::Deserialize;
use std::collections::HashMap;

/// Performs a regular query in npm download metrics api for each package given
pub async fn query_npm_metrics(
    pkg: &QueriedPackage,
    lbound: &NaiveDate,
    rbound: &NaiveDate,
) -> Result<ApiResult, ApiError> {
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

        println!(
            "Querying {} from {} to {}",
            pkg.name, rel_lbound, rel_rbound
        );

        let query = format!(
            "https://api.npmjs.org/downloads/range/{}:{}/{}",
            rel_lbound, rel_rbound, pkg.name
        );

        // TODO: do actual good error handling, instead of this garbage
        let resp = reqwest::get(&query).await?;
        if resp.status() == 429 {
            return Err(ApiError::RateLimit);
        }
        let text = resp.text().await?;
        let result: ApiResult = match serde_json::from_str(&text) {
            Ok(result) => result,
            Err(e) => {
                // get the error message from the response
                let json_map = serde_json::from_str::<HashMap<String, String>>(&text)?;
                let error = match json_map.get("error") {
                    Some(error) => error,
                    None => return Err(ApiError::Other(e.to_string())),
                };
                return Err(ApiError::Other(error.to_string()));
            }
        };

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
pub async fn bulkquery_npm_metrics(
    pkgs: &Vec<QueriedPackage>,
) -> Result<BulkApiResult, Box<dyn std::error::Error>> {
    todo!("bulkquery_npm_metrics")
}

pub type BulkApiResult = HashMap<String, ApiResult>;

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
    DoesNotExist(String), // where String is the package name
    Other(String),        // where String is the error message
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
            ApiError::RateLimit => write!(f, "rate limited"),
            ApiError::DoesNotExist(name) => write!(f, "package {} does not exist", name),
            ApiError::Other(msg) => write!(f, "other error: {}", msg),
        }
    }
}

impl std::error::Error for ApiError {}
