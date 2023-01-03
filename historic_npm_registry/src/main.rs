use chrono::DateTime;
use chrono::Utc;
use serde_json::Value;
use warp::Filter;

async fn lookup_package(client: reqwest::Client, t: DateTime<Utc>, name: String) -> Value {
    println!("{}, {}", t, name);
    Value::Null
    // todo!()
}

#[tokio::main]
async fn main() {
    let req_client = reqwest::Client::new();
    let req_client2 = req_client.clone();

    let non_scoped = warp::path::param::<DateTime<Utc>>()
        .and(warp::path::param::<String>())
        .and(warp::any().map(move || req_client.clone()))
        .then(|t, name, req_client_inner| async move {
            warp::reply::json(&lookup_package(req_client_inner, t, name).await)
        });

    let scoped = warp::path::param::<DateTime<Utc>>()
        .and(warp::path::param::<String>())
        .and(warp::path::param::<String>())
        .and(warp::any().map(move || req_client2.clone()))
        .then(|t, scope, name, req_client_inner| async move {
            warp::reply::json(
                &lookup_package(req_client_inner, t, format!("{}/{}", scope, name)).await,
            )
        });

    warp::serve(non_scoped.or(scoped))
        .run(([127, 0, 0, 1], 3030))
        .await;
}
