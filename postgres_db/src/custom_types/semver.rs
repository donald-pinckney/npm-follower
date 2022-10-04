use super::sql_types::*;
use super::{PrereleaseTag, Semver};
use diesel::deserialize;
use diesel::pg::Pg;
use diesel::serialize::{self, IsNull, Output, WriteTuple};
use diesel::sql_types::{Array, Int8, Nullable, Record, Text};
use diesel::types::{FromSql, ToSql};
use std::io::Write;

// ---------- SemverSql <----> Semver

#[derive(Debug, PartialEq, FromSqlRow, AsExpression)]
#[sql_type = "PrereleaseTagTypeEnumSql"]
enum PrereleaseTagTypeEnum {
    String,
    Int,
}

#[derive(SqlType)]
#[postgres(type_name = "prerelease_tag_type_enum")]
struct PrereleaseTagTypeEnumSql;

impl ToSql<PrereleaseTagStructSql, Pg> for PrereleaseTag {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        match self {
            PrereleaseTag::String(s) => WriteTuple::<(
                PrereleaseTagTypeEnumSql,
                Nullable<Text>,
                Nullable<Int8>,
            )>::write_tuple(
                &(
                    PrereleaseTagTypeEnum::String,
                    Some(s.as_str()),
                    None as Option<i64>,
                ),
                out,
            ),
            PrereleaseTag::Int(i) => WriteTuple::<(
                PrereleaseTagTypeEnumSql,
                Nullable<Text>,
                Nullable<Int8>,
            )>::write_tuple(
                &(PrereleaseTagTypeEnum::Int, None as Option<String>, Some(i)),
                out,
            ),
        }
    }
}

impl ToSql<PrereleaseTagTypeEnumSql, Pg> for PrereleaseTagTypeEnum {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        match self {
            PrereleaseTagTypeEnum::String => out.write_all(b"string")?,
            PrereleaseTagTypeEnum::Int => out.write_all(b"int")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<PrereleaseTagStructSql, Pg> for PrereleaseTag {
    fn from_sql(bytes: Option<&[u8]>) -> deserialize::Result<Self> {
        let (tag_type, str_case, int_case) = FromSql::<
            Record<(PrereleaseTagTypeEnumSql, Nullable<Text>, Nullable<Int8>)>,
            Pg,
        >::from_sql(bytes)?;
        match tag_type {
            PrereleaseTagTypeEnum::String => Ok(PrereleaseTag::String(not_none!(str_case))),
            PrereleaseTagTypeEnum::Int => Ok(PrereleaseTag::Int(not_none!(int_case))),
        }
    }
}

impl FromSql<PrereleaseTagTypeEnumSql, Pg> for PrereleaseTagTypeEnum {
    fn from_sql(bytes: Option<&[u8]>) -> deserialize::Result<Self> {
        match not_none!(bytes) {
            b"string" => Ok(PrereleaseTagTypeEnum::String),
            b"int" => Ok(PrereleaseTagTypeEnum::Int),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}

impl FromSql<SemverSql, Pg> for Semver {
    fn from_sql(bytes: Option<&[u8]>) -> deserialize::Result<Self> {
        let (major, minor, bug, prerelease, build) = FromSql::<
            Record<(Int8, Int8, Int8, Array<PrereleaseTagStructSql>, Array<Text>)>,
            Pg,
        >::from_sql(bytes)?;
        Ok(Semver {
            major,
            minor,
            bug,
            prerelease,
            build,
        })
    }
}

impl ToSql<SemverSql, Pg> for Semver {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        WriteTuple::<(Int8, Int8, Int8, Array<PrereleaseTagStructSql>, Array<Text>)>::write_tuple(
            &(
                self.major,
                self.minor,
                self.bug,
                &self.prerelease,
                &self.build,
            ),
            out,
        )
    }
}

// Unit tests
#[cfg(test)]
mod tests {
    use crate::custom_types::{PrereleaseTag, Semver};
    use crate::testing;
    use diesel::prelude::*;
    use diesel::RunQueryDsl;

    table! {
        use diesel::sql_types::*;
        use crate::custom_types::sql_type_names::Semver_struct;

        test_semver_to_sql {
            id -> Integer,
            v -> Semver_struct,
        }
    }

    #[derive(Insertable, Queryable, Identifiable, Debug, PartialEq)]
    #[table_name = "test_semver_to_sql"]
    struct TestSemverToSql {
        id: i32,
        v: Semver,
    }

    #[test]
    fn test_semver_to_sql_fn() {
        use self::test_semver_to_sql::dsl::*;

        let data = vec![
            TestSemverToSql {
                id: 1,
                v: Semver {
                    major: 1,
                    minor: 2,
                    bug: 3,
                    prerelease: vec![],
                    build: vec![],
                },
            },
            TestSemverToSql {
                id: 2,
                v: Semver {
                    major: 3,
                    minor: 4,
                    bug: 5,
                    prerelease: vec![PrereleaseTag::Int(8)],
                    build: vec!["alpha".into(), "1".into()],
                },
            },
            TestSemverToSql {
                id: 3,
                v: Semver {
                    major: 111111111111111,
                    minor: 222222222222222,
                    bug: 333333333333333,
                    prerelease: vec![PrereleaseTag::Int(444444444444444)],
                    build: vec!["alpha".into(), "555555555555555".into()],
                },
            },
        ];

        testing::using_test_db(|conn| {
            let _temp_table = testing::TempTable::new(
                &conn,
                "test_semver_to_sql",
                "id SERIAL PRIMARY KEY, v semver",
            );

            let inserted = diesel::insert_into(test_semver_to_sql)
                .values(&data)
                .get_results(&conn.conn)
                .unwrap();
            assert_eq!(data, inserted);

            let filter_all = test_semver_to_sql
                .filter(id.ge(1))
                .load(&conn.conn)
                .unwrap();
            assert_eq!(data, filter_all);

            let filter_eq_data = vec![TestSemverToSql {
                id: 1,
                v: Semver {
                    major: 1,
                    minor: 2,
                    bug: 3,
                    prerelease: vec![],
                    build: vec![],
                },
            }];
            let filter_eq = test_semver_to_sql
                .filter(v.eq(Semver {
                    major: 1,
                    minor: 2,
                    bug: 3,
                    prerelease: vec![],
                    build: vec![],
                }))
                .load(&conn.conn)
                .unwrap();
            assert_eq!(filter_eq_data, filter_eq);
        });
    }
}
