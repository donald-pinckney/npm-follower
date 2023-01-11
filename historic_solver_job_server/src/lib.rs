use std::io::Write;

use chrono::{DateTime, Utc};
use diesel::{
    pg::Pg,
    prelude::*,
    serialize::{self, Output, ToSql, WriteTuple},
    sql_types::{Array, Timestamptz},
    AsExpression,
};
use moka::future::Cache;
use postgres_db::{custom_types::Semver, schema::sql_types::SemverStruct};
use reqwest::{IntoUrl, Url};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

// job input table:
// update_from_id   |   update_to_id   |   downstream_package_id   |  state ("none", "started", "done")  |  start_time  |  work_node  |  update_package_name    |   update_from_version    |    update_to_version    |   update_to_time    |    downstream_package_name
// PK(update_from_id, update_to_id, downstream_package_id)

// job result table:
// update_from_id   |   update_to_id   |   downstream_package_id   |   result_category ("missing_dep", "gave_up", "error", "ok")   |   [(solve_time, [v])]
// PK(update_from_id, update_to_id, downstream_package_id)

diesel::table! {
    use diesel::sql_types::*;
    use postgres_db::schema::sql_types::SemverStruct;

    historic_solver_job_inputs (update_from_id, update_to_id, downstream_package_id) {
        update_from_id -> Int8,
        update_to_id -> Int8,
        downstream_package_id -> Int8,
        job_state -> Text, // ("none", "started", "done")
        start_time -> Nullable<Timestamptz>,
        work_node -> Nullable<Text>,
        update_package_name -> Text,
        update_from_version -> SemverStruct,
        update_to_version -> SemverStruct,
        update_to_time -> Timestamptz,
        downstream_package_name -> Text,
    }
}

#[derive(diesel::sql_types::SqlType)]
#[diesel(postgres_type(name = "historic_solver_solve_result_struct"))]
pub struct SolveResultSql;

// #[derive(diesel::sql_types::SqlType)]
// #[diesel(postgres_type(name = "Text"))]
// pub struct ResultCategorySql;

diesel::table! {
    use diesel::sql_types::*;
    use postgres_db::schema::sql_types::SemverStruct;
    use super::{SolveResultSql};

    historic_solver_job_results (update_from_id, update_to_id, downstream_package_id) {
        update_from_id -> Int8,
        update_to_id -> Int8,
        downstream_package_id -> Int8,
        result_category -> Text, // see below
        solve_history -> Array<SolveResultSql>, // [(solve_time, [v])]
        stdout -> Text,
        stderr -> Text,
    }
}

#[derive(Debug, Serialize, Deserialize, Queryable, QueryableByName)]
#[diesel(table_name = historic_solver_job_inputs)]
pub struct Job {
    pub update_from_id: i64,
    pub update_to_id: i64,
    pub downstream_package_id: i64,
    pub update_package_name: String,
    pub update_from_version: Semver,
    pub update_to_version: Semver,
    pub update_to_time: DateTime<Utc>,
    pub downstream_package_name: String,
}

#[derive(Debug, Serialize, Deserialize, Insertable)]
#[diesel(table_name = historic_solver_job_results)]
pub struct JobResult {
    pub update_from_id: i64,
    pub update_to_id: i64,
    pub downstream_package_id: i64,
    pub result_category: ResultCategory, // ("FromMissing", "GaveUp", "RemovedDep", "SolveError", "Ok", "MiscError")
    pub solve_history: Vec<SolveResult>,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Serialize, Deserialize, AsExpression)]
#[diesel(sql_type = diesel::sql_types::Text)]
pub enum ResultCategory {
    Ok,
    Error(ResultError),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ResultError {
    DownstreamMissingPackage,
    DownstreamMissingVersion,
    FromMissing,
    GaveUp,
    RemovedDep,
    SolveError,
    MiscError,
}

impl ToSql<diesel::sql_types::Text, Pg> for ResultCategory {
    fn to_sql(&self, out: &mut Output<Pg>) -> serialize::Result {
        let as_str: &'static [u8] = match self {
            ResultCategory::Ok => b"Ok",
            ResultCategory::Error(ResultError::DownstreamMissingPackage) => {
                b"DownstreamMissingPackage"
            }
            ResultCategory::Error(ResultError::DownstreamMissingVersion) => {
                b"DownstreamMissingVersion"
            }
            ResultCategory::Error(ResultError::FromMissing) => b"FromMissing",
            ResultCategory::Error(ResultError::GaveUp) => b"GaveUp",
            ResultCategory::Error(ResultError::RemovedDep) => b"RemovedDep",
            ResultCategory::Error(ResultError::SolveError) => b"SolveError",
            ResultCategory::Error(ResultError::MiscError) => b"MiscError",
        };

        out.write_all(as_str)?;

