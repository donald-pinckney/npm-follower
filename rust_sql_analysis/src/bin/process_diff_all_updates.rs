use std::collections::HashMap;

use diesel::QueryableByName;
use postgres_db::{
    connection::{DbConnection, QueryRunner},
    custom_types::{ParsedSpec, Semver},
    versions::Version,
};
use serde::{Deserialize, Serialize};

fn print_usage_exit(argv0: &str) -> ! {
    eprintln!("Usage: {} chunk_size", argv0);
    std::process::exit(1);
}

fn main() {
    utils::check_no_concurrent_processes("process_diff_all_updates");
    dotenvy::dotenv().ok();
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() < 2 {
        print_usage_exit(args[0].as_str());
    }
    let chunk_size = args[1].parse::<i64>().unwrap();
    let conn: DbConnection = DbConnection::connect();
    process_diff_all_updates(conn, chunk_size);
}

//  package_id | from_group_base_semver | to_group_base_semver | from_id |  to_id  | from_semver | to_semver |        from_created        |         to_created         |  ty

#[derive(Serialize, Deserialize, QueryableByName, Debug, Clone)]
struct Update {
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    package_id: i64,
    #[diesel(sql_type = postgres_db::schema::sql_types::SemverStruct)]
    from_group_base_semver: Semver,
    #[diesel(sql_type = postgres_db::schema::sql_types::SemverStruct)]
    to_group_base_semver: Semver,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    from_id: i64,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    to_id: i64,
    #[diesel(sql_type = postgres_db::schema::sql_types::SemverStruct)]
    from_semver: Semver,
    #[diesel(sql_type = postgres_db::schema::sql_types::SemverStruct)]
    to_semver: Semver,
    #[diesel(sql_type = diesel::sql_types::Timestamp)]
    from_created: chrono::NaiveDateTime,
    #[diesel(sql_type = diesel::sql_types::Timestamp)]
    to_created: chrono::NaiveDateTime,
    #[diesel(sql_type = diesel::sql_types::Text)]
    ty: String,
}

fn process_diff_all_updates(mut conn: DbConnection, chunk_size: i64) {
    let mut last = None;
    let mut num_processed = 0;

    loop {
        println!("Loading {} rows from the table...", chunk_size);
        let time = std::time::Instant::now();
        let query = diesel::sql_query(format!(
            "
            SELECT
                package_id,
                from_group_base_semver,
                to_group_base_semver,
                from_id,
                to_id,
                from_semver,
                to_semver,
                from_created,
                to_created,
                ty
            FROM analysis.all_updates{}
            ORDER BY from_id, to_id
            LIMIT {}",
            {
                if let Some((from_id, to_id)) = last {
                    format!(
                        "
                WHERE (from_id, to_id) > ({}, {})",
                        from_id, to_id
                    )
                } else {
                    "".to_string()
                }
            },
            chunk_size,
        ));

        let table: Vec<Update> = conn
            .get_results(query)
            .expect("Failed to load the table from the database");
        let table_len = table.len();
        println!("Loaded {} rows in {:?}!", table_len, time.elapsed());
        num_processed += table_len;
        if table.is_empty() {
            break;
        }
        last = table.last().map(|d| (d.from_id, d.to_id));

        println!("Progress: {} rows", num_processed);
        println!("Writing {} rows to the file...", table.len());
        let time = std::time::Instant::now();
        let len_table = table.len();

        // insert here queries to write
        dep_update_changes(&mut conn, table).unwrap();

        println!("Wrote {} rows in {:?}!", len_table, time.elapsed());
    }
}

fn dep_update_changes(
    conn: &mut DbConnection,
    table: Vec<Update>,
) -> Result<(), diesel::result::Error> {
    struct UpdateDep {
        from_id: i64,
        to_id: i64,
        did_add_dep: bool,
        did_remove_dep: bool,
        did_change_dep_constraint: bool,
    }
    let mut rows: Vec<UpdateDep> = Vec::new();
    for update in table {
        let from_ver = postgres_db::versions::get_version_by_id(conn, update.from_id);
        let to_ver = postgres_db::versions::get_version_by_id(conn, update.to_id);
        let mut from_deps = HashMap::new();
        let mut to_deps = HashMap::new();
        let mut fn_add_deps = |deps: &mut HashMap<String, ParsedSpec>, ver: &Version| {
            for dep_id in ver
                .prod_dependencies
                .iter()
                .chain(ver.dev_dependencies.iter())
                .chain(ver.peer_dependencies.iter())
                .chain(ver.optional_dependencies.iter())
            {
                let dep = postgres_db::dependencies::get_dependency_by_id(conn, *dep_id);
                deps.insert(dep.dst_package_name, dep.spec);
            }
        };
        fn_add_deps(&mut from_deps, &from_ver);
        fn_add_deps(&mut to_deps, &to_ver);

        let mut did_add_dep = false;
        let mut did_remove_dep = false;
        let mut did_change_dep_constraint = false;
        for (dep_name, from_spec) in from_deps {
            if let Some(to_spec) = to_deps.remove(&dep_name) {
                if from_spec != to_spec {
                    did_change_dep_constraint = true;
                }
            } else {
                did_remove_dep = true;
            }
        }
        if !to_deps.is_empty() {
            did_add_dep = true;
        }

        rows.push(UpdateDep {
            from_id: update.from_id,
            to_id: update.to_id,
            did_add_dep,
            did_remove_dep,
            did_change_dep_constraint,
        });
    }

    // insert into analysis.update_dep_changes
    let query = r#"
        INSERT INTO analysis.update_dep_changes (from_id, to_id, did_add_dep, did_remove_dep, did_change_dep_constraint)
        VALUES
    "#;
    let mut query = query.to_string();
    for (i, nf) in rows.iter().enumerate() {
        if i > 0 {
            query.push_str(", ");
        }
        query.push_str(&format!(
            "({}, {}, {}, {}, {})",
            nf.from_id, nf.to_id, nf.did_add_dep, nf.did_remove_dep, nf.did_change_dep_constraint
        ));
    }
    query.push_str(" ON CONFLICT (from_id, to_id) DO NOTHING");
    println!("Inserting {} rows into the table...", rows.len());
    let diesel_query = diesel::sql_query(query);
    conn.execute(diesel_query)?;
    Ok(())
}
