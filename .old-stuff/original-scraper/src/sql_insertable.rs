use rusqlite::ToSql;

pub trait SqlInsertable {
  const INSERT_TEMPLATE: &'static str;
  fn params(&self) -> Vec<&dyn ToSql>;
}
