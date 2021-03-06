use diesel::pg::Pg;
use diesel::types::{ToSql, FromSql};
use diesel::deserialize;
use diesel::serialize::{self, Output, WriteTuple, IsNull};
use diesel::sql_types::{Record, Nullable};
use std::io::Write;
use super::sql_types::*;
use super::{Semver, VersionComparator};


// ---------- VersionComparatorSql <----> VersionComparator


impl ToSql<VersionComparatorSql, Pg> for VersionComparator {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        match self {
            VersionComparator::Any => 
                WriteTuple::<(VersionOperatorEnumSql, Nullable<SemverSql>)>::write_tuple(&(VersionOperatorEnum::Any, None as Option<Semver>), out),
            VersionComparator::Eq(v) => 
                WriteTuple::<(VersionOperatorEnumSql, Nullable<SemverSql>)>::write_tuple(&(VersionOperatorEnum::Eq, Some(v)), out),
            VersionComparator::Gt(v) => 
                WriteTuple::<(VersionOperatorEnumSql, Nullable<SemverSql>)>::write_tuple(&(VersionOperatorEnum::Gt, Some(v)), out),
            VersionComparator::Gte(v) => 
                WriteTuple::<(VersionOperatorEnumSql, Nullable<SemverSql>)>::write_tuple(&(VersionOperatorEnum::Gte, Some(v)), out),
            VersionComparator::Lt(v) => 
                WriteTuple::<(VersionOperatorEnumSql, Nullable<SemverSql>)>::write_tuple(&(VersionOperatorEnum::Lt, Some(v)), out),
            VersionComparator::Lte(v) => 
                WriteTuple::<(VersionOperatorEnumSql, Nullable<SemverSql>)>::write_tuple(&(VersionOperatorEnum::Lte, Some(v)), out)
        }
    }
}

impl FromSql<VersionComparatorSql, Pg> for VersionComparator {
    fn from_sql(bytes: Option<&[u8]>) -> deserialize::Result<Self> {
        let (op, v): (VersionOperatorEnum, Option<Semver>) = FromSql::<Record<(VersionOperatorEnumSql, Nullable<SemverSql>)>, Pg>::from_sql(bytes)?;

        match op {
            VersionOperatorEnum::Any => {
                if v != None {
                    return Err(format!("VersionComparator::Any should not have a value").into());
                }
                Ok(VersionComparator::Any)
            },
            VersionOperatorEnum::Eq => Ok(VersionComparator::Eq(not_none!(v))),
            VersionOperatorEnum::Gt => Ok(VersionComparator::Gt(not_none!(v))),
            VersionOperatorEnum::Gte => Ok(VersionComparator::Gte(not_none!(v))),
            VersionOperatorEnum::Lt => Ok(VersionComparator::Lt(not_none!(v))),
            VersionOperatorEnum::Lte => Ok(VersionComparator::Lte(not_none!(v)))
        }
    }
}


#[derive(Debug, PartialEq, FromSqlRow, AsExpression)]
#[sql_type = "VersionOperatorEnumSql"]
enum VersionOperatorEnum {
    Any, Eq, Gt, Gte, Lt, Lte
}

#[derive(SqlType)]
#[postgres(type_name = "version_operator_enum")]
struct VersionOperatorEnumSql;


impl ToSql<VersionOperatorEnumSql, Pg> for VersionOperatorEnum {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        match self {
            VersionOperatorEnum::Any => out.write_all(b"*")?,
            VersionOperatorEnum::Eq => out.write_all(b"=")?,
            VersionOperatorEnum::Gt => out.write_all(b">")?,
            VersionOperatorEnum::Gte => out.write_all(b">=")?,
            VersionOperatorEnum::Lt => out.write_all(b"<")?,
            VersionOperatorEnum::Lte => out.write_all(b"<=")?,            
        }
        Ok(IsNull::No)
    }
}

