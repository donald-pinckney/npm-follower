use postgres_db::change_log::Change;
use serde_json::Value;
use relational_db_builder::deserialize_change;
use test_case::test_case;


const SEQ_832853_CLEANED: &'static str = include_str!("../test_data/seq_832853_cleaned.json");
const SEQ_1166950: &'static str = include_str!("../test_data/seq_1166950.json");

#[test_case(SEQ_832853_CLEANED)]
#[test_case(SEQ_1166950)]
fn test_deserialize(seq_json_str: &str) {
    let v: Value = serde_json::from_str(seq_json_str).unwrap();
    let change = Change { seq: 1, raw_json: v };

    deserialize_change(change).unwrap();
}