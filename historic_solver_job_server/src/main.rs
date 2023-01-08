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
use historic_solver_job_server::{job_results, Job, JobResult};
use postgres_db::{
    connection::async_pool::{DbConnection, QueryRunner},
    custom_types::Semver,
};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use warp::{http, reply, Filter, Rejection, Reply};

async fn handle_get_jobs(num_jobs: i64, node_name: String, db: DbConnection) -> Vec<Job> {
    if node_name == "TEST" {
        // TEST QUERY
        let query = sql_query(
            r#"
            WITH cte AS MATERIALIZED (
                SELECT update_from_id, update_to_id, downstream_package_id
                FROM   historic_solver.job_inputs
                WHERE  job_state = 'none'
                ORDER BY update_from_id, downstream_package_id
                LIMIT  ?
                FOR    UPDATE SKIP LOCKED
                )
             SELECT
             job.update_to_id, 
             job.downstream_package_id,
             job.update_package_name,
             job.update_from_version,
             job.update_to_version,
             job.update_to_time,
             job.downstream_package_name
             FROM job_inputs job, cte
             WHERE  job.update_from_id = cte.update_from_id AND job.update_to_id = cte.update_to_id AND job.downstream_package_id = cte.downstream_package_id;
        "#
        )
        .bind::<Int8, _>(num_jobs);

        db.get_results(query).await.unwrap()
    } else {
        // REAL QUERY
        let query = sql_query(
        r#"
        WITH cte AS MATERIALIZED (
            SELECT update_from_id, update_to_id, downstream_package_id
            FROM   historic_solver.job_inputs
            WHERE  job_state = 'none'
            ORDER BY update_from_id, downstream_package_id
            LIMIT  ?
            FOR    UPDATE SKIP LOCKED
            )
         UPDATE job_inputs job
         SET    job_state = 'started', start_time = now(), work_node = ?
         FROM   cte
         WHERE  job.update_from_id = cte.update_from_id AND job.update_to_id = cte.update_to_id AND job.downstream_package_id = cte.downstream_package_id
         RETURNING job.update_from_id, 
         job.update_to_id, 
         job.downstream_package_id,
         job.update_package_name,
         job.update_from_version,
         job.update_to_version,
         job.update_to_time,
         job.downstream_package_name;
        "#
        )
        .bind::<Int8, _>(num_jobs)
        .bind::<Text, _>(node_name);

        db.get_results(query).await.unwrap()
    }
}

async fn handle_submit_result(
    job_result: JobResult,
    db: DbConnection,
) -> Result<impl warp::Reply, warp::Rejection> {
    let query = diesel::insert_into(job_results::table).values(&job_result);

    db.execute(query).await.unwrap();

    Ok(warp::reply::with_status(
        "Result submitted",
        http::StatusCode::CREATED,
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

    let connection_pool = DbConnection::connect().await;
    let connection_pool_filter = warp::any().map(move || connection_pool.clone());

    let get_jobs = warp::path!("get_jobs" / i64 / String)
        .and(connection_pool_filter.clone())
        .then(|num_jobs, node_name, connection_pool| async move {
            warp::reply::json(&handle_get_jobs(num_jobs, node_name, connection_pool).await)
        });

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
        .and_then(handle_submit_result);

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
