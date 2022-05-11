use changes_stream2::ChangeEvent;
use serde_json::Map;
use serde_json::Value;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum Change<T> {
  Delete { seq: u64, package: String },
  Update { seq: u64, package: String, data: T },
}

impl From<ChangeEvent> for Change<Map<String, Value>> {
  fn from(mut e: ChangeEvent) -> Self {
    let seq = e.seq.as_u64().unwrap();

    if e.changes.len() != 1 {
      panic!("Expected exactly 1 change. Got: {:?}", e);
    }

    let mut doc = e.doc.take().unwrap();
    let package = e.id.clone();

    let doc_id = doc.remove("_id").unwrap().as_str().unwrap().to_string();
    let doc_rev = doc.remove("_rev").unwrap().as_str().unwrap().to_string();

    if doc_id != package {
      panic!(
        "Expected doc id to be package name. Got: {} vs. {}",
        doc_id, package
      );
    }

    if doc_rev != e.changes[0].rev {
      panic!(
        "Expected doc rev to be change rev. Got: {} vs. {}",
        doc_rev, e.changes[0].rev
      );
    }

    let doc_deleted = doc
      .remove("_deleted")
      .map(|deleted| deleted.as_bool().unwrap())
      .unwrap_or(false);
    if e.deleted != doc_deleted {
      panic!(
        "Expected doc._deleted == e.deleted. Got event: {:?}, doc: {:?}",
        e, doc
      );
    }

    if e.deleted {
      Change::Delete { seq, package }
    } else {
      Change::Update { seq, package, data: doc }
    }
  }
}

impl<T> Change<T> {
  pub fn package(&self) -> &str {
    match self {
      Change::Delete { seq: _seq, package } => package,
      Change::Update { package, .. } => package,
    }
  }

  pub fn seq(&self) -> u64 {
    match self {
      Change::Delete { seq, .. } => *seq,
      Change::Update { seq, .. } => *seq
    }
  }

  pub fn map<R, F>(self, f: F) -> Change<R> where F: Fn(T) -> R {
    match self {
      Change::Delete { seq, package } => Change::Delete { seq, package },
      Change::Update { seq, package, data } => Change::Update { seq, package, data: f(data) },
    }
  }
}
