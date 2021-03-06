use changes_stream2::ChangeEvent;
use std::path::Path;
use std::fs::File;
use changes_stream2::{ChangesStream, Event};
use futures_util::stream::StreamExt;
use postgres_db::{change_log, DbConnection};
use utils::check_no_concurrent_processes;
use async_std::task;
use std::time::Duration;

#[tokio::main]
async fn main() {
    check_no_concurrent_processes("changes_fetcher");

    let conn = postgres_db::connect();

    loop {
        listen_for_npm_changes_forever(&conn).await;
        println!("NPM changes streamer ended. Sleeping for 300 seconds before restarting...");
        task::sleep(Duration::from_secs(300)).await;
    }
}



async fn listen_for_npm_changes_forever(conn: &DbConnection) {
    let since_when = change_log::query_latest_change_seq(conn);

    let db_url = "https://replicate.npmjs.com";

    let db_resp: serde_json::Value = 
        reqwest::get(db_url).await.unwrap()
        .json::<serde_json::Value>().await.unwrap();
    
    let end_sequence = db_resp
        .as_object().unwrap()
        .get("update_seq").unwrap()
        .as_u64().unwrap();

    println!("Current last seq on NPM is: {}", end_sequence);
    println!("Starting replication for range: ({}, forever)", since_when.map(|s| s.to_string()).unwrap_or("start-of-time".to_owned()));


    let changes_url = match since_when {
        Some(since_when_num) => format!("https://replicate.npmjs.com/_changes?feed=continuous&style=main_only&include_docs=true&since={}", since_when_num),
        None => "https://replicate.npmjs.com/_changes?feed=continuous&style=main_only&include_docs=true".to_string(),
    };

    let mut changes = ChangesStream::new(changes_url).await.unwrap();
    while let Some(event) = changes.next().await {
        match event {
            Ok(Event::Change(change_json)) => {
                println!("inserting change seq: {}", change_json.seq);
                process_change_event(conn, change_json);               
            }
            Ok(Event::Finished(finished)) => {
                println!("Finished: {}", finished.last_seq);
                break
            },
            Err(err) => {
                println!("Error: {:?}", err);
                break
            }
        }
    }
}


pub fn process_change_event(conn: &DbConnection, change: ChangeEvent) {
    let seq = change.seq.as_i64().unwrap();
    let change_json = serde_json::to_value(&change)
        .expect("Failed to serialize ChangeEvent to a Value");

    change_log::insert_change(conn, seq, change_json);
}




fn _insert_saved_log_file(conn: &DbConnection) {
    use std::io::BufRead;
    use indicatif::{ProgressBar, ProgressIterator, ProgressStyle};

    // Iterate over the lines in a file
    let log_path = Path::new("testing/log.jsonl");
    let log_file = File::open(log_path).unwrap();
    let log_reader = std::io::BufReader::new(log_file);

    let bar_config = ProgressBar::new(2446804);
    bar_config.set_style(
        ProgressStyle::default_bar()
        .template("[{elapsed_precise}] {bar:100} {percent}% [{pos:>7}/{len:7}] [{per_sec}] [{eta_precise}]")
    );

    for line in log_reader.lines().progress_with(bar_config) {
        let line = line.unwrap();
        let change_json = serde_json::from_str::<changes_stream2::ChangeEvent>(&line).unwrap();
        process_change_event(conn, change_json);
    }
}