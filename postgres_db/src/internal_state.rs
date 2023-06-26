use crate::connection::QueryRunner;

use super::schema;
use diesel::prelude::*;

pub fn query_diff_log_processed_seq<R: QueryRunner>(conn: &mut R) -> Option<i64> {
    query_key_value_int_state("diff_log_processed_seq", conn)
}

pub fn set_diff_log_processed_seq<R: QueryRunner>(seq: i64, conn: &mut R) {
    set_key_value_int_state("diff_log_processed_seq", seq, conn);
}

pub fn query_relational_processed_seq<R: QueryRunner>(conn: &mut R) -> Option<i64> {
    query_key_value_int_state("relational_processed_seq", conn)
}

pub fn set_relational_processed_seq<R: QueryRunner>(seq: i64, conn: &mut R) {
    set_key_value_int_state("relational_processed_seq", seq, conn);
}

pub fn query_queued_downloads_seq<R: QueryRunner>(conn: &mut R) -> Option<i64> {
    query_key_value_int_state("queued_downloads_seq", conn)
}

pub fn set_queued_downloads_seq<R: QueryRunner>(seq: i64, conn: &mut R) {
    set_key_value_int_state("queued_downloads_seq", seq, conn);
}

pub fn query_tarball_transfer_last<R: QueryRunner>(conn: &mut R) -> Option<(String, i64)> {
    let last_url = query_key_value_string_state("tarball_transfer_last_url", conn)?;
    let last_seq = query_key_value_int_state("tarball_transfer_last_seq", conn)?;
    Some((last_url, last_seq))
}

pub fn set_tarball_transfer_last<R: QueryRunner>(url: String, seq: i64, conn: &mut R) {
    set_key_value_string_state("tarball_transfer_last_url", url, conn);
    set_key_value_int_state("tarball_transfer_last_seq", seq, conn);
}

fn query_key_value_int_state<R: QueryRunner>(the_key: &str, conn: &mut R) -> Option<i64> {
    use schema::internal_state::dsl::*;

    let nested: Option<Option<i64>> = conn
        .first(internal_state.filter(key.eq(the_key)).select(int_value))
        .optional()
        .expect("Error accessing table: internal_state");

    nested.flatten()
}

fn set_key_value_int_state<R: QueryRunner>(the_key: &str, the_value: i64, conn: &mut R) {
    use schema::internal_state::dsl::*;

    let new_pair = (key.eq(the_key), int_value.eq(the_value));
    conn.execute(
        diesel::insert_into(internal_state)
            .values(&new_pair)
            .on_conflict(key)
            .do_update()
            .set(new_pair),
    )
    .unwrap_or_else(|_| panic!("Failed to set key/value pair: {:?}", new_pair));
}

fn query_key_value_string_state<R: QueryRunner>(the_key: &str, conn: &mut R) -> Option<String> {
    use schema::internal_state::dsl::*;

    let nested: Option<Option<String>> = conn
        .first(internal_state.filter(key.eq(the_key)).select(string_value))
        .optional()
        .expect("Error accessing table: internal_state");

    nested.flatten()
}

fn set_key_value_string_state<R: QueryRunner>(the_key: &str, the_value: String, conn: &mut R) {
    use schema::internal_state::dsl::*;

    let new_pair = (key.eq(the_key), string_value.eq(the_value));
    conn.execute(
        diesel::insert_into(internal_state)
            .values(&new_pair)
            .on_conflict(key)
            .do_update()
            .set(new_pair.clone()),
    )
    .unwrap_or_else(|_| panic!("Failed to set key/value pair: {:?}", new_pair));
}
