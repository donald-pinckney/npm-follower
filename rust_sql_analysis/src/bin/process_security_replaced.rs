use diesel::prelude::*;
use rust_sql_analysis::process_diff_analysis;
use serde::{Deserialize, Serialize};

use postgres_db::{
    connection::{DbConnection, QueryRunner},
    custom_types::{Semver, VersionStateType},
    diff_analysis::{DiffAnalysis, DiffAnalysisJobResult},
    schema::sql_types::SemverStruct,
    versions::Version,
};

#[derive(Serialize, Deserialize, QueryableByName, Debug, Clone)]
struct SecurityReplaced {
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    id: i64,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    package_id: i64,
    #[diesel(sql_type = postgres_db::schema::sql_types::SemverStruct)]
    semver: Semver,
}

// queries all security_replaced rows. it's ok to query all, because the number of rows is
// "small".
const QUERY: &str = r#"
SELECT id, package_id, semver from security_replaced_versions
"#;

fn main() {
    utils::check_no_concurrent_processes("process_security_replaced");
    dotenvy::dotenv().ok();
    let mut conn: DbConnection = DbConnection::connect();
    let q = diesel::sql_query(QUERY);
    let mut res: Vec<SecurityReplaced> = conn.load(q).unwrap();
    // reverse the order, so that we can process the most recent ones first
    res.reverse();

    println!("Total number of 0.0.1-security packages: {}", res.len());

    for sec in res {
        let version: Version = postgres_db::versions::get_version_by_id(&mut conn, sec.id);
        // check if it has been published by npm, or if it's a fake version
        let npm_user = match version
            .extra_metadata
            .get("_npmUser")
            .and_then(|u| u.get("email"))
            .and_then(|u| u.as_str())
        {
            Some(u) => u,
            None => {
                println!("!! No npm user for version {}", version.id);
                continue;
            }
        };
        let has_npm_domain = npm_user
            .split('@')
            .collect::<Vec<&str>>()
            .last()
            .map(|s| s.trim())
            .map(|s| s == "npmjs.com" || s == "microsoft.com");
        if has_npm_domain != Some(true) {
            println!(
                "!! Version {} is not published by npm, but {}.",
                version.id, npm_user
            );
            continue;
        }

        // now, let's check if there are any versions with the same package_id, but not same
        // version id
        let mut versions: Vec<Version> =
            postgres_db::versions::get_versions_by_package_id(&mut conn, version.package_id);
        // remove the current version
        versions.retain(|v| v.id != version.id);
        if versions.is_empty() {
            continue;
        }

        for ver in versions {
            let name = ver
                .extra_metadata
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("NONE");
            let state = &ver.current_version_state_type;
            // check if the version is deleted/unpublished
            match state {
                VersionStateType::Normal => continue,
                _ => {}
            };

            // check if we have downloaded the package
            if postgres_db::download_tarball::get_tarball_by_url(&mut conn, &ver.tarball_url)
                .is_none()
            {
                println!("!! Tarball {} is not downloaded", ver.tarball_url);
                continue;
            }

            println!("Semver: {}", ver.semver);
            println!("Name: {}", name);
            // insert into possibly_malware_versions
            let query = diesel::sql_query(
                format!(
                    "INSERT INTO possibly_malware_versions (package_id, id, tarball_url) VALUES ({}, {}, '{}') ON CONFLICT DO NOTHING",
                    ver.package_id, ver.id, ver.tarball_url
                )
            );
            conn.execute(query).unwrap();
        }
    }
}
