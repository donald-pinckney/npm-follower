use std::collections::HashMap;
use std::collections::HashSet;
use std::fs::File;
use std::io::BufWriter;
use std::io::Write;
use std::path::PathBuf;

use crate::MaxConcurrencyClient;

use super::CONFIG;
use async_process::Command;
use async_process::Stdio;
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
use lazy_static::lazy_static;
use postgres_db::custom_types::Semver;
use serde_json::Value;
use tempfile::tempdir;
use tempfile::TempDir;

lazy_static! {
    static ref EPSILON: Duration = Duration::seconds(10);
}

// but yeah, my plan for looking at update flows is:
// 1. Suppose we want to look at how long it takes for lodash 1.0.0 -> 1.0.1 to flow to downstream packages. Let the upload time of 1.0.1 be T_0.
// 2. I get the set \mathcal{P} of all transitive downstream packages of lodash.
// 3. For each package P in \mathcal{P}, I select the most recent version V_0 at time T_0 - \epsilon.
// 4. Then I solve dependencies for V_0, pretending the time is T_0 - \epsilon. If it doesn't include lodash 1.0.0, then I bail out, since V_0 already out of date.
// 5. I then solve V_0 at time T_0. If it contains lodash 1.0.1, and no other versions of lodash, then I categorize the flow as "instant", and bail out.
// 6. Otherwise, I increment T = T_0 + 1 day, select the most recent version of P at time T, say V, and solve V at time T.
//    If it contains lodash 1.0.1 and no other versions, then the flow is categorized as "non-instant" with dT = T - T_0. Loop 6 until done, or give up.

pub(crate) async fn run_solve_job(job: Job, req_client: MaxConcurrencyClient) -> JobResult {
    let update_from_id = job.update_from_id;
    let update_to_id = job.update_to_id;
    let downstream_package_id = job.downstream_package_id;

    let mut solve_history = vec![];

    match run_solve_job_result(job, req_client, &mut solve_history).await {
        Ok(()) => JobResult {
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
            solve_history,
        },
    }
}

async fn run_solve_job_result(
    job: Job,
    req_client: MaxConcurrencyClient,
    history: &mut Vec<SolveResult>,
) -> Result<(), ResultError> {
    // 1. Fetch downstream packument at t=NOW

    let packument_doc = req_client
        .get(format!(
            "http://{}/now/{}",
            CONFIG.registry_host, job.downstream_package_name
        ))
        .await;

    let packument_doc = match packument_doc {
        Value::Object(o) => o,
        _ => panic!("non-object packument"),
    };

    let packument_doc = Some(parse_packument(packument_doc, &job.downstream_package_name))
        .expect("Downstream missing");

    // 2. Allocate temp dir to work in
    let new_tmp_dir = tempdir().unwrap();

    // 3. Solve initial version
    let initial_solve = solve_dependencies(
        &packument_doc,
        job.update_to_time - *EPSILON,
        &new_tmp_dir,
        &job.downstream_package_name,
    )
    .await?;

    // 4. If the current downstream doesn't contain current upstream, then we bail
    if !initial_solve.contains(&job.update_package_name, &job.update_from_version) {
        return Err(ResultError::FromMissing);
    }

    history.push(initial_solve.to_solve_result(&job.update_package_name));

    let mut dt = job.update_to_time + *EPSILON;

    // 5. Solve the immediately after version, and then loop
    loop {
        let this_solve = solve_dependencies(
            &packument_doc,
            dt,
            &new_tmp_dir,
            &job.downstream_package_name,
        )
        .await?;
        history.push(this_solve.to_solve_result(&job.update_package_name));

        if this_solve.are_deps_removed(&job.update_package_name) {
            return Err(ResultError::RemovedDep);
        }

        if this_solve.all_old_gone(&job.update_package_name, &job.update_from_version) {
            return Ok(());
        }

        if let Some(next_dt) = next_time(job.update_to_time, dt) {
            dt = next_dt
        } else {
            return Err(ResultError::GaveUp);
        }
    }
}

fn next_time(initial_time: DateTime<Utc>, current_time: DateTime<Utc>) -> Option<DateTime<Utc>> {
    let next_time = current_time + Duration::days(1);

    if next_time - initial_time > Duration::days(365) {
        None
    } else {
        Some(next_time)
    }
}

fn get_most_recent_leq_time(
    pack: &ParsedPackument<()>,
    dt: DateTime<Utc>,
    package_name: &str,
) -> Option<(Semver, Value)> {
    let mut restricted = restrict_time(pack, Some(dt), package_name)?;
    let latest = restricted.latest_tag;
    let mut v_blob = restricted.versions.remove(&latest).unwrap();
    let v_blob_obj = v_blob.as_object_mut().unwrap();
    v_blob_obj.remove("_id");
    v_blob_obj.remove("_shasum");
    v_blob_obj.remove("_from");
    v_blob_obj.remove("_npmVersion");
    v_blob_obj.remove("_nodeVersion");
    v_blob_obj.remove("_npmUser");
    let latest = semver_spec_serialization::parse_semver(&latest).unwrap();
    Some((latest, v_blob))
}

