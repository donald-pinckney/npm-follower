use super::*;
use diff_log_builder::process_changes;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub fn read_change_batches(s: &str) -> Vec<Vec<Value>> {
    serde_json::from_str(s).unwrap()
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct DiffTestState {
    pub hash_state: HashMap<String, InternalDiffLogStateRow>,
    pub diff_log: Vec<DiffLogEntry>,
}

pub fn run_change_batches(raw_batches: Vec<Vec<Value>>) -> DiffTestState {
    postgres_db::connection::testing::using_test_db(|conn| {
        let mut seq = 1;
        let change_batches: Vec<Vec<_>> = raw_batches
            .into_iter()
            .map(|b| {
                b.into_iter()
                    .map(|val| {
                        let this_seq = seq;
                        seq += 1;
                        // We just have to insert placeholder changes so that foreign key constraints are ok.
                        postgres_db::change_log::insert_change(conn, this_seq, Value::Null);
                        Change {
                            seq: this_seq,
                            raw_json: serde_json::from_value(val).unwrap(),
                        }
                    })
                    .collect()
            })
            .collect();

        for batch in change_batches {
            conn.run_psql_transaction(|mut trans_conn| {
                process_changes(&mut trans_conn, batch).unwrap();
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

        DiffTestState {
            hash_state: hash_state_map,
            diff_log,
        }
    })
}
