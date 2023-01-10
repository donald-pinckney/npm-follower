use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use historic_solver_job_server::{
    async_pool::{handle_get_jobs, handle_submit_result},
    Job, JobResult,
};
use lazy_static::lazy_static;
use postgres_db::connection::async_pool::DbConnection;
use reqwest::IntoUrl;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware, RequestBuilder};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use serde_json::Value;
use tokio::sync::mpsc::{self, UnboundedSender};

mod run_solve_job;

const JOBS_PER_THREAD: i64 = 1000;

#[derive(Debug)]
pub struct Configuration {
    num_threads: i64,
    registry_host: String,
    node_name: String,
    max_job_time: Duration,
}

lazy_static! {
    static ref CONFIG: Configuration = load_config();
}

#[async_trait]
trait RunnableJob {
    async fn run(self, req_client: MaxConcurrencyClient) -> JobResult;
}

#[async_trait]
impl RunnableJob for Job {
    async fn run(self, req_client: MaxConcurrencyClient) -> JobResult {
        run_solve_job::run_solve_job(self, req_client).await
    }
}

#[derive(Clone)]
struct MaxConcurrencyClient {
    client: ClientWithMiddleware,
    semaphore: Arc<tokio::sync::Semaphore>,
}

impl MaxConcurrencyClient {
    fn new(client: ClientWithMiddleware, max_concurrency: usize) -> Self {
        MaxConcurrencyClient {
            client,
            semaphore: Arc::new(tokio::sync::Semaphore::new(max_concurrency)),
        }
    }

    async fn get<U: IntoUrl>(&self, url: U) -> Value {
        let permit = self.semaphore.acquire().await.unwrap();
        let res = self
            .client
            .get(url)
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap();
        drop(permit);
        res
    }
}

#[tokio::main]
async fn main() {
    let (result_tx, mut result_rx) = mpsc::unbounded_channel();

    let schedule_more_jobs_if_fewer_than = JOBS_PER_THREAD * CONFIG.num_threads / 10;
    let start_time = Utc::now();

    tokio::spawn(async move {
        let mut active_jobs: i64 = 0;

        let db = DbConnection::connect().await;

        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(6);
        let req_client = ClientBuilder::new(reqwest::Client::new())
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();
        let req_client = MaxConcurrencyClient::new(req_client, CONFIG.num_threads as usize);

        grab_and_run_job_batch(&mut active_jobs, &result_tx, &db, &req_client).await;

        if active_jobs == 0 {
            println!("We got no initial jobs to run, exiting.");
            return;
        }

        while let Some(result) = result_rx.recv().await {
            write_result_to_postgres(result, &db).await;

            active_jobs -= 1;

            let now = Utc::now();
            let dt = now - start_time;

            if active_jobs < schedule_more_jobs_if_fewer_than && dt < CONFIG.max_job_time {
                grab_and_run_job_batch(&mut active_jobs, &result_tx, &db, &req_client).await;
            }

            if active_jobs == 0 {
                println!("No jobs left to run, exiting");
                return;
            }
        }
    })
    .await
    .unwrap();
}

async fn grab_and_run_job_batch(
    active_jobs: &mut i64,
    result_tx: &UnboundedSender<JobResult>,
    db: &DbConnection,
    req_client: &MaxConcurrencyClient,
) {
    let jobs = grab_job_batch(db).await;

    println!(
        "Fetched new jobs. Queue size {} -> {}",
        *active_jobs,
        *active_jobs + jobs.len() as i64
    );

    *active_jobs += jobs.len() as i64;

    for job in jobs {
        let result_tx = result_tx.clone();
        let req_client = req_client.clone();
        tokio::task::spawn(async move {
            let job_result = job.run(req_client).await;
            result_tx.send(job_result).unwrap();
        });
    }
}

async fn grab_job_batch(db: &DbConnection) -> Vec<Job> {
    handle_get_jobs(CONFIG.num_threads * JOBS_PER_THREAD, &CONFIG.node_name, db).await
}

async fn write_result_to_postgres(res: JobResult, db: &DbConnection) {
    handle_submit_result(res, db).await.unwrap();
}

fn load_config() -> Configuration {
    use dotenv::dotenv;
    use std::env;

    dotenv().expect("failed to load .env");

    let job_time_str = env::var("MAX_JOB_TIME").expect("MAX_JOB_TIME");
    let dash_comps: Vec<_> = job_time_str.split('-').collect();
    let colon_comps: Vec<_> = dash_comps.last().unwrap().split(':').collect();

    let non_day_secs: i64 = match colon_comps.len() {
        3 => {
            60 * 60 * colon_comps[0].parse::<i64>().unwrap()
                + 60 * colon_comps[1].parse::<i64>().unwrap()
                + colon_comps[2].parse::<i64>().unwrap()
        }
        2 => 60 * colon_comps[0].parse::<i64>().unwrap() + colon_comps[1].parse::<i64>().unwrap(),
        1 => colon_comps[0].parse().unwrap(),
        _ => panic!("invalid time string: {}", job_time_str),
    };

    let secs = if dash_comps.len() == 1 {
        non_day_secs
    } else {
        assert_eq!(dash_comps.len(), 2);
        24 * 60 * 60 * dash_comps[0].parse::<i64>().unwrap() + non_day_secs
    };

    let desired_secs = secs - 60 * 5;

    Configuration {
        num_threads: env::var("TOKIO_WORKER_THREADS")
            .expect("TOKIO_WORKER_THREADS")
            .parse()
            .expect("failed to parse TOKIO_WORKER_THREADS"),
        registry_host: env::var("REGISTRY_HOST").expect("REGISTRY_HOST"),
        node_name: env::var("NODE_NAME").expect("NODE_NAME"),
        max_job_time: Duration::seconds(desired_secs),
    }
}
