use std::io;
use std::io::BufRead;
use std::fs::File;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Instant, Duration};

use glob::glob;
use postgres_db::{DbConnection, testing};
use serde_json:: Value;
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

fn main() {

    let _status = Command::new("./grab_bench_many_changes.sh")
        .status()
        .expect("failed to execute process");

    let args: Vec<String> = std::env::args().collect();
    let filter_arg = match args[1].as_str() {
        "--bench" => None,
        x => Some(x)
    };

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