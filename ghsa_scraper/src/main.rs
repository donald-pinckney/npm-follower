use postgres_db::{
    connection::{DbConnection, QueryRunner},
    custom_types::Semver,
    ghsa::GhsaVulnerability,
};
use semver_spec_serialization::ParseSemverError;

use std::collections::{HashSet, HashMap};

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
pub struct Advisory {
    pub ghsa_id: String,
    pub summary: String,
    pub description: String,
    pub references: Vec<Reference>,
    pub withdrawn_at: Option<DateTime>,
    pub updated_at: DateTime,
    pub published_at: DateTime,
    pub cvss: Option<Cvss>,
    pub cwes: CweNodes,
    pub vulnerabilities: VulnNodes,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cvss {
    pub score: f32,
    pub vector_string: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reference {
    pub url: URI,
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
pub struct CweNodes {
    pub nodes: Vec<CweNode>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CweNode {
    pub cwe_id: String,
    pub description: String,
    pub name: String,
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

#[derive(Debug, Error)]
pub enum GQLError {
    #[error("GraphQL query failed: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("Malformed GraphQL response: {0}")]
    MalformedResponse(String),
    #[error("Serde error: {0}")]
    Serde(#[from] serde_json::Error),
}

pub async fn scrape_ghsa(token: &str) -> Result<Vec<SecurityVulnerability>, GQLError> {
    let mut scraped_vulns: Vec<SecurityVulnerability> = vec![];
    let mut ghsa_ids = HashSet::new(); // to avoid dups, graphql is flaky
    let mut cursor: Option<String> = Option::None;
    loop {
        let query = QueryAllGHSA::build_query(query_all_ghsa::Variables { cursor: cursor.clone() });
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
            "Cursor: {:?}\nNew Cursor: {:?}",
            cursor,
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
    Ok(scraped_vulns)
}

fn insert_ghsa<R>(conn: &mut R, vulns: Vec<SecurityVulnerability>)
where
    R: QueryRunner,
{
    let mut ghsa_cwe_pairs: HashSet<(String, String)> = HashSet::new();
    let mut cwe_info: HashMap<String, (String, String)> = HashMap::new();
    
    for vuln in vulns {
        let vulnerabilities: Vec<GhsaVulnerability> = vuln
            .advisory
            .vulnerabilities
            .nodes
            .into_iter()
            .flat_map(|vuln_node| {
                let patch = vuln_node.first_patched_version.and_then(|v| {
                    let parse_result = parse_version_best_effort(&v.identifier, false);
                    if parse_result.is_err() {
                        println!(
                            "Warning: failed to parse patch semver {}, id = {}",
                            v.identifier, vuln.advisory.ghsa_id
                        );
                    }
                    parse_result.ok().map(|(v, _is_wildcard)| v)
                });

                let (lower_v, lower_inc, upper_v, upper_inc) =
                    parse_range(vuln_node.vulnerable_version_range, &vuln.advisory.ghsa_id);

                Some(GhsaVulnerability {
                    ghsa_id: vuln.advisory.ghsa_id.clone(),
                    package_name: vuln_node.package.name,
                    vulnerable_version_lower_bound: lower_v,
                    vulnerable_version_lower_bound_inclusive: lower_inc,
                    vulnerable_version_upper_bound: upper_v,
                    vulnerable_version_upper_bound_inclusive: upper_inc,
                    first_patched_version: patch,
                })
            })
            .collect();

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

        vuln.advisory.cwes.nodes.into_iter().for_each(|cwe_node| {
            ghsa_cwe_pairs.insert((vuln.advisory.ghsa_id.clone(), cwe_node.cwe_id.clone()));
            cwe_info.insert(
                cwe_node.cwe_id,
                (cwe_node.name, cwe_node.description),
            );
        });

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
                .map(|r| r.url.to_string())
                .collect(),
            cvss_score: vuln.advisory.cvss.as_ref().and_then(|c| {
                if c.vector_string.is_none() && c.score == 0.0 {
                    None
                } else {
                    Some(c.score)
                }
            }),
            cvss_vector: vuln
                .advisory
                .cvss
                .filter(|c| c.vector_string.is_some())
                .map(|c| c.vector_string.unwrap()),
        };
        postgres_db::ghsa::insert_ghsa(conn, ghsa_db_struct, vulnerabilities);
    }


    let cwes_to_insert: Vec<_> = cwe_info.into_iter().map(|(id, (name, description))| {
        postgres_db::ghsa::Cwe {
            id,
            name,
            description
        }
    }).collect();

    let ghsa_cwe_relations_to_insert: Vec<_> = ghsa_cwe_pairs.into_iter().map(|(ghsa_id, cwe_id)| {
        postgres_db::ghsa::GhsaCweRelation {
            ghsa_id,
            cwe_id
        }
    }).collect();

    postgres_db::ghsa::insert_cwes(conn, cwes_to_insert);
    postgres_db::ghsa::associate_ghsa_to_cwe(conn, ghsa_cwe_relations_to_insert);
}

#[tokio::main]
async fn main() {
    utils::check_no_concurrent_processes("ghsa_scraper");
    dotenvy::from_filename(".secret.env").expect("failed to load .secret.env. To setup GHSA scraping, run:\necho \"export GITHUB_TOKEN=<TYPE API TOKEN HERE>\" >> .secret.env\n\nThe token must be a Github PAT with read:packages permission.\n\n");

    let github_token = std::env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN env var not set");

    let vulns = scrape_ghsa(&github_token)
        .await
        .unwrap_or_else(|e| {
            println!("Error: {}", e);
            std::process::exit(1);
        });

    let mut conn = DbConnection::connect();
    conn.run_psql_transaction(|mut conn| {
        insert_ghsa(&mut conn, vulns);
        Ok(((), true))
    })
    .unwrap();
}

fn parse_range(range_str: String, ghsa_id: &str) -> (Option<Semver>, bool, Option<Semver>, bool) {
    let range_components: Vec<_> = range_str.split(", ").collect();

    let (lower_str, lower_inc, upper_str, upper_inc) = if range_components.len() == 1 {
        let comp = range_components[0];
        parse_single_bound(comp)
    } else if range_components.len() == 2 {
        let (lower_v_str, lower_inc) = parse_lower_bound(range_components[0]);
        let (upper_v_str, upper_inc) = parse_upper_bound(range_components[1]);

        (Some(lower_v_str), lower_inc, Some(upper_v_str), upper_inc)
    } else {
        panic!("Invalid range string: {}, id = {}", range_str, ghsa_id);
    };

    // > 1.2 is equivalent to >= 1.3.0
    // >= 1.2 is equivalent to >= 1.2.0
    let lower_v_need_round_up = !lower_inc;

    // < 1.2 is equivalent to < 1.2.0
    // <= 1.2 is equivalent to < 1.3.0
    let upper_v_need_round_up = upper_inc;

    let lower_v = lower_str
        .map(|s| parse_version_best_effort(s, lower_v_need_round_up))
        .transpose();

    let upper_v = upper_str
        .map(|s| parse_version_best_effort(s, upper_v_need_round_up))
        .transpose();

    if lower_v.is_err() {
        panic!(
            "Error: failed to parse lower bound semver {}, id = {}",
            lower_str.unwrap(),
            ghsa_id
        )
    }

    if upper_v.is_err() {
        panic!(
            "Error: failed to parse upper bound semver {}, id = {}",
            upper_str.unwrap(),
            ghsa_id
        )
    }

    let lower_v = lower_v.unwrap();
    let upper_v = upper_v.unwrap();

    let lower_inc = lower_v.as_ref().map(|(_v, is_wildcard)| {
        if lower_v_need_round_up && *is_wildcard {
            !lower_inc
        } else {
            lower_inc
        }
    });

    let upper_inc = upper_v.as_ref().map(|(_v, is_wildcard)| {
        if upper_v_need_round_up && *is_wildcard {
            !upper_inc
        } else {
            upper_inc
        }
    });

    let lower_v = lower_v.map(|(v, _is_wildcard)| v);
    let upper_v = upper_v.map(|(v, _is_wildcard)| v);

    (
        lower_v,
        lower_inc.unwrap_or(true),
        upper_v,
        upper_inc.unwrap_or(true),
    )
}

fn parse_lower_bound(s: &str) -> (&str, bool) {
    if let Some(v_str) = s.strip_prefix(">= ") {
        (v_str, true)
    } else if let Some(v_str) = s.strip_prefix("> ") {
        (v_str, false)
    } else {
        panic!("Invalid lower bound string: {}", s);
    }
}

fn parse_upper_bound(s: &str) -> (&str, bool) {
    if let Some(v_str) = s.strip_prefix("<= ") {
        (v_str, true)
    } else if let Some(v_str) = s.strip_prefix("< ") {
        (v_str, false)
    } else {
        panic!("Invalid upper bound string: {}", s);
    }
}

fn parse_single_bound(s: &str) -> (Option<&str>, bool, Option<&str>, bool) {
    if let Some(v_str) = s.strip_prefix("= ") {
        (Some(v_str), true, Some(v_str), true)
    } else if let Some(v_str) = s.strip_prefix("<= ") {
        (None, true, Some(v_str), true)
    } else if let Some(v_str) = s.strip_prefix("< ") {
        (None, true, Some(v_str), false)
    } else if let Some(v_str) = s.strip_prefix(">= ") {
        (Some(v_str), true, None, true)
    } else if let Some(v_str) = s.strip_prefix("> ") {
        (Some(v_str), false, None, true)
    } else {
        panic!("Invalid single bound string: {}", s);
    }
}

fn parse_version_best_effort(
    s: &str,
    need_round_up: bool,
) -> Result<(Semver, bool), ParseSemverError> {
    match semver_spec_serialization::parse_semver(s) {
        Ok(v) => Ok((v, false)),
        Err(err) => {
            let components: Vec<_> = s.split('.').collect();
            let exact_semver = if components.len() == 2 {
                let major: i64 = components[0].parse()?;
                let minor: i64 = components[1].parse()?;
                if need_round_up {
                    Semver {
                        major,
                        minor: minor + 1,
                        bug: 0,
                        prerelease: vec![],
                        build: vec![],
                    }
                } else {
                    Semver {
                        major,
                        minor,
                        bug: 0,
                        prerelease: vec![],
                        build: vec![],
                    }
                }
            } else if components.len() == 1 {
                let major: i64 = components[0].parse()?;
                if need_round_up {
                    Semver {
                        major: major + 1,
                        minor: 0,
                        bug: 0,
                        prerelease: vec![],
                        build: vec![],
                    }
                } else {
                    Semver {
                        major,
                        minor: 0,
                        bug: 0,
                        prerelease: vec![],
                        build: vec![],
                    }
                }
            } else {
                Err(err)?
            };

            Ok((exact_semver, true))
        }
    }
}