impl FromSql<VersionOperatorEnumSql, Pg> for VersionOperatorEnum {
    fn from_sql(bytes: Option<&[u8]>) -> deserialize::Result<Self> {
        match not_none!(bytes) {
            b"*" => Ok(VersionOperatorEnum::Any),
            b"=" => Ok(VersionOperatorEnum::Eq),
            b">" => Ok(VersionOperatorEnum::Gt),
            b">=" => Ok(VersionOperatorEnum::Gte),
            b"<" => Ok(VersionOperatorEnum::Lt),
            b"<=" => Ok(VersionOperatorEnum::Lte),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}




// Unit tests
#[cfg(test)]
mod tests {
    use diesel::prelude::*;
    use diesel::RunQueryDsl;
    use crate::custom_types::{Semver, VersionComparator, PrereleaseTag};
    use crate::testing;

    table! {
        use diesel::sql_types::*;
        use crate::custom_types::sql_type_names::Version_comparator;

        test_version_comparator_to_sql {
            id -> Integer,
            vc -> Version_comparator,
        }
    }

    #[derive(Insertable, Queryable, Identifiable, Debug, PartialEq)]
    #[table_name = "test_version_comparator_to_sql"]
    struct TestVersionComparatorToSql {
        id: i32,
        vc: VersionComparator,
    }

    #[test]
    fn test_version_comparator_to_sql_fn() {
        use self::test_version_comparator_to_sql::dsl::*;

        let v1 = Semver { 
            major: 3, 
            minor: 4, 
            bug: 5, 
            prerelease: vec![PrereleaseTag::Int(8)], 
            build: vec!["alpha".into(), "1".into()] 
        };

        let v2 = Semver { 
            major: 8, 
            minor: 9, 
            bug: 12, 
            prerelease: vec![],
            build: vec![]
        };


        let data = vec![
            TestVersionComparatorToSql {
                id: 1,
                vc: VersionComparator::Any
            },
            TestVersionComparatorToSql {
                id: 2,
                vc: VersionComparator::Eq(v1.clone())
            },
            TestVersionComparatorToSql {
                id: 3,
                vc: VersionComparator::Gt(v1.clone())
            },
            TestVersionComparatorToSql {
                id: 4,
                vc: VersionComparator::Gte(v1.clone())
            },
            TestVersionComparatorToSql {
                id: 5,
                vc: VersionComparator::Lt(v1.clone())
            },
            TestVersionComparatorToSql {
                id: 6,
                vc: VersionComparator::Lte(v1)
            },
            TestVersionComparatorToSql {
                id: 7,
                vc: VersionComparator::Eq(v2.clone())
            },
            TestVersionComparatorToSql {
                id: 8,
                vc: VersionComparator::Gt(v2.clone())
            },
            TestVersionComparatorToSql {
                id: 9,
                vc: VersionComparator::Gte(v2.clone())
            },
            TestVersionComparatorToSql {
                id: 10,
                vc: VersionComparator::Lt(v2.clone())
            },
            TestVersionComparatorToSql {
                id: 11,
                vc: VersionComparator::Lte(v2.clone())
            },
        ];

        let conn = testing::test_connect();
        let _temp_table = testing::TempTable::new(&conn, "test_version_comparator_to_sql", "id SERIAL PRIMARY KEY, vc version_comparator");

        let inserted = diesel::insert_into(test_version_comparator_to_sql).values(&data).get_results(&conn.conn).unwrap();
        assert_eq!(data, inserted);

        let filter_all = test_version_comparator_to_sql
            .filter(id.ge(1))
            .load(&conn.conn)
            .unwrap();
        assert_eq!(data, filter_all);


        let filter_eq_data = vec![
            TestVersionComparatorToSql {
                id: 10,
                vc: VersionComparator::Lt(v2.clone())
            },
        ];
        let filter_eq = test_version_comparator_to_sql
            .filter(vc.eq(VersionComparator::Lt(v2)))
            .load(&conn.conn)
            .unwrap();
        assert_eq!(filter_eq_data, filter_eq);
    }
}
