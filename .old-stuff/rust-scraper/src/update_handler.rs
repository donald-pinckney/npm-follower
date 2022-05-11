use crate::packument::Packument;
use serde_json::Map;
use serde_json::Value;
use crate::changes::Change;

pub trait UpdateHandler {
    fn process_update(&mut self, apply_change: Change<Packument>);

    fn record_seq(&mut self, seq: u64);
    fn get_seq(&self) -> u64;
}