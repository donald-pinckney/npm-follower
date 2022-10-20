use std::time::Duration;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use postgres_db::change_log::Change;
use serde_json::Value;
use diff_log_builder::deserialize_change;


const BIG_CHANGE_JSON_STR: &str = include_str!("../resources/test_changes/input/seq_832853_cleaned.json");
const SMALL_CHANGE_JSON_STR: &str = include_str!("../resources/test_changes/input/seq_1166950.json");


pub fn bench_parse_big_change(c: &mut Criterion) {
    let v: Value = serde_json::from_str(BIG_CHANGE_JSON_STR).unwrap();
    let change = Change { seq: 1, raw_json: v };

    c.bench_function("parse test_change_big.json", |b| b.iter(|| deserialize_change(black_box(clone_change(&change)))));
}

pub fn bench_parse_small_change(c: &mut Criterion) {
    let v: Value = serde_json::from_str(SMALL_CHANGE_JSON_STR).unwrap();
    let change = Change { seq: 1, raw_json: v };

    c.bench_function("parse test_change_small.json", |b| b.iter(|| deserialize_change(black_box(clone_change(&change)))));
}

fn clone_change(c: &Change) -> Change {
    Change { seq: c.seq, raw_json: c.raw_json.clone() }
}

criterion_group!{
    name = benches;
    config = Criterion::default().sample_size(10).measurement_time(Duration::from_secs(60));
    targets = bench_parse_big_change, bench_parse_small_change
}

criterion_main!(benches);