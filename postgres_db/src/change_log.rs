use super::connection::DbConnection;
use super::schema;
use super::schema::change_log;
use diesel::prelude::*;
use diesel::Queryable;

#[derive(Queryable)]
pub struct Change {
    pub seq: i64,
    pub raw_json: serde_json::Value,
}

pub fn query_latest_change_seq(conn: &mut DbConnection) -> Option<i64> {
    use diesel::dsl::*;
    use schema::change_log::dsl::*;

    conn.first(change_log.select(max(seq)))
        .expect("Error checking for max sequence in change_log table")
}

pub fn query_num_changes_after_seq(after_seq: i64, conn: &mut DbConnection) -> i64 {
    use diesel::dsl::*;
    use schema::change_log::dsl::*;

    conn.first(change_log.filter(seq.gt(after_seq)).select(count(seq)))
        .unwrap_or_else(|_| {
            panic!(
                "Error querying DB for number of changes after seq: {}",
                after_seq
            )
        })
}

pub fn query_changes_after_seq(
    after_seq: i64,
    limit_size: i64,
    conn: &mut DbConnection,
) -> Vec<Change> {
    use schema::change_log::dsl::*;

    conn.load(
        change_log
            .filter(seq.gt(after_seq))
            .limit(limit_size)
            .order(seq),
    )
    .unwrap_or_else(|_| {
        panic!(
            "Error querying DB for changes after seq: {} (limit size = {})",
            after_seq, limit_size
        )
    })
}

#[derive(Insertable, Debug)]
#[table_name = "change_log"]
struct NewChange<'a> {
    seq: i64,
    raw_json: &'a serde_json::Value,
}

pub fn insert_change(conn: &mut DbConnection, seq: i64, raw_json: serde_json::Value) {
    let unsanitized_json_str = serde_json::to_string(&raw_json).unwrap();
    let sanitized_json_str = sanitize_null_escapes(&unsanitized_json_str);
    let sanitized_value: serde_json::Value = serde_json::from_str(&sanitized_json_str)
        .unwrap_or_else(|_| {
            panic!(
                "Failed to parse sanitized JSON string: {}\n\n->\n\n{}",
                unsanitized_json_str, sanitized_json_str
            )
        });

    let new_change = NewChange {
        seq,
        raw_json: &sanitized_value,
    };

    conn.execute(diesel::insert_into(change_log::table).values(&new_change))
        .unwrap_or_else(|_| {
            panic!(
                "Error saving new row: {:?}.\n\nunsanitized:\n{}\n\nsanitized:\n{}\n\nseq:\n{}",
                new_change, unsanitized_json_str, sanitized_json_str, seq
            )
        });
}

fn sanitize_null_escapes(s: &str) -> String {
    let mut sanitized = String::with_capacity(s.len());

    enum State {
        S0,
        S1,
        S2,
        S3,
        S4,
        S5,
    }
    use State::*;

    let mut state = S0;

    for c in s.chars() {
        state = match (state, c) {
            (S0, '\\') => S1,
            (S0, x) => {
                sanitized.push(x);
                S0
            }
            (S1, 'u') => S2,
            (S1, x) => {
                sanitized.push('\\');
                sanitized.push(x);
                S0
            }
            (S2, '0') => S3,
            (S2, x) => {
                sanitized.push_str("\\u");
                sanitized.push(x);
                S0
            }
            (S3, '0') => S4,
            (S3, x) => {
                sanitized.push_str("\\u0");
                sanitized.push(x);
                S0
            }
            (S4, '0') => S5,
            (S4, x) => {
                sanitized.push_str("\\u00");
                sanitized.push(x);
                S0
            }
            (S5, '0') => {
                sanitized.push_str("[NULL]");
                S0
            }
            (S5, x) => {
                sanitized.push_str("\\u000");
                sanitized.push(x);
                S0
            }
        };
    }

    sanitized
}

#[cfg(test)]
mod tests {
    use super::sanitize_null_escapes;

    #[test]
    fn sanitize_no_escapes() {
        assert_eq!(sanitize_null_escapes(r"cat"), "cat".to_string());
    }

    #[test]
    fn sanitize_other_escapes() {
        assert_eq!(sanitize_null_escapes(r"\u0001"), r"\u0001".to_string());
    }

    #[test]
    fn sanitize_1_bad_end() {
        assert_eq!(
            sanitize_null_escapes(r"bad:\u0000"),
            "bad:[NULL]".to_string()
        );
    }

    #[test]
    fn sanitize_1_bad_start() {
        assert_eq!(
            sanitize_null_escapes(r"\u0000:bad"),
            "[NULL]:bad".to_string()
        );
    }

    #[test]
    fn sanitize_2_bad() {
        assert_eq!(
            sanitize_null_escapes(r"\u0000\u0000"),
            "[NULL][NULL]".to_string()
        );
    }

    #[test]
    fn sanitize_4_bad() {
        assert_eq!(
            sanitize_null_escapes(r"\u0000\u0000\u0000\u0000"),
            "[NULL][NULL][NULL][NULL]".to_string()
        );
    }

    #[test]
    fn sanitize_2_bad_separated() {
        assert_eq!(
            sanitize_null_escapes(r"A\u0000A\u0000A"),
            "A[NULL]A[NULL]A".to_string()
        );
    }

    #[test]
    fn sanitize_4_bad_separated() {
        assert_eq!(
            sanitize_null_escapes(r"A\u0000A\u0000A\u0000A\u0000A"),
            "A[NULL]A[NULL]A[NULL]A[NULL]A".to_string()
        );
    }

    #[test]
    fn sanitize_bad_extra_backslack() {
        assert_eq!(
            sanitize_null_escapes(r"start \\u0000 end"),
            r"start \\u0000 end".to_string()
        );
    }
}
