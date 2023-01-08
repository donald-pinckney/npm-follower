// job input table:
// update_from_id   |   update_to_id   |   downstream_package_id   |  state ("none", "started", "done")  |  start_time  |  work_node  |  update_package_name    |   update_from_version    |    update_to_version    |   update_to_time    |    downstream_package_name
// PK(update_from_id, update_to_id, downstream_package_id)

// job result table:
// update_from_id   |   update_to_id   |   downstream_package_id   |   result_category ("missing_dep", "gave_up", "error", "ok")   |   [(solve_time, [v])]
// PK(update_from_id, update_to_id, downstream_package_id)

use chrono::{DateTime, Utc};
use diesel::{
    prelude::*,
    sql_query,
    sql_types::{Int8, Text},
};
use historic_solver_job_server::{
    async_pool::{handle_get_jobs, handle_submit_result},
    job_results, Job, JobResult,
};
use postgres_db::{
    connection::async_pool::{DbConnection, QueryRunner},
    custom_types::Semver,
};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use warp::{http, reply, Filter, Rejection, Reply};

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

    let connection_pool = DbConnection::connect().await;
    let connection_pool_filter = warp::any().map(move || connection_pool.clone());

    let get_jobs = warp::path!("get_jobs" / i64 / String)
        .and(connection_pool_filter.clone())
        .then(
            |num_jobs, node_name: String, connection_pool: DbConnection| async move {
                warp::reply::json(&handle_get_jobs(num_jobs, &node_name, &connection_pool).await)
            },
        );

    // let submit_result = warp::path!("get_jobs" / i32 / String)
    //     .and(connection_pool_filter)
    //     .then(|num_jobs, node_name, connection_pool| async move {
    //         handle_get_jobs(num_jobs, node_name, connection_pool).await
    //     });

    let submit_result = warp::post()
        .and(warp::path("submit_result"))
        .and(warp::path::end())
        .and(warp::body::json())
        .and(connection_pool_filter)
        .then(|job_result, connection_pool| async move {
            handle_submit_result(job_result, &connection_pool)
                .await
                .unwrap();
            warp::reply::with_status("Result submitted", http::StatusCode::CREATED)
        });

    // let scoped = warp::path::param::<String>()
    //     .and(warp::path::param::<String>())
    //     .and(warp::path::param::<String>())
    //     .and(warp::any().map(move || req_client2.clone()))
    //     .and(warp::any().map(move || cache2.clone()))
    //     .and(warp::path::end())
    //     .then(
    //         |t_str_url: String, scope, name, req_client_inner, cache| async move {
    //             handle_request(t_str_url, Some(scope), name, req_client_inner, cache).await
    //         },
    //     );

    let root = warp::path::end().map(|| StatusCode::NOT_FOUND);

    let css = warp::path!("static" / "main.css")
        .and(warp::path::end())
        .map(|| StatusCode::NOT_FOUND);

    let log = warp::log("http");

    warp::serve(
        root.or(css)
            .or(get_jobs)
            .or(submit_result)
            .recover(handle_rejection)
            .with(log),
    )
    .run(([0, 0, 0, 0], 80))
    .await;
}
