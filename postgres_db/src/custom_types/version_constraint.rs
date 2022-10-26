use super::sql_types::*;
use super::{VersionComparator, VersionConstraint};
use diesel::deserialize;
use diesel::pg::Pg;
use diesel::serialize::{self, Output, WriteTuple};
use diesel::sql_types::{Array, Record};
use diesel::types::{FromSql, ToSql};
use std::io::Write;

#[derive(Debug, PartialEq, FromSqlRow, AsExpression)]
#[sql_type = "ConstraintConjunctsSql"]
struct ConstraintConjuncts(Vec<VersionComparator>);

// ---------- ConstraintConjunctsSql <----> ConstraintConjuncts

impl<'a> ToSql<ConstraintConjunctsSql, Pg> for ConstraintConjuncts {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        WriteTuple::<(Array<VersionComparatorSql>,)>::write_tuple(&(&self.0,), out)
    }
}

impl<'a> FromSql<ConstraintConjunctsSql, Pg> for ConstraintConjuncts {
    fn from_sql(bytes: Option<&[u8]>) -> deserialize::Result<Self> {
        let (stuff,): (Vec<VersionComparator>,) =
            FromSql::<Record<(Array<VersionComparatorSql>,)>, Pg>::from_sql(bytes)?;
        Ok(ConstraintConjuncts(stuff))
    }
}

// ---------- Array<ConstraintConjunctsSql> <----> VersionConstraint

impl ToSql<Array<ConstraintConjunctsSql>, Pg> for VersionConstraint {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        let disjuncts: Vec<_> = self
            .0
            .iter()
            .map(|d| ConstraintConjuncts(d.clone()))
            .collect();
        // Failure of type inference :(
        ToSql::<Array<ConstraintConjunctsSql>, Pg>::to_sql(&disjuncts, out)
    }
}

impl FromSql<Array<ConstraintConjunctsSql>, Pg> for VersionConstraint {
    fn from_sql(bytes: Option<&[u8]>) -> deserialize::Result<Self> {
        let vals: Vec<ConstraintConjuncts> =
            FromSql::<Array<ConstraintConjunctsSql>, Pg>::from_sql(bytes)?;
        Ok(VersionConstraint(vals.into_iter().map(|d| d.0).collect()))
    }
}

// Unit tests
#[cfg(test)]
mod tests {
    use crate::custom_types::{PrereleaseTag, Semver, VersionComparator, VersionConstraint};
    use crate::testing;
    use diesel::prelude::*;
    use diesel::RunQueryDsl;

    table! {
        use diesel::sql_types::*;
        use crate::custom_types::sql_type_names::Constraint_conjuncts_struct;

        test_version_constraint_to_sql {
            id -> Integer,
            c -> Array<Constraint_conjuncts_struct>,
        }
    }

    #[derive(Insertable, Queryable, Identifiable, Debug, PartialEq)]
    #[table_name = "test_version_constraint_to_sql"]
    struct TestVersionConstraintToSql {
        id: i32,
        c: VersionConstraint,
    }

    #[test]
    fn test_version_constraint_to_sql_fn() {
        use self::test_version_constraint_to_sql::dsl::*;

        let c1 = VersionComparator::Eq(Semver {
            major: 3,
            minor: 4,
            bug: 5,
            prerelease: vec![PrereleaseTag::Int(8)],
            build: vec!["alpha".into(), "1".into()],
        });

        let c2 = VersionComparator::Lte(Semver {
            major: 8,
            minor: 9,
            bug: 12,
            prerelease: vec![],
            build: vec![],
        });

        let c3 = VersionComparator::Any;

        let data = vec![
            TestVersionConstraintToSql {
                id: 1,
                c: VersionConstraint(vec![vec![c1.clone()]]),
            },
            TestVersionConstraintToSql {
                id: 2,
                c: VersionConstraint(vec![vec![c1.clone(), c2.clone()]]),
            },
            TestVersionConstraintToSql {
                id: 3,
                c: VersionConstraint(vec![
                    vec![c1.clone(), c3.clone()],
                    vec![c2.clone(), c1.clone()],
                ]),
            },
            TestVersionConstraintToSql {
                id: 4,
                c: VersionConstraint(vec![vec![c1.clone(), c2.clone()], vec![c2.clone()]]),
            },
            TestVersionConstraintToSql {
                id: 5,
                c: VersionConstraint(vec![vec![c3], vec![c1, c2.clone()]]),
            },
        ];

        testing::using_test_db(|conn| {
            let _temp_table = testing::TempTable::new(
                conn,
                "test_version_constraint_to_sql",
                "id SERIAL PRIMARY KEY, c constraint_disjuncts NOT NULL",
            );

            let inserted = diesel::insert_into(test_version_constraint_to_sql)
                .values(&data)
                .get_results(&conn.conn)
                .unwrap();
            assert_eq!(data, inserted);

            let filter_all = test_version_constraint_to_sql
                .filter(id.ge(1))
                .load(&conn.conn)
                .unwrap();
            assert_eq!(data, filter_all);

            let bad_data1 = vec![TestVersionConstraintToSql {
                id: 6,
                c: VersionConstraint(vec![]),
            }];
            let (_, info) = unwrap_db_error(
                diesel::insert_into(test_version_constraint_to_sql)
                    .values(&bad_data1)
                    .get_results::<TestVersionConstraintToSql>(&conn.conn)
                    .unwrap_err(),
            );
            assert!(info
                .message()
                .contains(r#"violates check constraint "constraint_disjuncts_check""#));

            let bad_data2 = vec![TestVersionConstraintToSql {
                id: 6,
                c: VersionConstraint(vec![vec![]]),
            }];
            let (_, info) = unwrap_db_error(
                diesel::insert_into(test_version_constraint_to_sql)
                    .values(&bad_data2)
                    .get_results::<TestVersionConstraintToSql>(&conn.conn)
                    .unwrap_err(),
            );
            assert!(info
                .message()
                .contains(r#"violates check constraint "constraint_conjuncts_check""#));

            let bad_data3 = vec![TestVersionConstraintToSql {
                id: 6,
                c: VersionConstraint(vec![vec![c2.clone()], vec![]]),
            }];
            let (_, info) = unwrap_db_error(
                diesel::insert_into(test_version_constraint_to_sql)
                    .values(&bad_data3)
                    .get_results::<TestVersionConstraintToSql>(&conn.conn)
                    .unwrap_err(),
            );
            assert!(info
                .message()
                .contains(r#"violates check constraint "constraint_conjuncts_check""#));

            let bad_data4 = vec![TestVersionConstraintToSql {
                id: 6,
                c: VersionConstraint(vec![vec![], vec![c2]]),
            }];
            let (_, info) = unwrap_db_error(
                diesel::insert_into(test_version_constraint_to_sql)
                    .values(&bad_data4)
                    .get_results::<TestVersionConstraintToSql>(&conn.conn)
                    .unwrap_err(),
            );
            assert!(info
                .message()
                .contains(r#"violates check constraint "constraint_conjuncts_check""#));
        });
    }

    fn unwrap_db_error(
        err: diesel::result::Error,
    ) -> (
        diesel::result::DatabaseErrorKind,
        Box<dyn diesel::result::DatabaseErrorInformation + Send + Sync>,
    ) {
        match err {
            diesel::result::Error::DatabaseError(x, y) => (x, y),
            _ => panic!("Expected DatabaseError, got: {}", err),
        }
    }
}
