// but yeah, my plan for looking at update flows is:
// 1. Suppose we want to look at how long it takes for lodash 1.0.0 -> 1.0.1 to flow to downstream packages. Let the upload time of 1.0.1 be T_0.
// 2. I get the set \mathcal{P} of all transitive downstream packages of lodash.
// 3. For each package P in \mathcal{P}, I select the most recent version V_0 at time T_0 - \epsilon.
// 4. Then I solve dependencies for V_0, pretending the time is T_0 - \epsilon. If it doesn't include lodash 1.0.0, then I bail out, since V_0 already out of date.
// 5. I then solve V_0 at time T_0. If it contains lodash 1.0.1, and no other versions of lodash, then I categorize the flow as "instant", and bail out.
// 6. Otherwise, I increment T = T_0 + 1 day, select the most recent version of P at time T, say V, and solve V at time T.
//    If it contains lodash 1.0.1 and no other versions, then the flow is categorized as "non-instant" with dT = T - T_0. Loop 6 until done, or give up.

// job input table:
// update_from_id   |   update_to_id   |   downstream_package_id   |  state ("none", "started", "done")  |  start_time  |  work_node  |  update_package_name    |   update_from_version    |    update_to_version    |   update_to_time    |    downstream_package_name
// PK(update_from_id, update_to_id, downstream_package_id)

// job result table:
// update_from_id   |   update_to_id   |   downstream_package_id   |   result_category ("from_missing_dep", "gave_up", "removed_dep", "error", "ok")   |   [(solve_time, [v])]
// PK(update_from_id, update_to_id, downstream_package_id)

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use historic_solver_job_server::{Job, JobResult};
use lazy_static::lazy_static;
use tokio::sync::mpsc::{self, UnboundedSender};

const SCHEDULE_MORE_JOBS_IF_FEWER_THAN: usize = 100;

struct Configuration {
    npm_cache_dir: String,
    num_threads: i32,
    postgres_connection_str: String,
    registry_host: String,
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
    let (result_tx, mut result_rx) = mpsc::unbounded_channel();

    tokio::spawn(async move {
        let mut active_jobs = 0;

        grab_and_run_job_batch(&mut active_jobs, result_tx.clone()).await;

        while let Some(result) = result_rx.recv().await {
            write_result_to_postgres(result).await;

            active_jobs -= 1;

            if active_jobs < SCHEDULE_MORE_JOBS_IF_FEWER_THAN {
                grab_and_run_job_batch(&mut active_jobs, result_tx.clone()).await;
            }
        }
    });
}

async fn grab_and_run_job_batch(active_jobs: &mut usize, result_tx: UnboundedSender<JobResult>) {
    let jobs = grab_job_batch().await;
    if jobs.is_empty() {
        return;
    }

    *active_jobs += jobs.len();

    for job in jobs {
        let result_tx = result_tx.clone();
        tokio::task::spawn(async move {
            let job_result = job.run().await;
            result_tx.send(job_result).unwrap();
        });
    }
}

fn load_config() -> Configuration {
    // TODO: load from env vars
    Configuration {
        npm_cache_dir: "TODO".to_owned(),
        num_threads: 16,
        postgres_connection_str: todo!(),
        registry_host: todo!(),
    }
}

async fn grab_job_batch() -> Vec<Job> {
    // TODO: lookup jobs to run in postgres

    todo!()
}

async fn write_result_to_postgres(res: JobResult) {
    // TODO: send result to postgres
    todo!()
}
