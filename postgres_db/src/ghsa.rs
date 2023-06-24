use super::connection::DbConnection;
use super::schema;
use super::schema::ghsa;
use super::schema::vulnerabilities;
use super::schema::cwes;
use super::schema::ghsa_cwe_relation;
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
    pub refs: Vec<String>,
    pub cvss_score: Option<f32>,
    pub cvss_vector: Option<String>,
}

#[derive(Insertable, Debug, Clone)]
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

#[derive(Queryable, Debug, Clone)]
#[diesel(table_name = vulnerabilities)]
struct GhsaVulnerabilityRow {
    _id: i64,
    ghsa_id: String,
    package_name: String,
    vulnerable_version_lower_bound: Option<Semver>,
    vulnerable_version_lower_bound_inclusive: bool,
    vulnerable_version_upper_bound: Option<Semver>,
    vulnerable_version_upper_bound_inclusive: bool,
    first_patched_version: Option<Semver>,
}

#[derive(Insertable, Debug, Clone)]
#[diesel(table_name = cwes)]
pub struct Cwe {
    pub id: String,
    pub name: String,
    pub description: String
}


#[derive(Insertable, Debug, Clone)]
#[diesel(table_name = ghsa_cwe_relation)]
pub struct GhsaCweRelation {
    pub ghsa_id: String,
    pub cwe_id: String
}

pub fn insert_ghsa<R>(conn: &mut R, advisory: Ghsa, vulns: Vec<GhsaVulnerability>)
where
    R: QueryRunner,
{
    use schema::ghsa::dsl::*;

    // todo!()

    let insert_ghsa_query = diesel::insert_into(ghsa)
        .values(&advisory)
        .on_conflict(id)
        .do_update()
        .set((
            severity.eq(excluded(severity)),
            description.eq(excluded(description)),
            summary.eq(excluded(summary)),
            withdrawn_at.eq(excluded(withdrawn_at)),
            published_at.eq(excluded(published_at)),
            updated_at.eq(excluded(updated_at)),
            refs.eq(excluded(refs)),
            cvss_score.eq(excluded(cvss_score)),
            cvss_vector.eq(excluded(cvss_vector)),
        ));

    conn.execute(insert_ghsa_query)
        .expect("Failed to insert ghsa");

    // Probably not very efficient to delete rows and re-insert rather than updating,
    // but should be fine for these fairly small tables.

    let delete_old_query =
        diesel::delete(vulnerabilities::table).filter(vulnerabilities::ghsa_id.eq(advisory.id));

    conn.execute(delete_old_query)
        .expect("Failed to delete old vulnerabilities");

    let insert_vulns_query = diesel::insert_into(vulnerabilities::table).values(vulns);

    conn.execute(insert_vulns_query)
        .expect("Failed to insert vulnerabilities");
}

pub fn query_ghsa_by_id(conn: &mut DbConnection, ghsa_id: &str) -> (Ghsa, Vec<GhsaVulnerability>) {
    use schema::ghsa::dsl::*;

    // let query = ghsa.filter(id.eq(ghsa_id));

    let adv: Ghsa = conn
        .first(ghsa.filter(id.eq(ghsa_id)))
        .unwrap_or_else(|_err| panic!("Failed to find ghsa with id {}", ghsa_id));

    let vuln_rows: Vec<GhsaVulnerabilityRow> = conn
        .load(vulnerabilities::table.filter(vulnerabilities::ghsa_id.eq(ghsa_id)))
        .unwrap_or_else(|_err| {
            panic!(
                "Failed to query vulnerabilities for ghsa with id {}",
                ghsa_id
            )
        });

    let vulns = vuln_rows
        .into_iter()
        .map(|row| GhsaVulnerability {
            ghsa_id: row.ghsa_id,
            package_name: row.package_name,
            vulnerable_version_lower_bound: row.vulnerable_version_lower_bound,
            vulnerable_version_lower_bound_inclusive: row.vulnerable_version_lower_bound_inclusive,
            vulnerable_version_upper_bound: row.vulnerable_version_upper_bound,
            vulnerable_version_upper_bound_inclusive: row.vulnerable_version_upper_bound_inclusive,
            first_patched_version: row.first_patched_version,
        })
        .collect();

    (adv, vulns)
}

const INSERT_CHUNK_SIZE: usize = 256;

pub fn insert_cwes<R>(conn: &mut R, cwes_to_insert: Vec<Cwe>)
where
    R: QueryRunner,
{
    let mut chunk_iter = cwes_to_insert.chunks_exact(INSERT_CHUNK_SIZE);
    for chunk in &mut chunk_iter {
        insert_cwes_chunk(conn, chunk);
    }

    insert_cwes_chunk(conn, chunk_iter.remainder());
}


fn insert_cwes_chunk<R>(conn: &mut R, cwes_to_insert: &[Cwe])
where
    R: QueryRunner,
{
    use schema::cwes::dsl::*;

    let insert_cwes_query = diesel::insert_into(cwes)
        .values(cwes_to_insert)
        .on_conflict(id)
        .do_update()
        .set((
            name.eq(excluded(name)),
            description.eq(excluded(description))
        ));

    conn.execute(insert_cwes_query)
        .expect("Failed to insert cwes");
}


pub fn associate_ghsa_to_cwe<R>(conn: &mut R, assoc: Vec<GhsaCweRelation>)
where
    R: QueryRunner,
{
    let mut chunk_iter = assoc.chunks_exact(INSERT_CHUNK_SIZE);
    for chunk in &mut chunk_iter {
        associate_ghsa_to_cwe_chunk(conn, chunk);
    }

    associate_ghsa_to_cwe_chunk(conn, chunk_iter.remainder());
}

fn associate_ghsa_to_cwe_chunk<R>(conn: &mut R, assoc: &[GhsaCweRelation])
where
    R: QueryRunner,
{
    use schema::ghsa_cwe_relation::dsl::*;

    // TODO: batch
    let insert_rels = diesel::insert_into(ghsa_cwe_relation)
        .values(assoc)
        .on_conflict((ghsa_id, cwe_id))
        .do_nothing();

    conn.execute(insert_rels)
        .expect("Failed to insert rels");
}