use crate::packument::Packument;
use serde_json::Value;
use serde_json::Map;
use crate::changes::Change;
use crate::update_handler::UpdateHandler;

pub struct SqliteUpdateHandler {}

impl SqliteUpdateHandler {
  pub fn new() -> Self {
    SqliteUpdateHandler {}
  }
}

impl UpdateHandler for SqliteUpdateHandler {
  fn process_update(&mut self, change: Change<Packument>) {
    match change {
      Change::Delete { package } => {
        println!("Delete: {}", package);
      }
      Change::Update { package, data: doc } => {
        println!("Change {}: {:#?}", package, doc);
      }
    }
  }
  fn record_seq(&mut self, acked_seq: u64) {
    println!("Saving acked seq: {}", acked_seq);
  }
  fn get_seq(&self) -> u64 {
    return 0;
  }
}
