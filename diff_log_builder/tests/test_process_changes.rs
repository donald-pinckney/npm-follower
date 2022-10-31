use std::collections::HashMap;

use postgres_db::{
    change_log::Change,
    diff_log::{internal_diff_log_state::sql::InternalDiffLogStateRow, DiffLogEntry},
};

#[test]
fn test_process_changes_3_versions_1_batch() {
    run_change_batches(vec![vec![r#""#]]);
    assert_eq!(6, 5);
}

fn run_change_batches(
    str_batches: Vec<Vec<&str>>,
) -> (HashMap<String, InternalDiffLogStateRow>, Vec<DiffLogEntry>) {
    use diff_log_builder::process_changes;

    let mut seq = 1;
    let change_batches: Vec<Vec<_>> = str_batches
        .into_iter()
        .map(|b| {
            b.into_iter()
                .map(|s| {
                    let this_seq = seq;
                    seq += 1;
                    Change {
                        seq: this_seq,
                        raw_json: serde_json::from_str(s).unwrap(),
                    }
                })
                .collect()
        })
        .collect();

    postgres_db::connection::testing::using_test_db(|conn| {
        for batch in change_batches {
            conn.run_psql_transaction(|mut trans_conn| {
                process_changes(&mut trans_conn, batch);
                Ok(())
            })
            .unwrap();
        }

        let hash_state =
            postgres_db::diff_log::internal_diff_log_state::sql::testing::get_all_packages(conn);

        let hash_state_map: HashMap<_, _> = hash_state
            .into_iter()
            .map(|r| (r.package_name.clone(), r))
            .collect();

        let diff_log = postgres_db::diff_log::testing::get_all_diff_logs(conn);

        (hash_state_map, diff_log)
    })
}
