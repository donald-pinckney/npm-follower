use crate::custom_types::Semver;

use crate::connection::QueryRunner;
use crate::schema;
use crate::schema::sql_types::SemverStruct;
use diesel::deserialize;
use diesel::deserialize::FromSql;
use diesel::pg::Pg;
use diesel::pg::PgValue;
use diesel::prelude::*;
use diesel::serialize;
use diesel::serialize::ToSql;
use diesel::sql_types::Record;
use diesel::Insertable;
use diesel::QueryDsl;
use diesel::Queryable;
use schema::internal_diff_log_state;
use serde::Deserialize;
use serde::Serialize;

#[derive(
    Queryable, Insertable, AsChangeset, Identifiable, Debug, PartialEq, Eq, Serialize, Deserialize,
)]
#[cfg_attr(test, derive(Clone))]
#[diesel(table_name = internal_diff_log_state)]
#[diesel(primary_key(package_name))]
pub struct InternalDiffLogStateRow {
    pub package_name: String,
    pub package_only_packument_hash: String,
    pub versions: Vec<InternalDiffLogVersionStateElem>,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct InternalDiffLogVersionStateElem {
    pub v: Semver,
    pub pack_hash: String,
    pub deleted: bool,
}

impl<'a> ToSql<schema::sql_types::InternalDiffLogVersionState, Pg>
    for InternalDiffLogVersionStateElem
{
    fn to_sql(&self, out: &mut serialize::Output<Pg>) -> serialize::Result {
        let record: (&Semver, &String, bool) = (&self.v, &self.pack_hash, self.deleted);
        serialize::WriteTuple::<(
            SemverStruct,
            diesel::sql_types::Text,
            diesel::sql_types::Bool,
        )>::write_tuple(&record, out)
    }
}

impl<'a> FromSql<schema::sql_types::InternalDiffLogVersionState, Pg>
    for InternalDiffLogVersionStateElem
{
    fn from_sql(bytes: PgValue) -> deserialize::Result<Self> {
        let tup: (Semver, String, bool) = FromSql::<
            Record<(
                SemverStruct,
                diesel::sql_types::Text,
                diesel::sql_types::Bool,
            )>,
            Pg,
        >::from_sql(bytes)?;
        Ok(InternalDiffLogVersionStateElem {
            v: tup.0,
            pack_hash: tup.1,
            deleted: tup.2,
        })
    }
}

pub(crate) fn create_packages<R: QueryRunner>(rows: Vec<InternalDiffLogStateRow>, conn: &mut R) {
    use schema::internal_diff_log_state::dsl::*;

    // TODO[bug]: batch this
    conn.execute(diesel::insert_into(internal_diff_log_state).values(rows))
        .unwrap_or_else(|e| panic!("Error saving new rows: {}", e));
}

pub(crate) fn lookup_package<R: QueryRunner>(
    package_name_str: &String,
    conn: &mut R,
) -> Option<InternalDiffLogStateRow> {
    use schema::internal_diff_log_state::dsl::*;

    conn.get_result(internal_diff_log_state.filter(package_name.eq(package_name_str)))
        .optional()
        .unwrap_or_else(|e| panic!("Error fetching row: {}", e))
}

pub(crate) fn update_packages<R: QueryRunner>(rows: Vec<InternalDiffLogStateRow>, conn: &mut R) {
    for r in rows {
        conn.execute(diesel::update(&r).set(&r))
            .unwrap_or_else(|e| panic!("Error updating rows: {}", e));
    }
}

pub mod testing {
    // use super::schema;
    // use super::InternalDiffLogStateRow;
    // use super::QueryRunner;
    use super::*;

    pub fn get_all_packages<R: QueryRunner>(conn: &mut R) -> Vec<InternalDiffLogStateRow> {
        use schema::internal_diff_log_state::dsl::*;

        conn.load(internal_diff_log_state.order(package_name))
            .unwrap_or_else(|e| panic!("Error fetching row: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use crate::custom_types::PrereleaseTag;
    use crate::custom_types::Semver;
    use crate::testing;

    use super::*;

    #[test]
    fn test_diff_log_internal_state_read_write() {
        let pack_name = "react".to_string();
        let other_pack_name = "lodash".to_string();

        let v1 = Semver {
            major: 1,
            minor: 2,
            bug: 3,
            prerelease: vec![PrereleaseTag::Int(5), PrereleaseTag::String("alpha".into())],
            build: vec!["b23423".into()],
        };
        let v2 = Semver {
            major: 2,
            minor: 1,
            bug: 6,
            prerelease: vec![],
            build: vec![],
        };

        let state0 = InternalDiffLogStateRow {
            package_name: pack_name.clone(),
            package_only_packument_hash: "asdfqwer".into(),
            versions: vec![],
        };
        let state1 = InternalDiffLogStateRow {
            package_name: pack_name.clone(),
            package_only_packument_hash: "asdfqwer".into(),
            versions: vec![InternalDiffLogVersionStateElem {
                v: v1.clone(),
                pack_hash: "asdf1234".into(),
                deleted: false,
            }],
        };
        let state2 = InternalDiffLogStateRow {
            package_name: pack_name.clone(),

            package_only_packument_hash: "otherhash".into(),
            versions: vec![
                InternalDiffLogVersionStateElem {
                    v: v1.clone(),
                    pack_hash: "asdf1234".into(),
                    deleted: false,
                },
                InternalDiffLogVersionStateElem {
                    v: v2.clone(),
                    pack_hash: "qwer567".into(),
                    deleted: false,
                },
            ],
        };
        let state3 = InternalDiffLogStateRow {
            package_name: pack_name.clone(),

            package_only_packument_hash: "otherhash".into(),
            versions: vec![
                InternalDiffLogVersionStateElem {
                    v: v1.clone(),
                    pack_hash: "asdf1234".into(),
                    deleted: true,
                },
                InternalDiffLogVersionStateElem {
                    v: v2.clone(),
                    pack_hash: "qwer567".into(),
                    deleted: false,
                },
            ],
        };
        let state4 = InternalDiffLogStateRow {
            package_name: pack_name.clone(),

            package_only_packument_hash: "otherhash".into(),
            versions: vec![
                InternalDiffLogVersionStateElem {
                    v: v1,
                    pack_hash: "asdf1234".into(),
                    deleted: true,
                },
                InternalDiffLogVersionStateElem {
                    v: v2,
                    pack_hash: "qwer567".into(),
                    deleted: false,
                },
            ],
        };

        testing::using_test_db(|conn| {
            assert_eq!(lookup_package(&pack_name, conn), None);
            assert_eq!(lookup_package(&other_pack_name, conn), None);

            create_packages(vec![state0.clone()], conn);
            assert_eq!(lookup_package(&pack_name, conn), Some(state0));
            assert_eq!(lookup_package(&other_pack_name, conn), None);

            update_packages(vec![state1.clone()], conn);
            assert_eq!(lookup_package(&pack_name, conn), Some(state1));
            assert_eq!(lookup_package(&other_pack_name, conn), None);

            update_packages(vec![state2.clone()], conn);
            assert_eq!(lookup_package(&pack_name, conn), Some(state2));
            assert_eq!(lookup_package(&other_pack_name, conn), None);

            update_packages(vec![state3.clone()], conn);
            assert_eq!(lookup_package(&pack_name, conn), Some(state3));
            assert_eq!(lookup_package(&other_pack_name, conn), None);

            update_packages(vec![state4.clone()], conn);
            assert_eq!(lookup_package(&pack_name, conn), Some(state4));
            assert_eq!(lookup_package(&other_pack_name, conn), None);
        });
    }
}
