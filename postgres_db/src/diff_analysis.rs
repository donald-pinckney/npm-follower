use std::collections::HashMap;

use crate::connection::QueryRunner;

use super::schema::diff_analysis;
use diesel::Queryable;

use super::connection::DbConnection;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Queryable, Insertable, Debug, Clone)]
#[table_name = "diff_analysis"]
struct DiffAnalysisSql {
    pub from_id: i64,
    pub to_id: i64,
    pub job_result: serde_json::Value,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase", tag = "t", content = "d")]
pub enum DiffAnalysisJobResult {
    Diff(HashMap<String, FileDiff>),
    ErrTooManyFiles(usize),
    ErrClient(String),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileDiff {
    pub added: usize,
    pub removed: usize,
    pub total_old: Option<usize>,
    pub total_new: Option<usize>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DiffAnalysis {
    pub from_id: i64,
    pub to_id: i64,
    pub job_result: DiffAnalysisJobResult,
}

impl From<DiffAnalysisSql> for DiffAnalysis {
    fn from(sql: DiffAnalysisSql) -> Self {
        let job_result =
            serde_json::from_value(sql.job_result).expect("Failed to deserialize job result");
        Self {
            from_id: sql.from_id,
            to_id: sql.to_id,
            job_result,
        }
    }
}

impl From<DiffAnalysis> for DiffAnalysisSql {
    fn from(diff_analysis: DiffAnalysis) -> Self {
        let job_result =
            serde_json::to_value(diff_analysis.job_result).expect("Failed to serialize job result");
        Self {
            from_id: diff_analysis.from_id,
            to_id: diff_analysis.to_id,
            job_result,
        }
    }
}

pub fn insert_diff_analysis(
    conn: &mut DbConnection,
    from_id: i64,
    to_id: i64,
    job_result: DiffAnalysisJobResult,
) -> Result<(), diesel::result::Error> {
    let new_diff_analysis = DiffAnalysisSql {
        from_id,
        to_id,
        job_result: serde_json::to_value(job_result).expect("Failed to serialize job result"),
    };
    conn.execute(diesel::insert_into(diff_analysis::table).values(&new_diff_analysis))?;
    Ok(())
}
