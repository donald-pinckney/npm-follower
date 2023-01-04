use std::mem;
use std::str::FromStr;
use std::sync::Arc;

use chrono::DateTime;
use chrono::Utc;
use moka::future::Cache;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use serde_json::json;
use serde_json::Map;
use serde_json::Value;
use warp::http::StatusCode;
use warp::hyper::Uri;
use warp::reply;
use warp::reply::Json;
use warp::Filter;
use warp::Rejection;
use warp::Reply;

#[derive(Clone)]
struct ParsedPackument {
    latest_tag: Option<String>,
    versions: Map<String, Value>,
    sorted_times: Vec<(String, DateTime<Utc>)>, // sorted by the date
    modified_time: DateTime<Utc>,
    created_time: DateTime<Utc>,
}

type NpmCache = Cache<String, Option<Arc<ParsedPackument>>>;

fn restrict_time(v: &ParsedPackument, filter_time: DateTime<Utc>) -> Option<ParsedPackument> {
    let first_bad_time_idx = v.sorted_times.partition_point(|(_, vt)| *vt <= filter_time);
    if first_bad_time_idx == 0 {
        // Everything must be filtered out, so we bail early with None
        return None;
    } else if first_bad_time_idx == v.sorted_times.len() {
        // Nothing is filtered out
        return Some(v.clone());
    }

    let last_good_time_idx = first_bad_time_idx - 1;
    let (_, last_good_time) = &v.sorted_times[last_good_time_idx];

    let good_times = &v.sorted_times[..first_bad_time_idx];
    let good_versions: Map<String, Value> = good_times
        .iter()
        .map(|(v_name, _)| {
            (
                v_name.clone(),
                v.versions.get(v_name).expect("version must exist").clone(),
            )
        })
        .collect();

    let last_non_beta_good_version = good_times
        .iter()
        .rev()
        .find(|(v_name, _)| !v_name.contains('-') && !v_name.contains('+'))
        .map(|(v_name, _)| v_name.to_owned());

    Some(ParsedPackument {
        latest_tag: last_non_beta_good_version,
        versions: good_versions,
        sorted_times: good_times.to_vec(),
        modified_time: *last_good_time,
        created_time: v.created_time,
    })
}

fn parse_datetime(x: &str) -> DateTime<Utc> {
    let dt = DateTime::parse_from_rfc3339(x)
        .or_else(|_| DateTime::parse_from_rfc3339(&format!("{}Z", x)))
        .unwrap();
    dt.with_timezone(&Utc)
}

fn parse_packument(mut j: Map<String, Value>) -> ParsedPackument {
    let latest_tag = {
        let dist_tags = j.remove("dist-tags").expect("dist-tags must be present");
        dist_tags
            .as_object()
            .expect("dist-tags must be an object")
            .get("latest")
            .expect("latest tag must exist")
            .as_str()
            .expect("latest tag must be a string")
            .to_owned()
    };

    let versions = j.remove("versions").expect("versions must be present");
    let mut time = j.remove("time").expect("time must be present");

    let versions = match versions {
        Value::Object(o) => o,
        _ => panic!("versions must be an object"),
    };

    let time = time.as_object_mut().expect("time must be an object");
    let modified_time = parse_datetime(
        time.remove("modified")
            .expect("modified time must exist")
            .as_str()
            .expect("time must be a string"),
    );

    let created_time = parse_datetime(
        time.remove("created")
            .expect("created time must exist")
            .as_str()
            .expect("time must be a string"),
    );

    let mut sorted_times: Vec<_> = mem::take(time)
        .into_iter()
        .map(|(v, dt_str)| {
            (
                v,
                parse_datetime(dt_str.as_str().expect("dates must be strings")),
            )
        })
        .collect();

    sorted_times.sort_by_key(|(_, dt)| *dt);

    ParsedPackument {
        latest_tag: Some(latest_tag),
        versions,
        sorted_times,
        modified_time,
        created_time,
    }
}

async fn request_package_from_npm(
    full_name: &str,
    client: ClientWithMiddleware,
) -> Option<ParsedPackument> {
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

    Some(parse_packument(packument_doc))
}

async fn lookup_package(
    t: DateTime<Utc>,
    full_name: &str,
    client: ClientWithMiddleware,
    cache: NpmCache,
) -> Option<ParsedPackument> {
    println!("looking up: {}", full_name);
    if let Some(cache_hit) = cache.get(full_name) {
        cache_hit.and_then(|x| restrict_time(&x, t))
    } else {
        let npm_response = request_package_from_npm(full_name, client).await;
        let npm_response = npm_response.map(Arc::new);

        cache
            .insert(full_name.to_owned(), npm_response.clone())
            .await;
        npm_response.and_then(|x| restrict_time(&x, t))
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
        panic!("BAD DATE: {}, full_name = {}", t_str, full_name)
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
    let empty_advisories =
        warp::path!(String / "-" / "npm" / "v1" / "security" / "advisories" / "bulk")
            .and(warp::path::end())
            .map(|_t| warp::reply::json(&Value::Object(Map::default())));

    let log = warp::log("http");

    let tarball_redirect = warp::path!(String / String / "-" / String)
        .and(warp::path::end())
        .map(|_time, name, tarball_name| {
            let uri = Uri::builder()
                .scheme("https")
                .authority("registry.npmjs.org")
                .path_and_query(format!("/{}/-/{}", name, tarball_name))
                .build()
                .unwrap();
            warp::redirect::permanent(uri)
        });

    warp::serve(
        empty_advisories
            .or(root)
            .or(css)
            .or(non_scoped)
            .or(scoped)
            .or(tarball_redirect)
            .recover(handle_rejection)
            .with(log),
    )
    .run(([0, 0, 0, 0], 80))
    .await;
}
