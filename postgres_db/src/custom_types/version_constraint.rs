use super::sql_types::*;
use super::{VersionComparator, VersionConstraint};
use diesel::deserialize::{self, FromSql};
use diesel::pg::{Pg, PgValue};
use diesel::serialize::{self, Output, ToSql, WriteTuple};
use diesel::sql_types::{Array, Record};

#[derive(Debug, PartialEq, FromSqlRow, AsExpression)]
#[sql_type = "ConstraintConjunctsSql"]
struct ConstraintConjunctsBorrowed<'a>(&'a Vec<VersionComparator>);

#[derive(Debug, PartialEq, FromSqlRow, AsExpression)]
#[sql_type = "ConstraintConjunctsSql"]
struct ConstraintConjunctsOwned(Vec<VersionComparator>);

// ---------- ConstraintConjunctsSql <----> ConstraintConjuncts

impl<'a> ToSql<ConstraintConjunctsSql, Pg> for ConstraintConjunctsBorrowed<'a> {
    fn to_sql(&self, out: &mut Output<Pg>) -> serialize::Result {
        WriteTuple::<(Array<VersionComparatorSql>,)>::write_tuple(&(&self.0,), out)
    }
}

impl<'a> FromSql<ConstraintConjunctsSql, Pg> for ConstraintConjunctsOwned {
    fn from_sql(bytes: PgValue) -> deserialize::Result<Self> {
        let (stuff,): (Vec<VersionComparator>,) =
            FromSql::<Record<(Array<VersionComparatorSql>,)>, Pg>::from_sql(bytes)?;
        // todo!()
        Ok(ConstraintConjunctsOwned(stuff))
    }
}

// ---------- Array<ConstraintConjunctsSql> <----> VersionConstraint

impl ToSql<Array<ConstraintConjunctsSql>, Pg> for VersionConstraint {
    fn to_sql<'a>(&'a self, out: &mut Output<'a, '_, Pg>) -> serialize::Result {
        let disjuncts: Vec<_> = self
            .0
            .iter()
            .map(|d| ConstraintConjunctsBorrowed(d))
            .collect();
        let mut stuff = out.reborrow();
        // todo!()
        // Failure of type inference :(
        ToSql::<Array<ConstraintConjunctsSql>, Pg>::to_sql(&disjuncts, &mut stuff)
    }
}

impl FromSql<Array<ConstraintConjunctsSql>, Pg> for VersionConstraint {
    fn from_sql(bytes: PgValue) -> deserialize::Result<Self> {
        let vals: Vec<ConstraintConjunctsOwned> =
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
        use crate::custom_types::sql_types::ConstraintConjunctsSql;

        test_version_constraint_to_sql {
            id -> Integer,
            c -> Array<ConstraintConjunctsSql>,
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

            let inserted = conn
                .get_results(diesel::insert_into(test_version_constraint_to_sql).values(&data))
                .unwrap();
            assert_eq!(data, inserted);

            let filter_all = conn
                .load(test_version_constraint_to_sql.filter(id.ge(1)))
                .unwrap();
            assert_eq!(data, filter_all);

            let bad_data1 = vec![TestVersionConstraintToSql {
                id: 6,
                c: VersionConstraint(vec![]),
            }];
            let (_, info) = unwrap_db_error(
                conn.get_results::<_, TestVersionConstraintToSql>(
                    diesel::insert_into(test_version_constraint_to_sql).values(&bad_data1),
                )
                .unwrap_err(), // .get_results::<TestVersionConstraintToSql>(&mut conn.conn)
            );
            assert!(info
                .message()
                .contains(r#"violates check constraint "constraint_disjuncts_check""#));

            let bad_data2 = vec![TestVersionConstraintToSql {
                id: 6,
                c: VersionConstraint(vec![vec![]]),
            }];
            let (_, info) = unwrap_db_error(
                conn.get_results::<_, TestVersionConstraintToSql>(
                    diesel::insert_into(test_version_constraint_to_sql).values(&bad_data2),
                )
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
                conn.get_results::<_, TestVersionConstraintToSql>(
                    diesel::insert_into(test_version_constraint_to_sql).values(&bad_data3),
                )
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
                conn.get_results::<_, TestVersionConstraintToSql>(
                    diesel::insert_into(test_version_constraint_to_sql).values(&bad_data4),
                )
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
