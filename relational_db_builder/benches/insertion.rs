use std::io;
use std::io::BufRead;
use std::fs::File;
use criterion::black_box;

use serde_json:: Value;

use postgres_db::change_log::Change;


fn insertion_benchmark() {
    
    for l in io::BufReader::new(File::open("../change_log_benchmark.jsonl").unwrap()).lines() {
        let l = l.unwrap();
        let raw_json: Value = serde_json::from_str(&l).unwrap();
        let seq = raw_json["seq"].as_i64().unwrap();
        let c = Change { seq, raw_json };
        // println!("{:?}", c.raw_json);
    }
    println!("done");
}

fn main() {
    use std::time::Instant;
    let now = Instant::now();

    // Code block to measure.
    {
        black_box(insertion_benchmark)();
    }

    let elapsed = now.elapsed();
    println!("Elapsed: {:.2?}", elapsed);
}