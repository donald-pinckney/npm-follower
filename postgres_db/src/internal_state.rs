use super::connection::DbConnection;
use super::schema;
use diesel::prelude::*;

pub fn query_diff_log_processed_seq(conn: &mut DbConnection) -> Option<i64> {
    query_key_value_state("diff_log_processed_seq", conn)
}

pub fn set_diff_log_processed_seq(seq: i64, conn: &mut DbConnection) {
    set_key_value_state("diff_log_processed_seq", seq, conn);
}

pub fn query_relational_processed_seq(conn: &mut DbConnection) -> Option<i64> {
    query_key_value_state("relational_processed_seq", conn)
}

pub fn set_relational_processed_seq(seq: i64, conn: &mut DbConnection) {
    set_key_value_state("relational_processed_seq", seq, conn);
}

pub fn query_queued_downloads_seq(conn: &mut DbConnection) -> Option<i64> {
    query_key_value_state("queued_downloads_seq", conn)
}

pub fn set_queued_downloads_seq(seq: i64, conn: &mut DbConnection) {
    set_key_value_state("queued_downloads_seq", seq, conn);
}

fn query_key_value_state(the_key: &str, conn: &mut DbConnection) -> Option<i64> {
    use schema::internal_state::dsl::*;

    conn.first(internal_state.filter(key.eq(the_key)).select(value))
        .optional()
        .expect("Error accessing table: internal_state")
}

fn set_key_value_state(the_key: &str, the_value: i64, conn: &mut DbConnection) {
    use schema::internal_state::dsl::*;

    let new_pair = (key.eq(the_key), value.eq(the_value));
    conn.execute(
        diesel::insert_into(internal_state)
            .values(&new_pair)
            .on_conflict(key)
            .do_update()
            .set(new_pair),
    )
    .unwrap_or_else(|_| panic!("Failed to set key/value pair: {:?}", new_pair));
}
