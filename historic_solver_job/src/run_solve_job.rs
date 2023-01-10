use super::Configuration;
use super::CONFIG;
use chrono::DateTime;
use chrono::Duration;
use chrono::Utc;
use historic_solver_job_server::packument_requests::parse_packument;
use historic_solver_job_server::packument_requests::restrict_time;
use historic_solver_job_server::packument_requests::ParsedPackument;
use historic_solver_job_server::ResultCategory;
use historic_solver_job_server::ResultError;
use historic_solver_job_server::SolveResult;
use historic_solver_job_server::{Job, JobResult};
use reqwest_middleware::ClientWithMiddleware;
use serde_json::Value;
use tempfile::tempdir;

// but yeah, my plan for looking at update flows is:
// 1. Suppose we want to look at how long it takes for lodash 1.0.0 -> 1.0.1 to flow to downstream packages. Let the upload time of 1.0.1 be T_0.
// 2. I get the set \mathcal{P} of all transitive downstream packages of lodash.
// 3. For each package P in \mathcal{P}, I select the most recent version V_0 at time T_0 - \epsilon.
// 4. Then I solve dependencies for V_0, pretending the time is T_0 - \epsilon. If it doesn't include lodash 1.0.0, then I bail out, since V_0 already out of date.
// 5. I then solve V_0 at time T_0. If it contains lodash 1.0.1, and no other versions of lodash, then I categorize the flow as "instant", and bail out.
// 6. Otherwise, I increment T = T_0 + 1 day, select the most recent version of P at time T, say V, and solve V at time T.
//    If it contains lodash 1.0.1 and no other versions, then the flow is categorized as "non-instant" with dT = T - T_0. Loop 6 until done, or give up.

pub async fn run_solve_job(job: Job, req_client: ClientWithMiddleware) -> JobResult {
    let update_from_id = job.update_from_id;
    let update_to_id = job.update_to_id;
    let downstream_package_id = job.downstream_package_id;

    match run_solve_job_result(job, req_client).await {
        Ok(solve_history) => JobResult {
            update_from_id,
            update_to_id,
            downstream_package_id,
            result_category: ResultCategory::Ok,
            solve_history,
        },
        Err(err) => JobResult {
            update_from_id,
            update_to_id,
            downstream_package_id,
            result_category: ResultCategory::Error(err),
            solve_history: vec![],
        },
    }
}

async fn run_solve_job_result(
    job: Job,
    req_client: ClientWithMiddleware,
) -> Result<Vec<SolveResult>, ResultError> {
    // 1. Fetch downstream packument at t=NOW

    let packument_doc = req_client
        .get(format!(
            "http://{}/now/{}",
            CONFIG.registry_host, job.downstream_package_name
        ))
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

    let packument_doc = Some(parse_packument(packument_doc)).expect("Downstream missing");

    let (before_update_semver, before_update_blob) =
        get_most_recent_lt_time(&packument_doc, job.update_to_time)
            .ok_or(ResultError::DownstreamMissing)?;

    println!("{:?}", before_update_blob);

    // 3. Choose most recent non-beta before update_to_time

    todo!()
}

fn get_most_recent_leq_time(pack: &ParsedPackument, dt: DateTime<Utc>) -> Option<(String, Value)> {
    let mut restricted = restrict_time(pack, Some(dt))?;
    let latest = restricted.latest_tag?;
    let v_blob = restricted.versions.remove(&latest).unwrap();
    Some((latest, v_blob))
}

fn get_most_recent_lt_time(pack: &ParsedPackument, dt: DateTime<Utc>) -> Option<(String, Value)> {
    get_most_recent_leq_time(pack, dt - Duration::seconds(10))
}
