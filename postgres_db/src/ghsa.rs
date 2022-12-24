use std::collections::HashMap;

use super::connection::DbConnection;
use super::schema;
use super::schema::ghsa;
use crate::connection::QueryRunner;
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
    pub packages: Vec<Option<String>>, // diesel has problems without having this as an Option...
    // {
    //    "package_name": {
    //      "vulnerable": "< 1.2.3"
    //      "patched": "1.2.3",
    //    }
    // }
    pub vulns: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct VulnMap {
    pub vulns: HashMap<String, (String, Option<String>)>,
}

impl std::ops::Deref for VulnMap {
    type Target = HashMap<String, (String, Option<String>)>;

    fn deref(&self) -> &Self::Target {
        &self.vulns
    }
}

impl VulnMap {
    pub fn new(vulns: serde_json::Value) -> Self {
        let mut map = HashMap::new();
        for (package, vuln) in vulns.as_object().unwrap() {
            let vulnerable = vuln["vulnerable"].as_str().unwrap();
            let patched = vuln["patched"].as_str();
            map.insert(
                package.to_string(),
                (vulnerable.to_string(), patched.map(|s| s.to_string())),
            );
        }
        Self { vulns: map }
    }
}

pub fn insert_ghsa(conn: &mut DbConnection, advisory: Ghsa) {
    use schema::ghsa::dsl::*;
    let query = diesel::insert_into(ghsa)
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
            packages.eq(excluded(packages)),
            vulns.eq(excluded(vulns)),
        ));

    conn.execute(query).expect("Failed to insert ghsa");
}

pub fn query_ghsa_by_id(conn: &mut DbConnection, ghsa_id: &str) -> Option<Ghsa> {
    use schema::ghsa::dsl::*;
    let query = ghsa.filter(id.eq(ghsa_id));

    conn.load(query).expect("Failed to query ghsa").pop()
}
