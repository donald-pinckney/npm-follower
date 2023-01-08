use std::io::Write;

use chrono::{DateTime, Utc};
use diesel::{
    pg::Pg,
    prelude::*,
    serialize::{self, Output, ToSql, WriteTuple},
    sql_types::{Array, Timestamptz},
    AsExpression,
};
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
    FromMissing,
    GaveUp,
    RemovedDep,
    SolveError,
    Ok,
    MiscError,
}

impl ToSql<ResultCategorySql, Pg> for ResultCategory {
    fn to_sql(&self, out: &mut Output<Pg>) -> serialize::Result {
        let as_str: &'static [u8] = match self {
            ResultCategory::FromMissing => b"FromMissing",
            ResultCategory::GaveUp => b"GaveUp",
            ResultCategory::RemovedDep => b"RemovedDep",
            ResultCategory::SolveError => b"SolveError",
            ResultCategory::Ok => b"Ok",
            ResultCategory::MiscError => b"MiscError",
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
