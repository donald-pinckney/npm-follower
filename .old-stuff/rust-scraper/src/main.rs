mod sqlite_handler;
mod update_handler;
mod changes;
mod packument;
mod version;
mod utils;
mod inserter;
mod sql_data;
mod sql_insertable;

use crate::changes::Change;
use crate::update_handler::UpdateHandler;
use changes_stream2::{ChangesStream, Event};
use futures_util::stream::StreamExt;

#[tokio::main]
async fn main() {
    let mut update_handler = sqlite_handler::SqliteUpdateHandler::new();
    let since_seq: u64 = update_handler.get_seq();

    // let url = format!("https://replicate.npmjs.com/_changes?heartbeat=30000&feed=continuous&style=main_only&include_docs=true&since={}", since_seq);
    let url = format!("https://replicate.npmjs.com/_changes?heartbeat=30000&feed=continuous&style=main_only&include_docs=true&since=now");

    let mut changes = ChangesStream::new(url).await.unwrap();
    let mut acked_seq = since_seq;
    let mut update_count: u64 = 0;

    while let Some(event) = changes.next().await {
        match event {
            Ok(Event::Change(change_json)) => {
                let seq = change_json.seq.as_u64().unwrap();
                let raw_change = Change::from(change_json);
                let packument_change = raw_change.map(|doc| packument::process_packument_blob(doc).unwrap());
                
                update_handler.process_update(packument_change);

                acked_seq = seq;
                update_count += 1;
                if update_count % 100 == 0 {
                    update_count = 0;
                    update_handler.record_seq(acked_seq);
                }
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