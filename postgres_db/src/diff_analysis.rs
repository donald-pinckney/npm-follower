use std::collections::HashMap;

use crate::connection::QueryRunner;

use super::schema::tarball_analysis::diff_analysis;
use diesel::{upsert::excluded, Queryable};

use diesel::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Queryable, Insertable, Debug, Clone)]
#[diesel(table_name = diff_analysis)]
struct DiffAnalysisSql {
    pub from_id: i64,
    pub to_id: i64,
    pub job_result: serde_json::Value,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase", tag = "t", content = "d")]
pub enum DiffAnalysisJobResult {
    Diff(HashMap<String, FileDiff>),
    ErrTooManyFiles(usize, usize), // old, new
    ErrUnParseable,
    ErrClient(String),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileDiff {
    // renaming for compactness
    #[serde(rename = "a")]
    pub added: usize,
    #[serde(rename = "r")]
    pub removed: usize,
    #[serde(rename = "to")]
    pub total_old: Option<usize>,
    #[serde(rename = "tn")]
    pub total_new: Option<usize>,
    #[serde(rename = "w")]
    pub average_width: f64,
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

pub fn insert_diff_analysis<R: QueryRunner>(
    conn: &mut R,
    diff: DiffAnalysis,
) -> Result<(), diesel::result::Error> {
    let diff: DiffAnalysisSql = diff.into();
    conn.execute(
        diesel::insert_into(diff_analysis::table)
            .values(&diff)
            .on_conflict((diff_analysis::from_id, diff_analysis::to_id))
            .do_update()
            .set((
                diff_analysis::from_id.eq(excluded(diff_analysis::from_id)),
                diff_analysis::to_id.eq(excluded(diff_analysis::to_id)),
                diff_analysis::job_result.eq(excluded(diff_analysis::job_result)),
            )),
    )?;
    Ok(())
}

pub fn insert_batch_diff_analysis<R: QueryRunner>(
    conn: &mut R,
    diffs: Vec<DiffAnalysis>,
) -> Result<(), diesel::result::Error> {
    let diffs: Vec<DiffAnalysisSql> = diffs.into_iter().map(|d| d.into()).collect();
    conn.execute(
        diesel::insert_into(diff_analysis::table)
            .values(&diffs)
            .on_conflict((diff_analysis::from_id, diff_analysis::to_id))
            .do_update()
            .set((
                diff_analysis::from_id.eq(excluded(diff_analysis::from_id)),
                diff_analysis::to_id.eq(excluded(diff_analysis::to_id)),
                diff_analysis::job_result.eq(excluded(diff_analysis::job_result)),
            )),
    )?;
    Ok(())
}

pub fn count_diff_analysis<R: QueryRunner>(conn: &mut R) -> Result<i64, diesel::result::Error> {
    use crate::schema::tarball_analysis::diff_analysis::dsl::*;
    let count = conn.get_result(diff_analysis.count())?;
    Ok(count)
}

pub fn query_table<R: QueryRunner>(
    conn: &mut R,
    limit: Option<i64>,
    last: Option<(i64, i64)>,
) -> Result<Vec<DiffAnalysis>, diesel::result::Error> {
    use super::schema::tarball_analysis::diff_analysis::dsl::*;
    let results: Vec<DiffAnalysisSql> = match (limit, last) {
        (Some(limit), Some(last)) => conn.load(
            diff_analysis
                .filter(
                    from_id
                        .gt(last.0)
                        .or(from_id.eq(last.0).and(to_id.gt(last.1))),
                )
                .order((from_id.asc(), to_id.asc()))
                .limit(limit),
        )?,
        (Some(limit), None) => conn.load(
            diff_analysis
                .limit(limit)
                .order((from_id.asc(), to_id.asc())),
        )?,
        (None, Some(last)) => conn.load(
            diff_analysis
                .filter(
                    from_id
                        .gt(last.0)
                        .or(from_id.eq(last.0).and(to_id.gt(last.1))),
                )
                .order((from_id.asc(), to_id.asc())),
        )?,
        (None, None) => conn.load(diff_analysis.order((from_id.asc(), to_id.asc())))?,
    };

    Ok(results.into_iter().map(|d| d.into()).collect())
}
