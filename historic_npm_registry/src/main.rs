use std::mem;
use std::str::FromStr;
use std::sync::Arc;

use chrono::DateTime;
use chrono::Utc;
use historic_solver_job_server::packument_requests::parse_packument;
use historic_solver_job_server::packument_requests::restrict_time;
use historic_solver_job_server::packument_requests::NpmCache;
use historic_solver_job_server::packument_requests::ParsedPackument;
use moka::future::Cache;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use serde_json::json;
use serde_json::Map;
use serde_json::Value;
use warp::http::StatusCode;
use warp::hyper::Uri;
use warp::reply;
use warp::Filter;
use warp::Rejection;
use warp::Reply;

async fn request_package_from_npm(
    full_name: &str,
    client: ClientWithMiddleware,
) -> Option<ParsedPackument> {
    // println!("hitting NPM for: {}", full_name);

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

    Some(parse_packument(packument_doc))
}

async fn lookup_package(
    maybe_t: Option<DateTime<Utc>>,
    full_name: &str,
    client: ClientWithMiddleware,
    cache: NpmCache,
) -> Option<ParsedPackument> {
    // println!("looking up: {}", full_name);
    if let Some(cache_hit) = cache.get(full_name) {
        cache_hit.and_then(|x| restrict_time(&x, maybe_t, full_name))
    } else {
        let npm_response = request_package_from_npm(full_name, client).await;
        let npm_response = npm_response.map(Arc::new);

        cache
            .insert(full_name.to_owned(), npm_response.clone())
            .await;
        npm_response.and_then(|x| restrict_time(&x, maybe_t, full_name))
    }
}

fn serialize_datetime(dt: DateTime<Utc>) -> Value {
    Value::String(dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
}

fn serialize_packument_in_npm_format(
    package_name: &str,
    packument: Option<ParsedPackument>,
) -> Value {
    if packument.is_none() {
        let mut m = Map::new();
        m.insert("error".to_owned(), Value::String("Not found".to_owned()));
        return Value::Object(m);
    }

    let packument = packument.unwrap();

    let time_dict: Map<String, Value> = std::iter::once((
        "modified".to_owned(),
        serialize_datetime(packument.modified_time),
    ))
    .chain(
        std::iter::once((
            "created".to_owned(),
            serialize_datetime(packument.created_time),
        ))
        .chain(
            packument
                .sorted_times
                .into_iter()
                .map(|(v, dt)| (v, serialize_datetime(dt))),
        ),
    )
    .collect();

    if let Some(latest_tag) = packument.latest_tag {
        json!({
            "_id": package_name,
            "name": package_name,
            "dist-tags": {
                "latest": latest_tag
            },
            "versions": packument.versions,
            "time": time_dict
        })
    } else {
        json!({
            "_id": package_name,
            "name": package_name,
            "dist-tags": {
            },
            "versions": packument.versions,
            "time": time_dict
        })
    }
}

async fn handle_request(
    t_str_url_encoded: String,
    scope: Option<String>,
    name: String,
    client: ClientWithMiddleware,
    cache: NpmCache,
) -> warp::reply::Json {
    // println!("handle_request");

    let name = percent_encoding::percent_decode(name.as_bytes())
        .decode_utf8()
        .unwrap();

    let full_name = if let Some(s) = scope {
        // println!("got scope as /: {}", s);
        format!("{}/{}", s, name)
    } else {
        let comps: Vec<_> = name.split('/').collect();
        if comps.len() == 2 {
            // println!("got scope as %2f: {}", comps[0]);
            format!("{}/{}", comps[0], comps[1])
        } else if comps.len() == 1 {
            comps[0].to_owned()
        } else {
            panic!("Invalid request. Got name: {}", name);
        }
    };

    let t_str = percent_encoding::percent_decode(t_str_url_encoded.as_bytes())
        .decode_utf8()
        .unwrap();

    let parsed_t = if t_str == "now" {
        None
    } else if let Ok(t) = DateTime::<Utc>::from_str(&t_str) {
        Some(t)
    } else {
        panic!("BAD DATE: {}, full_name = {}", t_str, full_name)
    };

    let matching_versions = lookup_package(parsed_t, &full_name, client, cache).await;
    warp::reply::json(&serialize_packument_in_npm_format(
        &full_name,
        matching_versions,
    ))
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
    pretty_env_logger::init();

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
        .and(warp::path::end())
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
        .and(warp::path::end())
        .then(
            |t_str_url: String, scope, name, req_client_inner, cache| async move {
                handle_request(t_str_url, Some(scope), name, req_client_inner, cache).await
            },
        );

    let root = warp::path::end().map(|| StatusCode::NOT_FOUND);

    let css = warp::path!("static" / "main.css")
        .and(warp::path::end())
        .map(|| StatusCode::NOT_FOUND);
    let empty_advisories = warp::path!(String / "-" / "npm" / "v1" / "security")
        .map(|_t| warp::reply::json(&Value::Object(Map::default())));

    let log = warp::log("http");

    let tarball_redirect = warp::path!(String / String / "-" / String)
        .and(warp::path::end())
        .map(|_time, _name, _tarball_name| ())
        .untuple_one()
        .and(warp::fs::file("empty-package.tar"));

    // .untuple_one()
    // .untuple_one();
    // .map(|_time, name, tarball_name| {

    //     let bytes = include_bytes!("empty-package.tar");
    //     warp::reply::

    //     let uri = Uri::builder()
    //         .scheme("https")
    //         .authority("registry.npmjs.org")
    //         .path_and_query(format!("/{}/-/{}", name, tarball_name))
    //         .build()
    //         .unwrap();
    //     warp::redirect::permanent(uri)
    // });

    let tarball_scoped_redirect = warp::path!(String / String / String / "-" / String)
        .and(warp::path::end())
        .map(|_time, _scope, _name, _tarball_name| ())
        .untuple_one()
        .and(warp::fs::file("empty-package.tar"));

    // .map(|_time, scope, name, tarball_name| {
    //     let uri = Uri::builder()
    //         .scheme("https")
    //         .authority("registry.npmjs.org")
    //         .path_and_query(format!("/{}/{}/-/{}", scope, name, tarball_name))
    //         .build()
    //         .unwrap();
    //     warp::redirect::permanent(uri)
    // });

    warp::serve(
        empty_advisories
            .or(root)
            .or(css)
            .or(non_scoped)
            .or(scoped)
            .or(tarball_redirect)
            .or(tarball_scoped_redirect)
            .recover(handle_rejection)
            .with(log),
    )
    .run(([0, 0, 0, 0], 80))
    .await;
}
