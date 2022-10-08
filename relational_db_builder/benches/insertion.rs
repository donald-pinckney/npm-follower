use std::collections::HashMap;
use std::io;
use std::io::BufRead;
use std::fs::File;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

use chrono::Utc;
use glob::glob;
use postgres_db::custom_types::{Semver, ParsedSpec, VersionConstraint, VersionComparator};
use postgres_db::testing;
use relational_db_builder::packument::{Packument, VersionPackument, Spec, Dist};
use serde_json::{ Value, Map};
use colored::Colorize;

use postgres_db::change_log::Change;


fn load_change_dataset(path: &PathBuf) -> Vec<Change> {
    io::BufReader::new(File::open(path).unwrap()).lines().map(|l| {
        let l = l.unwrap();
        let raw_json: Value = serde_json::from_str(&l).unwrap();
        let seq = raw_json["seq"].as_i64().unwrap();
        Change { seq, raw_json }
    }).collect()
}


fn run_bench(path: PathBuf) {
    testing::using_test_db(|conn| {
        println!("\n====>> *** Running insertion bench: {} *** <<====\n", path.file_name().unwrap().to_string_lossy().bold());

        let now = Instant::now();
        let changes = load_change_dataset(&path);
        let load_dt = now.elapsed();
        println!("Loaded {} ({:.2} seconds)", path.display(), load_dt.as_secs_f64());

        let now = Instant::now();
        let changes: Vec<_> = changes.into_iter().filter_map(relational_db_builder::deserialize_change).collect();
        let parse_dt = now.elapsed();
        println!("Parsed {} ({:.2} seconds)", path.display(), parse_dt.as_secs_f64());

        let now = Instant::now();
        for (name, pack) in changes {
            relational_db_builder::apply_packument_change(conn, name, pack)
        }
        let insert_dt = now.elapsed();
        println!("Inserted {} {}", path.display(), format!("({:.2} seconds)", insert_dt.as_secs_f64()).bold());
    });
}

fn build_synthetic_bench_insert_same_package(n: i32) -> Vec<(String, Packument)> {
    use chrono::TimeZone;

    let package_name = "react";
    let mut packs = vec![];


    let start_time = Utc.ymd(1990, 1, 1).and_hms(1, 0, 0);
    for x in 0..n {
        let num_versions = x + 1;
        let versions_this_tick: Vec<_> = (0..num_versions).map(|vi| {
            let v = Semver { major: (vi + 1) as i64, minor: 0, bug: 0, prerelease: vec![], build: vec![] };
            let vt = start_time + chrono::Duration::seconds(vi as i64);

            let vp = VersionPackument {
                prod_dependencies: vec![("lodash".to_owned(), Spec { raw: Value::String("1.2.3".to_owned()), parsed: ParsedSpec::Range(VersionConstraint(vec![vec![VersionComparator::Eq(Semver { major: 1, minor: 2, bug: 3, prerelease: vec![], build: vec![] })]])) })],
                dev_dependencies: vec![],
                peer_dependencies: vec![],
                optional_dependencies: vec![],
                dist: Dist { tarball_url: format!("https://registry.npmjs.org/{}/-/{}-{}.tgz", package_name, package_name, v), shasum: None, unpacked_size: None, file_count: None, integrity: None, signature0_sig: None, signature0_keyid: None, npm_signature: None },
                repository: None,
                extra_metadata: HashMap::new(),
            };

            (v, vt, vp)
        }).collect();

        let vers_times: HashMap<_, _> = versions_this_tick.iter().map(|(v, vt, _)| (v.clone(), vt.clone())).collect();
        let vers_packs: HashMap<_, _> = versions_this_tick.iter().map(|(v, _, vp)| (v.clone(), vp.clone())).collect();

        let this_version = versions_this_tick.last().unwrap();
        let p = Packument::Normal { latest: Some(this_version.0.clone()), created: start_time, modified: this_version.1.clone(), other_dist_tags: Map::new(), version_times: vers_times, versions: vers_packs };
        packs.push((package_name.to_owned(), p));

    }
    packs
}




