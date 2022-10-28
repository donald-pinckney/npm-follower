use diesel::sql_query;

use crate::connection::DbConnection;
use crate::custom_types::{PrereleaseTag, Semver};

pub use crate::connection::testing::using_test_db;

impl Semver {
    pub fn new_testing_semver(n: i64) -> Semver {
        if n % 2 == 0 {
            Semver {
                major: n,
                minor: n + 1,
                bug: n,
                prerelease: vec![PrereleaseTag::String("alpha".into()), PrereleaseTag::Int(n)],
                build: vec!["stuff".into()],
            }
        } else {
            Semver {
                major: n + 2,
                minor: n + 1,
                bug: n,
                prerelease: vec![],
                build: vec![],
            }
        }
    }
}

// Used to create and automatically drop temporary tables, used for tests.
pub fn using_temp_table<F>(
    conn: &mut DbConnection,
    table_name: &'static str,
    columns: &'static str,
    f: F,
) where
    F: FnOnce(&mut DbConnection),
{
    conn.batch_execute(&format!(
        "DROP TABLE IF EXISTS {}; CREATE TABLE {} ({})",
        table_name, table_name, columns
    ))
    .unwrap();

    f(conn);

    let drop_query = sql_query(&format!("DROP TABLE {}", table_name));
    conn.execute(drop_query).unwrap();
}
