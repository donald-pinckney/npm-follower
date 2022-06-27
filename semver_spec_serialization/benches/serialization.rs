use std::time::Duration;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use semver_spec_serialization::parse_semver;
use semver_spec_serialization::parse_spec;

pub fn bench_parse_semver(c: &mut Criterion) {
    c.bench_function("parse semver 1.2.3-alpha.1+build.56", |b| b.iter(|| parse_semver(black_box("1.2.3-alpha.1+build.56"))));
}

pub fn bench_parse_spec(c: &mut Criterion) {
    c.bench_function("parse spec ^1.2.3", |b| b.iter(|| parse_spec(black_box("^1.2.3"), false)));
}

// criterion_group!(benches, bench_parse_semver, bench_parse_spec);

criterion_group!{
    name = benches;
    config = Criterion::default().measurement_time(Duration::from_secs(20));
    targets = bench_parse_semver, bench_parse_spec
}

criterion_main!(benches);