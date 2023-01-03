use std::str::FromStr;

use chrono::DateTime;
use chrono::Utc;
use serde_json::Value;
use warp::http::StatusCode;
use warp::reply;
use warp::Filter;
use warp::Rejection;
use warp::Reply;

async fn lookup_package(t: DateTime<Utc>, name: String, client: reqwest::Client) -> Value {
    println!("{}, {}", t, name);
    Value::Null
    // todo!()
}

async fn handle_request(
    t_str_url_encoded: String,
    scope: Option<String>,
    name: String,
    client: reqwest::Client,
) -> warp::reply::Json {
    let full_name = if let Some(s) = scope {
        format!("{}/{}", s, name)
    } else {
        name
    };

    let t_str = percent_encoding::percent_decode(t_str_url_encoded.as_bytes())
        .decode_utf8()
        .unwrap();

    if let Ok(t) = DateTime::<Utc>::from_str(&t_str) {
        warp::reply::json(&lookup_package(t, full_name, client).await)
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
    let req_client = reqwest::Client::new();
    let req_client2 = req_client.clone();

    let non_scoped = warp::path::param::<String>()
        .and(warp::path::param::<String>())
        .and(warp::any().map(move || req_client.clone()))
        .then(|t_str_url: String, name, req_client_inner| async move {
            handle_request(t_str_url, None, name, req_client_inner).await
        });

    let scoped = warp::path::param::<String>()
        .and(warp::path::param::<String>())
        .and(warp::path::param::<String>())
        .and(warp::any().map(move || req_client2.clone()))
        .then(
            |t_str_url: String, scope, name, req_client_inner| async move {
                handle_request(t_str_url, Some(scope), name, req_client_inner).await
            },
        );

    warp::serve(non_scoped.or(scoped).recover(handle_rejection))
        .run(([0, 0, 0, 0], 3030))
        .await;
}
