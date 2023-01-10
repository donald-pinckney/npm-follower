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
use serde::{Deserialize, Serialize};

// job input table:
// update_from_id   |   update_to_id   |   downstream_package_id   |  state ("none", "started", "done")  |  start_time  |  work_node  |  update_package_name    |   update_from_version    |    update_to_version    |   update_to_time    |    downstream_package_name
// PK(update_from_id, update_to_id, downstream_package_id)

// job result table:
// update_from_id   |   update_to_id   |   downstream_package_id   |   result_category ("missing_dep", "gave_up", "error", "ok")   |   [(solve_time, [v])]
// PK(update_from_id, update_to_id, downstream_package_id)

diesel::table! {
    use diesel::sql_types::*;
    use postgres_db::schema::sql_types::SemverStruct;

    job_inputs (update_from_id, update_to_id, downstream_package_id) {
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

#[derive(diesel::sql_types::SqlType)]
#[diesel(postgres_type(name = "Text"))]
pub struct ResultCategorySql;

diesel::table! {
    use diesel::sql_types::*;
    use postgres_db::schema::sql_types::SemverStruct;
    use super::{SolveResultSql, ResultCategorySql};

    job_results (update_from_id, update_to_id, downstream_package_id) {
        update_from_id -> Int8,
        update_to_id -> Int8,
        downstream_package_id -> Int8,
        result_category -> ResultCategorySql, // see below
        solve_history -> Array<SolveResultSql>, // [(solve_time, [v])]
    }
}

#[derive(Debug, Serialize, Deserialize, Queryable, QueryableByName)]
#[diesel(table_name = job_inputs)]
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
#[diesel(table_name = job_results)]
pub struct JobResult {
    pub update_from_id: i64,
    pub update_to_id: i64,
    pub downstream_package_id: i64,
    pub result_category: ResultCategory, // ("FromMissing", "GaveUp", "RemovedDep", "SolveError", "Ok", "MiscError")
    pub solve_history: Vec<SolveResult>,
}

#[derive(Debug, Serialize, Deserialize, AsExpression)]
#[diesel(sql_type = ResultCategorySql)]
pub enum ResultCategory {
    Ok,
    Error(ResultError),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ResultError {
    DownstreamMissing,
    FromMissing,
    GaveUp,
    RemovedDep,
    SolveError,
    MiscError,
}

impl ToSql<ResultCategorySql, Pg> for ResultCategory {
    fn to_sql(&self, out: &mut Output<Pg>) -> serialize::Result {
        let as_str: &'static [u8] = match self {
            ResultCategory::Ok => b"Ok",
            ResultCategory::Error(ResultError::DownstreamMissing) => b"DownstreamMissing",
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
    pub update_versions: Vec<Semver>,
}

type SolveResultRecordSql = (Timestamptz, Array<SemverStruct>);

impl ToSql<SolveResultSql, Pg> for SolveResult {
    fn to_sql(&self, out: &mut Output<Pg>) -> serialize::Result {
        let record: (&DateTime<Utc>, &Vec<Semver>) = (&self.solve_time, &self.update_versions);
        WriteTuple::<SolveResultRecordSql>::write_tuple(&record, out)
    }
}

pub mod async_pool {
    use diesel::{
        sql_query,
        sql_types::{Int8, Text},
    };
    use postgres_db::connection::async_pool::{DbConnection, QueryRunner};

    use super::job_results;
    use super::Job;
    use super::JobResult;

    pub async fn handle_get_jobs(num_jobs: i64, node_name: &str, db: &DbConnection) -> Vec<Job> {
        if node_name == "TEST" {
            // TEST QUERY
            let query = sql_query(
                r#"
                WITH cte AS MATERIALIZED (
                    SELECT update_from_id, update_to_id, downstream_package_id
                    FROM   historic_solver.job_inputs
                    WHERE  job_state = 'none'
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
                LIMIT  $1
                FOR    UPDATE SKIP LOCKED
                )
             UPDATE job_inputs job
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
        let query = diesel::insert_into(job_results::table).values(&job_result);

        db.execute(query).await.unwrap();

        Ok(())
    }
}

pub mod packument_requests {
    use std::sync::Arc;

    use chrono::{DateTime, Utc};
    use moka::future::Cache;
    use serde_json::{Map, Value};

    #[derive(Clone)]
    pub struct ParsedPackument {
        pub latest_tag: Option<String>,
        pub versions: Map<String, Value>,
        pub sorted_times: Vec<(String, DateTime<Utc>)>, // sorted by the date
        pub modified_time: DateTime<Utc>,
        pub created_time: DateTime<Utc>,
    }

    pub type NpmCache = Cache<String, Option<Arc<ParsedPackument>>>;

    pub fn restrict_time(
        packument: &ParsedPackument,
        maybe_filter_time: Option<DateTime<Utc>>,
    ) -> Option<ParsedPackument> {
        let filter_time = maybe_filter_time.unwrap_or(DateTime::<Utc>::MAX_UTC);

        let first_bad_time_idx = packument
            .sorted_times
            .partition_point(|(_, vt)| *vt <= filter_time);
        if first_bad_time_idx == 0 {
            // Everything must be filtered out, so we bail early with None
            return None;
        } else if first_bad_time_idx == packument.sorted_times.len() {
            // Nothing is filtered out
            return Some(packument.clone());
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
                        .expect("version must exist")
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
            latest_tag: last_non_beta_good_version,
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

    pub fn parse_packument(mut j: Map<String, Value>) -> ParsedPackument {
        let latest_tag = {
            let dist_tags = j.remove("dist-tags").expect("dist-tags must be present");
            dist_tags
                .as_object()
                .expect("dist-tags must be an object")
                .get("latest")
                .expect("latest tag must exist")
                .as_str()
                .expect("latest tag must be a string")
                .to_owned()
        };

        let versions = j.remove("versions").expect("versions must be present");
        let mut time = j.remove("time").expect("time must be present");

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
        }

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

        ParsedPackument {
            latest_tag: Some(latest_tag),
            versions,
            sorted_times,
            modified_time,
            created_time,
        }
    }
}
// pub mod sync {
//     use diesel::{
//         sql_query,
//         sql_types::{Int8, Text},
//     };
//     use postgres_db::connection::{DbConnection, QueryRunner};
//     use warp::http;

//     use super::job_results;
//     use super::Job;
//     use super::JobResult;

//     pub fn handle_get_jobs(num_jobs: i64, node_name: String, db: &mut DbConnection) -> Vec<Job> {
//         if node_name == "TEST" {
//             // TEST QUERY
//             let query = sql_query(
//                 r#"
//                 WITH cte AS MATERIALIZED (
//                     SELECT update_from_id, update_to_id, downstream_package_id
//                     FROM   historic_solver.job_inputs
//                     WHERE  job_state = 'none'
//                     ORDER BY update_from_id, downstream_package_id
//                     LIMIT  ?
//                     FOR    UPDATE SKIP LOCKED
//                     )
//                  SELECT
//                  job.update_to_id,
//                  job.downstream_package_id,
//                  job.update_package_name,
//                  job.update_from_version,
//                  job.update_to_version,
//                  job.update_to_time,
//                  job.downstream_package_name
//                  FROM job_inputs job, cte
//                  WHERE  job.update_from_id = cte.update_from_id AND job.update_to_id = cte.update_to_id AND job.downstream_package_id = cte.downstream_package_id;
//             "#
//             )
//             .bind::<Int8, _>(num_jobs);

//             db.get_results(query).unwrap()
//         } else {
//             // REAL QUERY
//             let query = sql_query(
//             r#"
//             WITH cte AS MATERIALIZED (
//                 SELECT update_from_id, update_to_id, downstream_package_id
//                 FROM   historic_solver.job_inputs
//                 WHERE  job_state = 'none'
//                 ORDER BY update_from_id, downstream_package_id
//                 LIMIT  ?
//                 FOR    UPDATE SKIP LOCKED
//                 )
//              UPDATE job_inputs job
//              SET    job_state = 'started', start_time = now(), work_node = ?
//              FROM   cte
//              WHERE  job.update_from_id = cte.update_from_id AND job.update_to_id = cte.update_to_id AND job.downstream_package_id = cte.downstream_package_id
//              RETURNING job.update_from_id,
//              job.update_to_id,
//              job.downstream_package_id,
//              job.update_package_name,
//              job.update_from_version,
//              job.update_to_version,
//              job.update_to_time,
//              job.downstream_package_name;
//             "#
//             )
//             .bind::<Int8, _>(num_jobs)
//             .bind::<Text, _>(node_name);

//             db.get_results(query).unwrap()
//         }
//     }

//     pub fn handle_submit_result(
//         job_result: JobResult,
//         db: &mut DbConnection,
//     ) -> Result<impl warp::Reply, warp::Rejection> {
//         let query = diesel::insert_into(job_results::table).values(&job_result);

//         db.execute(query).unwrap();

//         Ok(warp::reply::with_status(
//             "Result submitted",
//             http::StatusCode::CREATED,
//         ))
//     }
// }