fn build_synthetic_bench_insert_different_packages(n: i32) -> Vec<(String, Packument)> {
    use chrono::TimeZone;

    let package_name_base = "react";
    let mut packs = vec![];


    let start_time = Utc.ymd(1990, 1, 1).and_hms(1, 0, 0);
    for x in 0..n {
        let package_name = format!("{}{}", package_name_base, x);
        let v = Semver { major: 1 as i64, minor: 0, bug: 0, prerelease: vec![], build: vec![] };
        let vt = start_time + chrono::Duration::seconds(x as i64);

        let vp_this_tick = VersionPackument {
            prod_dependencies: vec![("lodash".to_owned(), Spec { raw: Value::String("1.2.3".to_owned()), parsed: ParsedSpec::Range(VersionConstraint(vec![vec![VersionComparator::Eq(Semver { major: 1, minor: 2, bug: 3, prerelease: vec![], build: vec![] })]])) })],
            dev_dependencies: vec![],
            peer_dependencies: vec![],
            optional_dependencies: vec![],
            dist: Dist { tarball_url: format!("https://registry.npmjs.org/{}/-/{}-{}.tgz", package_name, package_name, v), shasum: None, unpacked_size: None, file_count: None, integrity: None, signature0_sig: None, signature0_keyid: None, npm_signature: None },
            repository: None,
            extra_metadata: HashMap::new(),
        };

        let mut vers_times: HashMap<_, _> = HashMap::new();
        vers_times.insert(v.clone(), vt);

        let mut vers_packs: HashMap<_, _> = HashMap::new();
        vers_packs.insert(v.clone(), vp_this_tick);

        let p = Packument::Normal { latest: Some(v.clone()), created: vt, modified: vt, other_dist_tags: Map::new(), version_times: vers_times, versions: vers_packs };
        packs.push((package_name.to_owned(), p));
    }
    packs
}



fn run_synthetic_bench<F>(n: i32, name: &str, build_fn: F) where F: FnOnce(i32) -> Vec<(String, Packument)> {
    testing::using_test_db(|conn| {
        println!("\n====>> *** Running synthetic insertion bench: {} ({}) *** <<====\n", name.bold(), n);

        let now = Instant::now();
        let changes: Vec<_> = build_fn(n);
        let parse_dt = now.elapsed();
        println!("Loaded {} ({:.2} seconds)", name, parse_dt.as_secs_f64());

        let now = Instant::now();
        for (name, pack) in changes {
            relational_db_builder::apply_packument_change(conn, name, pack)
        }
        let insert_dt = now.elapsed();
        println!("Inserted {} {}", name, format!("({:.2} seconds)", insert_dt.as_secs_f64()).bold());
    });
}


fn main() {
    let _status = Command::new("./grab_bench_many_changes.sh")
        .status()
        .expect("failed to execute process");

    let args: Vec<String> = std::env::args().collect();
    let filter_arg = match args[1].as_str() {
        "--bench" => None,
        x => Some(x)
    };

    let synthetic_benches: Vec<(&str, fn(i32) -> Vec<(String, Packument)>)> = vec![
        ("synth_insert_same_package", build_synthetic_bench_insert_same_package),
        ("synth_insert_different_packages", build_synthetic_bench_insert_different_packages)
    ];
    
    let synth_match_maybe = synthetic_benches.iter().filter_map(|(name, f)| {
        if filter_arg == Some(name) {
            Some((name, f))
        } else {
            None
        }
    }).last();

    if let Some((syn_name, syn_build)) = synth_match_maybe {
        dbg!(&args);
        let n = args[2].parse().unwrap();
        run_synthetic_bench(n, syn_name, syn_build);
        return
    }

    
    let mut benches = vec![];
    

    for e in glob("resources/bench_many_changes/*.jsonl").expect("Failed to read glob pattern") {
        let data_path = e.unwrap();
        let should_run = filter_arg.map(|f| data_path.file_name().unwrap().to_string_lossy().contains(f)).unwrap_or(true);
        benches.push((data_path, should_run));
    }


    for (b, run) in benches {
        if run {
            run_bench(b);
        } else {
            // println!("Skipping insertion bench: {}", b.file_name().unwrap().to_string_lossy());
        }
    }
}