#[macro_use]
extern crate diesel;

pub mod change_log;
// #[allow(clippy::let_unit_value)] // for redis
pub mod dependencies;
pub mod diff_log;
pub mod download_queue;
pub mod download_tarball;
pub mod internal_state;
pub mod packages;
pub mod packument;
#[allow(unused_imports)]
mod schema;
pub mod versions;

pub mod connection;
pub mod custom_types;

pub mod serde_non_string_key_serialization;

#[cfg(test)]
pub mod testing;
