use super::Configuration;
use super::CONFIG;
use historic_solver_job_server::{Job, JobResult};

pub async fn run_solve_job(job: Job) -> JobResult {
    // 1. Allocate a temp dir to use for all solves

    // 2. Fetch downstream packument at t=NOW

    // 3. Choose most recent non-beta before update_to_time

    todo!()
}
