use diesel::pg::Pg;
use diesel::types::{ToSql, FromSql};
use diesel::deserialize;
use diesel::serialize::{self, Output, WriteTuple, IsNull};
use diesel::sql_types::{Record, Nullable, Array, Integer};
use std::io::Write;
use super::sql_types::*;
use super::{Semver, VersionComparator, VersionConstraint};



#[derive(Debug, PartialEq, FromSqlRow, AsExpression)]
#[sql_type = "ConstraintConjunctsSql"]
struct ConstraintConjuncts(Vec<VersionComparator>);

// ---------- ConstraintConjunctsSql <----> ConstraintConjuncts


impl<'a> ToSql<ConstraintConjunctsSql, Pg> for ConstraintConjuncts {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        WriteTuple::<(Array<VersionComparatorSql>,)>::write_tuple(
            &(&self.0,), 
            out
        )
    }
}

impl<'a> FromSql<ConstraintConjunctsSql, Pg> for ConstraintConjuncts {
    fn from_sql(bytes: Option<&[u8]>) -> deserialize::Result<Self> {
        let (stuff,): (Vec<VersionComparator>,) = FromSql::<Record<(Array<VersionComparatorSql>,)>, Pg>::from_sql(bytes)?;
        Ok(ConstraintConjuncts(stuff))
    }
}



// ---------- Array<ConstraintConjunctsSql> <----> VersionConstraint


impl ToSql<Array<ConstraintConjunctsSql>, Pg> for VersionConstraint {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        let disjuncts: Vec<_> = self.0.iter().map(|d| ConstraintConjuncts(d.clone())).collect();
        // Failure of type inference :(
        ToSql::<Array<ConstraintConjunctsSql>, Pg>::to_sql(&disjuncts, out)
    }
}

impl FromSql<Array<ConstraintConjunctsSql>, Pg> for VersionConstraint {
    fn from_sql(bytes: Option<&[u8]>) -> deserialize::Result<Self> {
        let vals: Vec<ConstraintConjuncts> = FromSql::<Array<ConstraintConjunctsSql>, Pg>::from_sql(bytes)?;
        Ok(VersionConstraint(vals.into_iter().map(|d| d.0).collect()))
    }
}




// Unit tests
#[cfg(test)]
mod tests {
    use diesel::prelude::*;
    use diesel::RunQueryDsl;
    use crate::custom_types::{Semver, VersionComparator, PrereleaseTag, VersionConstraint};
    use crate::testing;

    table! {
        use diesel::sql_types::*;
        use crate::custom_types::sql_type_names::Constraint_conjuncts;

        test_version_constraint_to_sql {
            id -> Integer,
            c -> Array<Constraint_conjuncts>,
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
            build: vec![PrereleaseTag::String("alpha".into()), PrereleaseTag::Int(1)] 
        });

        let c2 = VersionComparator::Lte(Semver { 
            major: 8, 
            minor: 9, 
            bug: 12, 
            prerelease: vec![],
            build: vec![]
        });

        let c3 = VersionComparator::Any;


        let data = vec![
            TestVersionConstraintToSql {
                id: 1,
                c: VersionConstraint(vec![])
            },
            TestVersionConstraintToSql {
                id: 2,
                c: VersionConstraint(vec![vec![c1.clone()]])
            },
            TestVersionConstraintToSql {
                id: 3,
                c: VersionConstraint(vec![vec![c1.clone(), c2.clone()]])
            },
            TestVersionConstraintToSql {
                id: 4,
                c: VersionConstraint(vec![vec![c1.clone(), c2.clone()], vec![c2.clone(), c1.clone()]])
            },
            TestVersionConstraintToSql {
                id: 5,
                c: VersionConstraint(vec![vec![c1.clone(), c2.clone()], vec![c2.clone()]])
            },
            TestVersionConstraintToSql {
                id: 6,
                c: VersionConstraint(vec![vec![c2.clone()], vec![c1.clone(), c2.clone()]])
            },
        ];

        let conn = testing::test_connect();
        let _temp_table = testing::TempTable::new(&conn, "test_version_constraint_to_sql", "id SERIAL PRIMARY KEY, c constraint_conjuncts[] NOT NULL");

        let inserted = diesel::insert_into(test_version_constraint_to_sql).values(&data).get_results(&conn.conn).unwrap();
        assert_eq!(data, inserted);

        let filter_all = test_version_constraint_to_sql
            .filter(id.ge(1))
            .load(&conn.conn)
            .unwrap();
        assert_eq!(data, filter_all);
    }
}
