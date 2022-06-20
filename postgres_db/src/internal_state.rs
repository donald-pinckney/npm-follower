use diesel::prelude::*;
use super::DbConnection;
use super::schema;


pub fn query_relational_processed_seq(conn: &DbConnection) -> Option<i64> {
    query_key_value_state("relational_processed_seq", conn)
}

pub fn set_relational_processed_seq(seq: i64, conn: &DbConnection) {
    set_key_value_state("relational_processed_seq", seq, conn);
}


pub fn query_queued_downloads_seq(conn: &DbConnection) -> Option<i64> {
    query_key_value_state("queued_downloads_seq", conn)
}

pub fn set_queued_downloads_seq(seq: i64, conn: &DbConnection) {
    set_key_value_state("queued_downloads_seq", seq, conn);
}

fn query_key_value_state(the_key: &str, conn: &DbConnection) -> Option<i64> {
    use schema::internal_state::dsl::*;
    
    internal_state
        .filter(key.eq(the_key))
        .select(value)
        .first(&conn.conn)
        .optional()
        .expect("Error accessing table: internal_state")
}


fn set_key_value_state(the_key: &str, the_value: i64, conn: &DbConnection) {
    use schema::internal_state::dsl::*;

    let new_pair = (key.eq(the_key), value.eq(the_value));
    diesel::insert_into(internal_state)
        .values(&new_pair)
        .on_conflict(key)
        .do_update()
        .set(new_pair)
        .execute(&conn.conn)
        .expect(&format!("Failed to set key/value pair: {:?}", new_pair));
}