        Ok(serialize::IsNull::No)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SolveResult {
    pub solve_time: DateTime<Utc>,
    pub downstream_version: Semver,
    pub update_versions: Vec<Semver>,
}

type SolveResultRecordSql = (Timestamptz, SemverStruct, Array<SemverStruct>);

impl ToSql<SolveResultSql, Pg> for SolveResult {
    fn to_sql(&self, out: &mut Output<Pg>) -> serialize::Result {
        let record: (&DateTime<Utc>, &Semver, &Vec<Semver>) = (
            &self.solve_time,
            &self.downstream_version,
            &self.update_versions,
        );
        WriteTuple::<SolveResultRecordSql>::write_tuple(&record, out)
    }
}

pub mod async_pool {
    use diesel::{
        sql_query,
        sql_types::{Int8, Text},
    };
    use postgres_db::connection::async_pool::{DbConnection, QueryRunner};

    // use crate::historic_solver_job_inputs;

    use super::historic_solver_job_results;
    use super::Job;
    use super::JobResult;

    pub async fn handle_get_jobs(num_jobs: i64, node_name: &str, db: &DbConnection) -> Vec<Job> {
        if node_name == "TEST" {
            // TEST QUERY
            let query = sql_query(
                r#"
                WITH cte AS MATERIALIZED (
                    SELECT update_from_id, update_to_id, downstream_package_id
                    FROM   historic_solver_job_inputs
                    WHERE  job_state = 'none'
                    AND    update_from_id = 29256283 AND update_to_id = 29528818 AND downstream_package_id = 2465926
                    ORDER BY update_from_id, downstream_package_id
                    LIMIT  $1
                    FOR    UPDATE SKIP LOCKED
                    )
                 SELECT
                 job.update_from_id,
                 job.update_to_id, 
                 job.downstream_package_id,
                 job.update_package_name,
                 job.update_from_version,
                 job.update_to_version,
                 job.update_to_time,
                 job.downstream_package_name
                 FROM historic_solver_job_inputs job, cte
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
                FROM   historic_solver_job_inputs
                WHERE  job_state = 'none'
                ORDER BY update_from_id desc, downstream_package_id desc
                LIMIT  $1
                FOR    UPDATE SKIP LOCKED
                )
             UPDATE historic_solver_job_inputs job
             SET    job_state = 'started', start_time = now(), work_node = $2
             FROM   cte
             WHERE  job.update_from_id = cte.update_from_id AND job.update_to_id = cte.update_to_id AND job.downstream_package_id = cte.downstream_package_id
             RETURNING 
             job.update_from_id, 
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

    pub async fn handle_submit_result(
        job_result: JobResult,
        db: &DbConnection,
    ) -> Result<(), String> {
        use crate::historic_solver_job_inputs::dsl::*;
        use diesel::prelude::*;

        let query = diesel::insert_into(historic_solver_job_results::table).values(&job_result);

        db.execute(query).await.unwrap();

        let done_query = diesel::update(
            historic_solver_job_inputs.filter(
                update_from_id
                    .eq(job_result.update_from_id)
                    .and(update_to_id.eq(job_result.update_to_id))
                    .and(downstream_package_id.eq(job_result.downstream_package_id)),
            ),
        )
        .set(job_state.eq("done".to_string()));

        db.execute(done_query).await.unwrap();

        Ok(())
    }
}

pub mod packument_requests {
    use std::sync::Arc;

    use chrono::{DateTime, Utc};
    use moka::future::Cache;
    use serde_json::{Map, Value};

    #[derive(Clone)]
    pub struct ParsedPackument<T> {
        pub latest_tag: T,
        pub versions: Map<String, Value>,
        pub sorted_times: Vec<(String, DateTime<Utc>)>, // sorted by the date
        pub modified_time: DateTime<Utc>,
        pub created_time: DateTime<Utc>,
    }

    pub type NpmCache = Cache<String, Option<Arc<ParsedPackument<()>>>>;

    pub fn restrict_time(
        packument: &ParsedPackument<()>,
        maybe_filter_time: Option<DateTime<Utc>>,
        package_name: &str,
    ) -> Option<ParsedPackument<String>> {
        let filter_time = maybe_filter_time.unwrap_or(DateTime::<Utc>::MAX_UTC);

        let first_bad_time_idx = packument
            .sorted_times
            .partition_point(|(_, vt)| *vt <= filter_time);
        if first_bad_time_idx == 0 {
            // Everything must be filtered out, so we bail early with None
            return None;
        }

        let last_good_time_idx = first_bad_time_idx - 1;
        let (_, last_good_time) = &packument.sorted_times[last_good_time_idx];

        let good_times = &packument.sorted_times[..first_bad_time_idx];
        let good_versions: Map<String, Value> = good_times
            .iter()
            .map(|(v_name, _)| {
                (
                    v_name.clone(),
                    packument
                        .versions
                        .get(v_name)
                        .expect(&format!(
                            "version must exist, pkg = {}, v = {}",
                            package_name, v_name
                        ))
                        .clone(),
                )
            })
            .collect();

        let last_non_beta_good_version = good_times
            .iter()
            .rev()
            .find(|(v_name, _)| !v_name.contains('-') && !v_name.contains('+'))
            .map(|(v_name, _)| v_name.to_owned());

        Some(ParsedPackument {
            latest_tag: last_non_beta_good_version.unwrap(),
            versions: good_versions,
            sorted_times: good_times.to_vec(),
            modified_time: *last_good_time,
            created_time: packument.created_time,
        })
    }

    pub fn parse_datetime(x: &str) -> DateTime<Utc> {
        let dt = DateTime::parse_from_rfc3339(x)
            .or_else(|_| DateTime::parse_from_rfc3339(&format!("{}Z", x)))
            .unwrap();
        dt.with_timezone(&Utc)
    }

    pub fn parse_packument(
        mut j: Map<String, Value>,
        package_name: &str,
    ) -> Option<ParsedPackument<()>> {
        let versions = j.remove("versions")?;

        let mut time = j
            .remove("time")
            .expect(format!("time must be present: {}", package_name).as_str());

        let mut versions = match versions {
            Value::Object(o) => o,
            _ => panic!("versions must be an object"),
        };

        // remove checksums from versions
        for version_blob in versions.values_mut() {
            if let Some(dist_obj) = version_blob
                .as_object_mut()
                .and_then(|version_obj| version_obj.get_mut("dist"))
                .and_then(|dist_blob| dist_blob.as_object_mut())
            {
                if let Some(tarball) = dist_obj.get("tarball") {
                    let tarball = tarball.clone();
                    dist_obj.clear();
                    dist_obj.insert("tarball".to_owned(), tarball);
                }
            }

            if let Some(vobj) = version_blob.as_object_mut() {
                vobj.retain(|k, _| {
                    k == "name"
                        || k == "version"
                        || k == "_id"
                        || k == "dist"
                        || k == "dependencies"
                        || k == "devDependencies"
                        || k == "optionalDependencies"
                        || k == "peerDependencies"
                        || k == "peerDependenciesMeta"
                        || k == "overrides"
                        || k == "bundleDependencies"
                });
            }
        }

        // Get modified & created times
        let time = time.as_object_mut().expect("time must be an object");
        let modified_time = parse_datetime(
            time.remove("modified")
                .expect("modified time must exist")
                .as_str()
                .expect("time must be a string"),
        );

        let created_time = parse_datetime(
            time.remove("created")
                .expect("created time must exist")
                .as_str()
                .expect("time must be a string"),
        );

        // Remove all betas, and interesect time versions and verion versions
        time.retain(|v_name, _| {
            if v_name.contains('-') || v_name.contains('+') {
                false
            } else {
                versions.contains_key(v_name)
            }
        });

        versions.retain(|v_name, _| time.contains_key(v_name));

        let mut sorted_times: Vec<_> = std::mem::take(time)
            .into_iter()
            .map(|(v, dt_str)| {
                (
                    v,
                    parse_datetime(dt_str.as_str().expect("dates must be strings")),
                )
            })
            .collect();

        sorted_times.sort_by_key(|(_, dt)| *dt);

        Some(ParsedPackument {
            latest_tag: (),
            versions,
            sorted_times,
            modified_time,
            created_time,
        })
    }
}

#[derive(Clone)]
pub struct MaxConcurrencyClient {
    client: ClientWithMiddleware,
    semaphore: Arc<tokio::sync::Semaphore>,
}

impl MaxConcurrencyClient {
    pub fn new(client: ClientWithMiddleware, max_concurrency: usize) -> Self {
        MaxConcurrencyClient {
            client,
            semaphore: Arc::new(tokio::sync::Semaphore::new(max_concurrency)),
        }
    }

    pub async fn get<U: IntoUrl + std::fmt::Debug>(&self, url: U) -> Value {
        let permit = self.semaphore.acquire().await.unwrap();
        // println!("GET: {:?}", url);

        let res = self.get_retry(url).await;

        // println!("{}", res);
        drop(permit);
        res
    }

    async fn get_retry<U: IntoUrl + std::fmt::Debug>(&self, url: U) -> Value {
        let u: Url = url.into_url().unwrap();
        let mut i = 0;
        loop {
            if i >= 10 {
                panic!("stoping after 10 retries");
            }
            
            if let Ok(send_ok) = self.client.get(u.clone()).send().await {
                if let Ok(this_resp) = send_ok.json::<Value>().await {
                    return this_resp;
                }
            }

            i += 1;

            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
        }
    }
}
