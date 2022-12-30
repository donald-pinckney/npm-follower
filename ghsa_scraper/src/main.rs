use postgres_db::connection::{DbConnection, QueryRunner};

use std::collections::{HashMap, HashSet};

use graphql_client::{GraphQLQuery, Response};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub type URI = String;
pub type DateTime = String; // TODO: change this to chrono time

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/github.graphql",
    query_path = "src/query.graphql",
    response_derives = "Debug, Deserialize, Serialize, Clone"
)]
struct QueryAllGHSA;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityVulnerability {
    pub advisory: Advisory,
    pub severity: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Package {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FirstPatchedVersion {
    pub identifier: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Advisory {
    pub ghsa_id: String,
    pub summary: String,
    pub description: String,
    pub references: Vec<Reference>,
    pub withdrawn_at: Option<DateTime>,
    pub updated_at: DateTime,
    pub published_at: DateTime,
    pub cvss: Option<Cvss>,
    pub vulnerabilities: VulnNodes,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VulnNodes {
    pub nodes: Vec<VulnNode>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VulnNode {
    pub vulnerable_version_range: String,
    pub package: Package,
    pub first_patched_version: Option<FirstPatchedVersion>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reference {
    pub url: URI,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cvss {
    pub score: f32,
    pub vector_string: Option<String>,
}

#[derive(Debug, Error)]
pub enum GQLError {
    #[error("GraphQL query failed: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("Malformed GraphQL response: {0}")]
    MalformedResponse(String),
    #[error("Serde error: {0}")]
    Serde(#[from] serde_json::Error),
}

pub async fn scrape_ghsa(
    token: &str,
    mut cursor: Option<String>,
) -> Result<(Vec<SecurityVulnerability>, Option<String>), GQLError> {
    let mut scraped_vulns: Vec<SecurityVulnerability> = vec![];
    let mut ghsa_ids = HashSet::new(); // to avoid dups, graphql is flaky
    loop {
        let query = QueryAllGHSA::build_query(query_all_ghsa::Variables { cursor });
        let res = reqwest::Client::new()
            .post("https://api.github.com/graphql")
            .bearer_auth(token.to_string())
            // github's graphql api requires user-agent header
            .header("User-Agent", "much scraper, much win")
            .json(&query)
            .send()
            .await?;
        let res: Response<query_all_ghsa::ResponseData> = res.json().await?;
        let data: query_all_ghsa::ResponseData = res.data.ok_or_else(|| {
            let errs = res
                .errors
                .unwrap_or_default()
                .iter()
                .map(|e| e.message.clone())
                .collect::<Vec<_>>()
                .join(", ");
            GQLError::MalformedResponse(errs)
        })?;
        let vulns =
            serde_json::to_value(data.security_vulnerabilities.nodes.ok_or_else(|| {
                GQLError::MalformedResponse("SecurityVulnerabilities.nodes was null".to_string())
            })?)?;
        let vulns: Vec<SecurityVulnerability> = serde_json::from_value(vulns)?;
        let num_vulns = vulns.len();
        println!("Scraped {} vulns", num_vulns);
        println!(
            "Cursor: {:?}",
            data.security_vulnerabilities.page_info.end_cursor
        );
        cursor = data.security_vulnerabilities.page_info.end_cursor;
        for vuln in vulns {
            if !ghsa_ids.contains(&vuln.advisory.ghsa_id) {
                ghsa_ids.insert(vuln.advisory.ghsa_id.clone());
                scraped_vulns.push(vuln);
            }
        }
        if !data.security_vulnerabilities.page_info.has_next_page {
            break;
        }
    }
    println!("In total, found {} vulns", scraped_vulns.len());
    Ok((scraped_vulns, cursor))
}

fn insert_ghsa<R>(conn: &mut R, vulns: Vec<SecurityVulnerability>)
where
    R: QueryRunner,
{
    for vuln in vulns {
        let mut packages = Vec::new();
        let mut vulnmap = postgres_db::ghsa::VulnMap {
            vulns: HashMap::new(),
        };

        for node in vuln.advisory.vulnerabilities.nodes {
            let patch = node.first_patched_version.map(|v| v.identifier);
            let vuln = node.vulnerable_version_range;
            vulnmap
                .vulns
                .insert(node.package.name.clone(), (vuln, patch));
            packages.push(Some(node.package.name));
        }

        // convert timezones to UTC
        let withdrawn_at = vuln.advisory.withdrawn_at.map(|s| {
            chrono::DateTime::parse_from_rfc3339(&s)
                .unwrap()
                .with_timezone(&chrono::Utc)
        });
        let updated_at = chrono::DateTime::parse_from_rfc3339(&vuln.advisory.updated_at)
            .unwrap()
            .with_timezone(&chrono::Utc);
        let published_at = chrono::DateTime::parse_from_rfc3339(&vuln.advisory.published_at)
            .unwrap()
            .with_timezone(&chrono::Utc);

        let ghsa_db_struct = postgres_db::ghsa::Ghsa {
            id: vuln.advisory.ghsa_id,
            severity: vuln.severity,
            description: vuln.advisory.description,
            summary: vuln.advisory.summary,
            withdrawn_at,
            published_at,
            updated_at,
            refs: vuln
                .advisory
                .references
                .iter()
                .map(|r| Some(r.url.to_string()))
                .collect(),
            cvss_score: vuln.advisory.cvss.as_ref().map(|c| c.score),
            cvss_vector: vuln
                .advisory
                .cvss
                .filter(|c| c.vector_string.is_some())
                .map(|c| c.vector_string.unwrap()),
            packages,
            vulns: serde_json::to_value(vulnmap.vulns).unwrap(),
        };
        postgres_db::ghsa::insert_ghsa(conn, ghsa_db_struct);
    }
}

#[tokio::main]
async fn main() {
    utils::check_no_concurrent_processes("ghsa_scraper");
    dotenvy::from_filename(".secret.env").expect("failed to load .secret.env. To setup GHSA scraping, run:\necho \"export GITHUB_TOKEN=<TYPE API TOKEN HERE>\" >> .secret.env\n\nThe token must be a Github PAT with read:packages permission.\n\n");

    let mut conn = DbConnection::connect();
    let github_token = std::env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN env var not set");

    let last_cursor = postgres_db::internal_state::query_gha_pointer(&mut conn);
    let (vulns, next_cursor) = scrape_ghsa(&github_token, last_cursor)
        .await
        .unwrap_or_else(|e| {
            println!("Error: {}", e);
            std::process::exit(1);
        });

    conn.run_psql_transaction(|mut conn| {
        insert_ghsa(&mut conn, vulns);

        if let Some(cur) = next_cursor {
            postgres_db::internal_state::set_gha_pointer(cur, &mut conn);
        }

        Ok(((), true))
    })
    .unwrap();
}
