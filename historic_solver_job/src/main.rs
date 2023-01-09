// but yeah, my plan for looking at update flows is:
// 1. Suppose we want to look at how long it takes for lodash 1.0.0 -> 1.0.1 to flow to downstream packages. Let the upload time of 1.0.1 be T_0.
// 2. I get the set \mathcal{P} of all transitive downstream packages of lodash.
// 3. For each package P in \mathcal{P}, I select the most recent version V_0 at time T_0 - \epsilon.
// 4. Then I solve dependencies for V_0, pretending the time is T_0 - \epsilon. If it doesn't include lodash 1.0.0, then I bail out, since V_0 already out of date.
// 5. I then solve V_0 at time T_0. If it contains lodash 1.0.1, and no other versions of lodash, then I categorize the flow as "instant", and bail out.
// 6. Otherwise, I increment T = T_0 + 1 day, select the most recent version of P at time T, say V, and solve V at time T.
//    If it contains lodash 1.0.1 and no other versions, then the flow is categorized as "non-instant" with dT = T - T_0. Loop 6 until done, or give up.

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use historic_solver_job_server::{
    async_pool::{handle_get_jobs, handle_submit_result},
    Job, JobResult,
};
use lazy_static::lazy_static;
use postgres_db::connection::async_pool::DbConnection;
use tokio::sync::mpsc::{self, UnboundedSender};

const JOBS_PER_THREAD: i64 = 1000;

#[derive(Debug)]
struct Configuration {
    npm_cache_dir: String,
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
    async fn run(self) -> JobResult;
}

#[async_trait]
impl RunnableJob for Job {
    async fn run(self) -> JobResult {
        todo!()
    }
}

#[tokio::main]
async fn main() {
    println!("config: {:#?}", *CONFIG);
    return;

    let (result_tx, mut result_rx) = mpsc::unbounded_channel();

    let schedule_more_jobs_if_fewer_than = JOBS_PER_THREAD * CONFIG.num_threads / 10;
    let start_time = Utc::now();

    tokio::spawn(async move {
        let mut active_jobs: i64 = 0;

        let db = DbConnection::connect().await;

        grab_and_run_job_batch(&mut active_jobs, &result_tx, &db).await;

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
                grab_and_run_job_batch(&mut active_jobs, &result_tx, &db).await;
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
) {
    let jobs = grab_job_batch(db).await;

    *active_jobs += jobs.len() as i64;

    for job in jobs {
        let result_tx = result_tx.clone();
        tokio::task::spawn(async move {
            let job_result = job.run().await;
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

    // TODO: load from env vars
    Configuration {
        npm_cache_dir: env::var("NPM_CACHE_DIR").expect("NPM_CACHE_DIR"),
        num_threads: env::var("NUM_THREADS")
            .expect("NUM_THREADS")
            .parse()
            .expect("failed to parse NUM_THREADS"),
        registry_host: env::var("REGISTRY_HOST").expect("REGISTRY_HOST"),
        node_name: env::var("NODE_NAME").expect("NODE_NAME"),
        max_job_time: Duration::seconds(desired_secs),
    }
}