async fn solve_dependencies(
    packument_doc: &ParsedPackument<()>,
    dt: DateTime<Utc>,
    temp_dir: &TempDir,
    solve_package_name: &str,
) -> Result<SolveSolutionMetrics, ResultError> {
    let (semver_at_time, package_json_at_time) =
        get_most_recent_leq_time(packument_doc, dt, solve_package_name)
            .ok_or(ResultError::DownstreamMissing)?;

    let solve_dir = make_solve_dir(temp_dir);

    let res = solve_dependencies_impl(semver_at_time, package_json_at_time, dt, &solve_dir).await;

    std::fs::remove_dir_all(solve_dir).unwrap();

    res
}

async fn solve_dependencies_impl(
    downstream_v: Semver,
    package_json: Value,
    dt: DateTime<Utc>,
    solve_dir: &PathBuf,
) -> Result<SolveSolutionMetrics, ResultError> {
    // Write the package json out
    let package_json_file = File::create(solve_dir.join("package.json")).unwrap();
    let mut writer = BufWriter::new(package_json_file);
    serde_json::to_writer(&mut writer, &package_json).unwrap();
    writer.flush().unwrap();

    // Solve with npm
    // let mut cmd = Command::new("npm");
    // cmd.arg("install");
    // cmd.current_dir(solve_dir);
    // let output = cmd.output().unwrap();
    // if !output.status.success() {
    //     return Err(ResultError::NpmFailed);
    // }

    let registry_url = format!(
        "http://{}/{}/",
        CONFIG.registry_host,
        urlencoding::encode(&dt.to_string())
    );

    let status = Command::new("npm")
        .arg("install")
        .arg("--ignore-scripts")
        .arg("--no-audit")
        .arg("--no-fund")
        .arg("--registry")
        .arg(registry_url)
        .current_dir(solve_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .unwrap();

    if !status.success() {
        return Err(ResultError::SolveError);
    }

    // Read the package-lock.json
    let package_lock_file = File::open(solve_dir.join("package-lock.json")).unwrap();

    let mut lock_json: Value = serde_json::from_reader(package_lock_file).unwrap();

    let deps = lock_json
        .as_object_mut()
        .unwrap()
        .remove("packages")
        .unwrap();

    let mut solution = SolveSolutionMetrics::new(downstream_v, dt);

    for (dep_name, dep_info) in deps.as_object().unwrap().iter() {
        if dep_name.is_empty() {
            continue;
        }

        let dep_info = dep_info.as_object().unwrap();
        if dep_info.contains_key("link") {
            continue;
        }

        let version = dep_info.get("version").unwrap().as_str().unwrap();
        let version = semver_spec_serialization::parse_semver(version).unwrap();
        solution.push_dep(dep_name.clone(), version);
    }

    Ok(solution)
}

fn make_solve_dir(in_dir: &TempDir) -> PathBuf {
    let solve_dir = in_dir.path().join("solve_root");
    std::fs::create_dir(&solve_dir).unwrap();
    solve_dir
}

struct SolveSolutionMetrics {
    downstream_v: Semver,
    solve_time: DateTime<Utc>,
    deps: HashMap<String, HashSet<Semver>>,
}

impl SolveSolutionMetrics {
    fn new(downstream_v: Semver, solve_time: DateTime<Utc>) -> Self {
        Self {
            downstream_v,
            solve_time,
            deps: HashMap::new(),
        }
    }

    fn push_dep(&mut self, package: String, version: Semver) {
        self.deps.entry(package).or_default().insert(version);
    }

    fn contains(&self, package: &str, version: &Semver) -> bool {
        self.deps
            .get(package)
            .map(|versions| versions.contains(version))
            .unwrap_or(false)
    }

    fn to_solve_result(&self, update_package: &str) -> SolveResult {
        let mut versions: Vec<Semver> = self
            .deps
            .get(update_package)
            .map(|versions| versions.iter().cloned().collect())
            .unwrap_or_default();

        versions.sort();

        SolveResult {
            solve_time: self.solve_time,
            downstream_version: self.downstream_v.clone(),
            update_versions: versions,
        }
    }

    fn all_old_gone(&self, package: &str, old_version: &Semver) -> bool {
        self.deps
            .get(package)
            .unwrap()
            .iter()
            .all(|v| v > old_version)
    }

    fn are_deps_removed(&self, package: &str) -> bool {
        !self.deps.contains_key(package)
    }
}
