use std::io::Write;

use crate::schema::sql_types::SemverStruct;

use super::sql_types::*;
use super::{PrereleaseTag, Semver};
use diesel::deserialize::{self, FromSql};
use diesel::pg::{Pg, PgValue};
use diesel::serialize::{self, IsNull, Output, ToSql, WriteTuple};
use diesel::sql_types::{Array, Int8, Nullable, Record, Text};

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
    fn to_sql(&self, out: &mut Output<Pg>) -> serialize::Result {
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
    fn to_sql(&self, out: &mut Output<Pg>) -> serialize::Result {
        match self {
            PrereleaseTagTypeEnum::String => out.write_all(b"string")?,
            PrereleaseTagTypeEnum::Int => out.write_all(b"int")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<PrereleaseTagStructSql, Pg> for PrereleaseTag {
    fn from_sql(bytes: PgValue) -> deserialize::Result<Self> {
        let (tag_type, str_case, int_case) = FromSql::<
            Record<(PrereleaseTagTypeEnumSql, Nullable<Text>, Nullable<Int8>)>,
            Pg,
        >::from_sql(bytes)?;
        match tag_type {
            PrereleaseTagTypeEnum::String => {
                Ok(PrereleaseTag::String(super::helpers::not_none(str_case)?))
            }
            PrereleaseTagTypeEnum::Int => {
                Ok(PrereleaseTag::Int(super::helpers::not_none(int_case)?))
            }
        }
    }
}

impl FromSql<PrereleaseTagTypeEnumSql, Pg> for PrereleaseTagTypeEnum {
    fn from_sql(bytes: PgValue) -> deserialize::Result<Self> {
        let bytes = bytes.as_bytes();

        match bytes {
            b"string" => Ok(PrereleaseTagTypeEnum::String),
            b"int" => Ok(PrereleaseTagTypeEnum::Int),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}

impl FromSql<SemverStruct, Pg> for Semver {
    fn from_sql(bytes: PgValue) -> deserialize::Result<Self> {
        let (major, minor, bug, prerelease, build): (
            i64,
            i64,
            i64,
            Option<Vec<PrereleaseTag>>,
            Option<Vec<String>>,
        ) = FromSql::<
            Record<(
                Int8,
                Int8,
                Int8,
                Nullable<Array<PrereleaseTagStructSql>>,
                Nullable<Array<Text>>,
            )>,
            Pg,
        >::from_sql(bytes)?;
        Ok(Semver {
            major,
            minor,
            bug,
            prerelease: prerelease.unwrap_or_default(),
            build: build.unwrap_or_default(),
        })
    }
}

impl ToSql<SemverStruct, Pg> for Semver {
    fn to_sql(&self, out: &mut Output<Pg>) -> serialize::Result {
        WriteTuple::<(
            Int8,
            Int8,
            Int8,
            Nullable<Array<PrereleaseTagStructSql>>,
            Nullable<Array<Text>>,
        )>::write_tuple(
            &(
                self.major,
                self.minor,
                self.bug,
                if self.prerelease.is_empty() {
                    None
                } else {
                    Some(&self.prerelease)
                },
                if self.build.is_empty() {
                    None
                } else {
                    Some(&self.build)
                },
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

    table! {
        use diesel::sql_types::*;
        use crate::schema::sql_types::SemverStruct;

        test_semver_to_sql {
            id -> Integer,
            v -> SemverStruct,
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
            testing::using_temp_table(
                conn,
                "test_semver_to_sql",
                "id SERIAL PRIMARY KEY, v semver",
                |conn| {
                    let inserted = conn
                        .get_results(diesel::insert_into(test_semver_to_sql).values(&data))
                        .unwrap();
                    assert_eq!(data, inserted);

                    let filter_all = conn.load(test_semver_to_sql.filter(id.ge(1))).unwrap();
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
                    let filter_eq = conn
                        .load(test_semver_to_sql.filter(v.eq(Semver {
                            major: 1,
                            minor: 2,
                            bug: 3,
                            prerelease: vec![],
                            build: vec![],
                        })))
                        .unwrap();
                    assert_eq!(filter_eq_data, filter_eq);
                },
            );
        });
    }
}
