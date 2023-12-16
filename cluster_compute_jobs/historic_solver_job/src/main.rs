use std::sync::Arc;

use async_trait::async_trait;
use chrono::{Duration, Utc};
use historic_solver_job::{
    async_pool::{handle_get_jobs, handle_submit_result},
    Job, JobResult, MaxConcurrencyClient,
};
use lazy_static::lazy_static;
use postgres_db::connection::async_pool::DbConnection;
use reqwest_middleware::ClientBuilder;
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use tokio::sync::mpsc;
use tokio::sync::RwLock;

mod run_solve_job;

const JOBS_PER_THREAD: i64 = 8;

#[derive(Debug)]
pub struct Configuration {
    num_threads: i64,
    registry_host: String,
    node_name: String,
    npm_config_cache: String,
    max_job_time: Duration,
}

lazy_static! {
    static ref CONFIG: Configuration = load_config();
}

#[async_trait]
trait RunnableJob {
    async fn run(
        self,
        req_client: MaxConcurrencyClient,
        subprocess_semaphore: Arc<tokio::sync::Semaphore>,
        nuke_process_semaphore: Arc<RwLock<()>>,
    ) -> JobResult;
}

#[async_trait]
impl RunnableJob for Job {
    async fn run(
        self,
        req_client: MaxConcurrencyClient,
        subprocess_semaphore: Arc<tokio::sync::Semaphore>,
        nuke_process_semaphore: Arc<RwLock<()>>,
    ) -> JobResult {
        run_solve_job::run_solve_job(
            self,
            req_client,
            subprocess_semaphore,
            nuke_process_semaphore,
        )
        .await
    }
}

#[tokio::main]
async fn main() {
    let (result_tx, mut result_rx) = mpsc::unbounded_channel();

    let schedule_more_jobs_if_fewer_than = JOBS_PER_THREAD * CONFIG.num_threads / 10;
    let start_time = Utc::now();

    let active_jobs: Arc<RwLock<i64>> = Arc::new(tokio::sync::RwLock::new(0));
    let active_jobs2 = active_jobs.clone();

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));

        loop {
            interval.tick().await;
            println!("queue size: {}", active_jobs2.read().await);
        }
    });

    let nuke_cache_lock = Arc::new(RwLock::new(()));
    let nuke_cache_lock2 = nuke_cache_lock.clone();

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5 * 60 * 60));

        loop {
            interval.tick().await;
            let permit = nuke_cache_lock2.write().await;
            println!("Clearing npm cache!");

            // TODO: nuke cache
            std::fs::remove_dir_all(&CONFIG.npm_config_cache).unwrap();
            std::fs::create_dir(&CONFIG.npm_config_cache).unwrap();

            drop(permit)
        }
    });

    tokio::spawn(async move {
        let db = DbConnection::connect().await;

        let subprocess_semaphore =
            Arc::new(tokio::sync::Semaphore::new(CONFIG.num_threads as usize));

        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(6);
        let req_client = ClientBuilder::new(reqwest::Client::new())
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();
        let req_client = MaxConcurrencyClient::new(req_client, CONFIG.num_threads as usize);

        grab_and_run_job_batch(
            active_jobs.as_ref(),
            &result_tx,
            &db,
            &req_client,
            &subprocess_semaphore,
            &nuke_cache_lock,
        )
        .await;

        if *active_jobs.read().await == 0 {
            println!("We got no initial jobs to run, exiting.");
            return;
        }

        while let Some(result) = result_rx.recv().await {
            write_result_to_postgres(result, &db).await;

            let new_active_jobs = {
                let mut active_jobs_lock = active_jobs.write().await;
                *active_jobs_lock -= 1;
                *active_jobs_lock
            };

            let now = Utc::now();
            let dt = now - start_time;

            if new_active_jobs < schedule_more_jobs_if_fewer_than && dt < CONFIG.max_job_time {
                grab_and_run_job_batch(
                    active_jobs.as_ref(),
                    &result_tx,
                    &db,
                    &req_client,
                    &subprocess_semaphore,
                    &nuke_cache_lock,
                )
                .await;
            }

            if *active_jobs.read().await == 0 {
                println!("No jobs left to run, exiting");
                return;
            }
        }
    })
    .await
    .unwrap();
}

async fn grab_and_run_job_batch(
    active_jobs: &RwLock<i64>,
    result_tx: &mpsc::UnboundedSender<JobResult>,
    db: &DbConnection,
    req_client: &MaxConcurrencyClient,
    subprocess_semaphore: &Arc<tokio::sync::Semaphore>,
    nuke_cache_lock: &Arc<RwLock<()>>,
) {
    let jobs = grab_job_batch(db).await;

    {
        let mut active_jobs = active_jobs.write().await;
        println!(
            "Fetched new jobs. Queue size {} -> {}",
            *active_jobs,
            *active_jobs + jobs.len() as i64
        );

        *active_jobs += jobs.len() as i64;
    }

    for job in jobs {
        let result_tx = result_tx.clone();
        let req_client = req_client.clone();
        let subprocess_semaphore = subprocess_semaphore.clone();
        let nuke_cache_lock = nuke_cache_lock.clone();
        tokio::task::spawn(async move {
            let job_result = job
                .run(req_client, subprocess_semaphore, nuke_cache_lock)
                .await;
            result_tx.send(job_result).unwrap();
        });
    }
}

async fn grab_job_batch(db: &DbConnection) -> Vec<Job> {
    handle_get_jobs(CONFIG.num_threads * JOBS_PER_THREAD, &CONFIG.node_name, db).await
}

async fn write_result_to_postgres(res: JobResult, db: &DbConnection) {
    // println!("{:#?}", res);
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

    let desired_secs = secs - 60 * 20;

    Configuration {
        num_threads: env::var("TOKIO_WORKER_THREADS")
            .expect("TOKIO_WORKER_THREADS")
            .parse()
            .expect("failed to parse TOKIO_WORKER_THREADS"),
        registry_host: env::var("REGISTRY_HOST").expect("REGISTRY_HOST"),
        node_name: env::var("NODE_NAME").expect("NODE_NAME"),
        npm_config_cache: env::var("npm_config_cache").expect("npm_config_cache"),
        max_job_time: Duration::seconds(desired_secs),
    }
}
