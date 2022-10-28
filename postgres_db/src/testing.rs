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
pub struct TempTable<'a> {
    pub connection: &'a mut DbConnection,
    pub table_name: &'static str,
}

impl<'a> TempTable<'a> {
    pub fn new(
        connection: &'a mut DbConnection,
        table_name: &'static str,
        columns: &'static str,
    ) -> Self {
        connection
            .batch_execute(&format!(
                "DROP TABLE IF EXISTS {}; CREATE TABLE {} ({})",
                table_name, table_name, columns
            ))
            .unwrap();
        TempTable {
            connection,
            table_name,
        }
    }
}

impl<'a> Drop for TempTable<'a> {
    fn drop(&mut self) {
        let drop_query = sql_query(&format!("DROP TABLE {}", self.table_name));
        self.connection.execute(drop_query).unwrap();
    }
}
