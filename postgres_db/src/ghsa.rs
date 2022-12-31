use std::collections::HashMap;

use super::connection::DbConnection;
use super::schema;
use super::schema::ghsa;
use super::schema::vulnerabilities;
use crate::connection::QueryRunner;
use crate::custom_types::Semver;
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel::upsert::excluded;
use diesel::Queryable;

#[derive(Queryable, Insertable, Debug, Clone)]
#[diesel(table_name = ghsa)]
pub struct Ghsa {
    pub id: String,
    pub severity: String,
    pub description: String,
    pub summary: String,
    pub withdrawn_at: Option<DateTime<Utc>>,
    pub published_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub refs: Vec<Option<String>>, // diesel has problems without having this as an Option...
    pub cvss_score: Option<f32>,
    pub cvss_vector: Option<String>,
}

#[derive(Queryable, Insertable, Debug, Clone)]
#[diesel(table_name = vulnerabilities)]
pub struct GhsaVulnerability {
    pub ghsa_id: String,
    pub package_name: String,
    pub vulnerable_version_lower_bound: Option<Semver>,
    pub vulnerable_version_lower_bound_inclusive: bool,
    pub vulnerable_version_upper_bound: Option<Semver>,
    pub vulnerable_version_upper_bound_inclusive: bool,
    pub first_patched_version: Option<Semver>,
}

pub fn insert_ghsa<R>(conn: &mut R, advisory: Ghsa, vulnerabilities: Vec<GhsaVulnerability>)
where
    R: QueryRunner,
{
    use schema::ghsa::dsl::*;

    // todo!()

    // let query = diesel::insert_into(ghsa)
    //     .values(&advisory)
    //     .on_conflict(id)
    //     .do_update()
    //     .set((
    //         severity.eq(excluded(severity)),
    //         description.eq(excluded(description)),
    //         summary.eq(excluded(summary)),
    //         withdrawn_at.eq(excluded(withdrawn_at)),
    //         published_at.eq(excluded(published_at)),
    //         updated_at.eq(excluded(updated_at)),
    //         refs.eq(excluded(refs)),
    //         cvss_score.eq(excluded(cvss_score)),
    //         cvss_vector.eq(excluded(cvss_vector)),
    //         packages.eq(excluded(packages)),
    //         vulns.eq(excluded(vulns)),
    //     ));

    // conn.execute(query).expect("Failed to insert ghsa");
}

pub fn query_ghsa_by_id(
    conn: &mut DbConnection,
    ghsa_id: &str,
) -> Option<(Ghsa, Vec<GhsaVulnerability>)> {
    use schema::ghsa::dsl::*;
    // let query = ghsa.filter(id.eq(ghsa_id));

    // conn.load(query).expect("Failed to query ghsa").pop()

    todo!()
}
