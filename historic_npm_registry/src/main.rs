use std::collections::BTreeMap;
use std::str::FromStr;
use std::sync::Arc;

use chrono::DateTime;
use chrono::Utc;
use moka::future::Cache;
use postgres_db::custom_types::Semver;
use postgres_db::packument::AllVersionPackuments;
use postgres_db::packument::VersionOnlyPackument;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use serde_json::Map;
use serde_json::Value;
use warp::http::StatusCode;
use warp::reply;
use warp::Filter;
use warp::Rejection;
use warp::Reply;

type NpmCache = Cache<String, Option<Arc<AllVersionPackuments>>>;

fn restrict_time(v: &AllVersionPackuments, t: DateTime<Utc>) -> AllVersionPackuments {
    v.clone()
}

async fn request_package_from_npm(
    full_name: &str,
    client: ClientWithMiddleware,
) -> Option<AllVersionPackuments> {
    println!("hitting NPM for: {}", full_name);

    let packument_doc = client
        .get(format!("https://registry.npmjs.org/{}", full_name))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();

    let packument_doc = match packument_doc {
        Value::Object(o) => o,
        _ => panic!("non-object packument"),
    };

    let (_name, pkg_doc, versions) =
        diff_log_builder::deserialize_packument_doc(packument_doc, None, None)
            .expect("failed to parse packument");

    if !pkg_doc.is_normal() {
        return None;
    }

    Some(versions)
}

async fn lookup_package(
    t: DateTime<Utc>,
    full_name: &str,
    client: ClientWithMiddleware,
    cache: NpmCache,
) -> Option<AllVersionPackuments> {
    println!("looking up: {}", full_name);
    if let Some(cache_hit) = cache.get(full_name) {
        cache_hit.map(|x| restrict_time(&x, t))
    } else {
        let npm_response = request_package_from_npm(full_name, client).await;
        let npm_response = npm_response.map(Arc::new);

        cache
            .insert(full_name.to_owned(), npm_response.clone())
            .await;
        npm_response.map(|x| restrict_time(&x, t))
    }
}

fn serialize_packument_in_npm_format(
    package_name: &str,
    versions: Option<AllVersionPackuments>,
) -> Value {
    if versions.is_none() {
        let mut m = Map::new();
        m.insert("error".to_owned(), Value::String("Not found".to_owned()));
        return Value::Object(m);
    }

    let versions = versions.unwrap();

    todo!()
}

async fn handle_request(
    t_str_url_encoded: String,
    scope: Option<String>,
    name: String,
    client: ClientWithMiddleware,
    cache: NpmCache,
) -> warp::reply::Json {
    println!("handle_request");

    let full_name = if let Some(s) = scope {
        format!("{}/{}", s, name)
    } else {
        name
    };

    let t_str = percent_encoding::percent_decode(t_str_url_encoded.as_bytes())
        .decode_utf8()
        .unwrap();

    if let Ok(t) = DateTime::<Utc>::from_str(&t_str) {
        let matching_versions = lookup_package(t, &full_name, client, cache).await;
        warp::reply::json(&serialize_packument_in_npm_format(
            &full_name,
            matching_versions,
        ))
    } else {
        panic!("BAD DATE: {}", t_str)
    }
}

// Custom rejection handler that maps rejections into responses.
async fn handle_rejection(err: Rejection) -> Result<impl Reply, std::convert::Infallible> {
    eprintln!("unhandled rejection: {:?}", err);
    Ok(reply::with_status(
        "INTERNAL_SERVER_ERROR",
        StatusCode::INTERNAL_SERVER_ERROR,
    ))
}

#[tokio::main]
async fn main() {
    let retry_policy = ExponentialBackoff::builder().build_with_max_retries(6);
    let req_client = ClientBuilder::new(reqwest::Client::new())
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build();
    let req_client2 = req_client.clone();

    let cache = Cache::new(4_194_304);
    let cache2 = cache.clone();

    let non_scoped = warp::path::param::<String>()
        .and(warp::path::param::<String>())
        .and(warp::any().map(move || req_client.clone()))
        .and(warp::any().map(move || cache.clone()))
        .then(
            |t_str_url: String, name, req_client_inner, cache| async move {
                handle_request(t_str_url, None, name, req_client_inner, cache).await
            },
        );

    let scoped = warp::path::param::<String>()
        .and(warp::path::param::<String>())
        .and(warp::path::param::<String>())
        .and(warp::any().map(move || req_client2.clone()))
        .and(warp::any().map(move || cache2.clone()))
        .then(
            |t_str_url: String, scope, name, req_client_inner, cache| async move {
                handle_request(t_str_url, Some(scope), name, req_client_inner, cache).await
            },
        );

    let empty_advisories =
        warp::path!(String / "-" / "npm" / "v1" / "security" / "advisories" / "bulk")
            .map(|_t| warp::reply::json(&Value::Object(Map::default())));

    warp::serve(
        empty_advisories
            .or(non_scoped)
            .or(scoped)
            .recover(handle_rejection),
    )
    .run(([0, 0, 0, 0], 80))
    .await;
}
