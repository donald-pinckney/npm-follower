use std::io::Write;

use crate::schema::sql_types::ParsedSpecStruct;

use super::sql_types::*;
use super::{AliasSubspec, ParsedSpec, VersionConstraint};
use diesel::deserialize::{self, FromSql};
use diesel::pg::{Pg, PgValue};
use diesel::serialize::{self, IsNull, Output, ToSql, WriteTuple};
use diesel::sql_types::{Array, Int8, Nullable, Record, Text};

// ---------- ParsedSpecStructSql <----> ParsedSpec

type ParsedSpecStructRecordSql = (
    SpecTypeEnumSql,
    Nullable<Array<ConstraintConjunctsSql>>,
    Nullable<Text>,
    Nullable<Text>,
    Nullable<Text>,
    Nullable<Text>,
    Nullable<Int8>,
    Nullable<AliasSubspecTypeEnumSql>,
    Nullable<Array<ConstraintConjunctsSql>>,
    Nullable<Text>,
    Nullable<Text>,
    Nullable<Text>,
    Nullable<Text>,
);

type ParsedSpecStructRecordRust = (
    SpecTypeEnum,
    Option<VersionConstraint>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
    Option<AliasSubspecTypeEnum>,
    Option<VersionConstraint>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

impl<'a> ToSql<ParsedSpecStruct, Pg> for ParsedSpec {
    fn to_sql(&self, out: &mut Output<Pg>) -> serialize::Result {
        let record: ParsedSpecStructRecordRust = match self {
            ParsedSpec::Range(vc) => (
                SpecTypeEnum::Range,
                Some(vc.clone()),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ),
            ParsedSpec::Tag(tag) => (
                SpecTypeEnum::Tag,
                None,
                Some(tag.clone()),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ),
            ParsedSpec::Git(git) => (
                SpecTypeEnum::Git,
                None,
                None,
                Some(git.clone()),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ),
            ParsedSpec::Remote(url) => (
                SpecTypeEnum::Remote,
                None,
                None,
                None,
                Some(url.clone()),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ),
            ParsedSpec::Alias(a_name, a_id, AliasSubspec::Range(vc)) => (
                SpecTypeEnum::Alias,
                None,
                None,
                None,
                None,
                Some(a_name.clone()),
                *a_id,
                Some(AliasSubspecTypeEnum::Range),
                Some(vc.clone()),
                None,
                None,
                None,
                None,
            ),
            ParsedSpec::Alias(a_name, a_id, AliasSubspec::Tag(tag)) => (
                SpecTypeEnum::Alias,
                None,
                None,
                None,
                None,
                Some(a_name.clone()),
                *a_id,
                Some(AliasSubspecTypeEnum::Tag),
                None,
                Some(tag.clone()),
                None,
                None,
                None,
            ),
            ParsedSpec::File(path) => (
                SpecTypeEnum::File,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(path.clone()),
                None,
                None,
            ),
            ParsedSpec::Directory(path) => (
                SpecTypeEnum::Directory,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(path.clone()),
                None,
            ),
            ParsedSpec::Invalid(message) => (
                SpecTypeEnum::Invalid,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(message.clone()),
            ),
        };

        WriteTuple::<ParsedSpecStructRecordSql>::write_tuple(&record, out)
    }
}

impl<'a> FromSql<ParsedSpecStruct, Pg> for ParsedSpec {
    fn from_sql(bytes: PgValue) -> deserialize::Result<Self> {
        let tup: ParsedSpecStructRecordRust =
            FromSql::<Record<ParsedSpecStructRecordSql>, Pg>::from_sql(bytes)?;
        match tup {
            (
                SpecTypeEnum::Range,
                Some(vc),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ) => Ok(ParsedSpec::Range(vc)),
            (
                SpecTypeEnum::Tag,
                None,
                Some(tag),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ) => Ok(ParsedSpec::Tag(tag)),
            (
                SpecTypeEnum::Git,
                None,
                None,
                Some(git),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ) => Ok(ParsedSpec::Git(git)),
            (
                SpecTypeEnum::Remote,
                None,
                None,
                None,
                Some(url),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ) => Ok(ParsedSpec::Remote(url)),
            (
                SpecTypeEnum::Alias,
                None,
                None,
                None,
                None,
                Some(a_name),
                a_id,
                Some(AliasSubspecTypeEnum::Range),
                Some(vc),
                None,
                None,
                None,
                None,
            ) => Ok(ParsedSpec::Alias(a_name, a_id, AliasSubspec::Range(vc))),
            (
                SpecTypeEnum::Alias,
                None,
                None,
                None,
                None,
                Some(a_name),
                a_id,
                Some(AliasSubspecTypeEnum::Tag),
                None,
                Some(tag),
                None,
                None,
                None,
            ) => Ok(ParsedSpec::Alias(a_name, a_id, AliasSubspec::Tag(tag))),
            (
                SpecTypeEnum::File,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(path),
                None,
                None,
            ) => Ok(ParsedSpec::File(path)),
            (
                SpecTypeEnum::Directory,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(path),
                None,
            ) => Ok(ParsedSpec::Directory(path)),
            (
                SpecTypeEnum::Invalid,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(message),
            ) => Ok(ParsedSpec::Invalid(message)),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}

// ---------- SpecTypeEnumSql <----> SpecTypeEnum

#[derive(Debug, PartialEq, FromSqlRow, AsExpression)]
#[sql_type = "SpecTypeEnumSql"]
enum SpecTypeEnum {
    Range,
    Tag,
    Git,
    Remote,
    Alias,
    File,
    Directory,
    Invalid,
}

#[derive(SqlType)]
#[postgres(type_name = "dependency_type_enum")]
struct SpecTypeEnumSql;

impl ToSql<SpecTypeEnumSql, Pg> for SpecTypeEnum {
    fn to_sql(&self, out: &mut Output<Pg>) -> serialize::Result {
        match self {
            SpecTypeEnum::Range => out.write_all(b"range")?,
            SpecTypeEnum::Tag => out.write_all(b"tag")?,
            SpecTypeEnum::Git => out.write_all(b"git")?,
            SpecTypeEnum::Remote => out.write_all(b"remote")?,
            SpecTypeEnum::Alias => out.write_all(b"alias")?,
            SpecTypeEnum::File => out.write_all(b"file")?,
            SpecTypeEnum::Directory => out.write_all(b"directory")?,
            SpecTypeEnum::Invalid => out.write_all(b"invalid")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<SpecTypeEnumSql, Pg> for SpecTypeEnum {
    fn from_sql(bytes: PgValue) -> deserialize::Result<Self> {
        let bytes = bytes.as_bytes();

        match bytes {
            b"range" => Ok(SpecTypeEnum::Range),
            b"tag" => Ok(SpecTypeEnum::Tag),
            b"git" => Ok(SpecTypeEnum::Git),
            b"remote" => Ok(SpecTypeEnum::Remote),
            b"alias" => Ok(SpecTypeEnum::Alias),
            b"file" => Ok(SpecTypeEnum::File),
            b"directory" => Ok(SpecTypeEnum::Directory),
            b"invalid" => Ok(SpecTypeEnum::Invalid),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}

// ---------- AliasSubspecTypeEnumSql <----> AliasSubspecTypeEnum

#[derive(Debug, PartialEq, FromSqlRow, AsExpression)]
#[sql_type = "AliasSubspecTypeEnumSql"]
enum AliasSubspecTypeEnum {
    Range,
    Tag,
}

#[derive(SqlType)]
#[postgres(type_name = "alias_subdependency_type_enum")]
struct AliasSubspecTypeEnumSql;

impl ToSql<AliasSubspecTypeEnumSql, Pg> for AliasSubspecTypeEnum {
    fn to_sql(&self, out: &mut Output<Pg>) -> serialize::Result {
        match self {
            AliasSubspecTypeEnum::Range => out.write_all(b"range")?,
            AliasSubspecTypeEnum::Tag => out.write_all(b"tag")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<AliasSubspecTypeEnumSql, Pg> for AliasSubspecTypeEnum {
    fn from_sql(bytes: PgValue) -> deserialize::Result<Self> {
        let bytes = bytes.as_bytes();

        match bytes {
            b"range" => Ok(AliasSubspecTypeEnum::Range),
            b"tag" => Ok(AliasSubspecTypeEnum::Tag),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}

// Unit tests
#[cfg(test)]
mod tests {
    use crate::custom_types::{
        AliasSubspec, ParsedSpec, PrereleaseTag, Semver, VersionComparator, VersionConstraint,
    };
    use crate::testing;
    use diesel::prelude::*;

    table! {
        use diesel::sql_types::*;
        use crate::schema::sql_types::ParsedSpecStruct;

        test_parsed_spec_to_sql {
            id -> Integer,
            s -> ParsedSpecStruct,
        }
    }

    #[derive(Insertable, Queryable, Identifiable, Debug, PartialEq)]
    #[table_name = "test_parsed_spec_to_sql"]
    struct TestParsedSpecToSql {
        id: i32,
        s: ParsedSpec,
    }

    #[test]
    fn test_parsed_spec_to_sql_fn() {
        use self::test_parsed_spec_to_sql::dsl::*;

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

        let vc = VersionConstraint(vec![vec![c1, c2, c3]]);

        let data = vec![
            TestParsedSpecToSql {
                id: 1,
                s: ParsedSpec::Range(vc.clone()),
            },
            TestParsedSpecToSql {
                id: 2,
                s: ParsedSpec::Tag("some_tag".into()),
            },
            TestParsedSpecToSql {
                id: 3,
                s: ParsedSpec::Git("https://some/github.stuff".into()),
            },
            TestParsedSpecToSql {
                id: 4,
                s: ParsedSpec::Remote("https://some/tarball.tgz".into()),
            },
            TestParsedSpecToSql {
                id: 5,
                s: ParsedSpec::Alias("bar".into(), None, AliasSubspec::Range(vc.clone())),
            },
            TestParsedSpecToSql {
                id: 6,
                s: ParsedSpec::Alias("bar".into(), None, AliasSubspec::Tag("some_tag".into())),
            },
            TestParsedSpecToSql {
                id: 7,
                s: ParsedSpec::Alias("bar".into(), Some(75), AliasSubspec::Range(vc)),
            },
            TestParsedSpecToSql {
                id: 8,
                s: ParsedSpec::Alias("bar".into(), Some(75), AliasSubspec::Tag("some_tag".into())),
            },
            TestParsedSpecToSql {
                id: 9,
                s: ParsedSpec::File("some/path.tgz".into()),
            },
            TestParsedSpecToSql {
                id: 10,
                s: ParsedSpec::Directory("../some/package/directory".into()),
            },
            TestParsedSpecToSql {
                id: 11,
                s: ParsedSpec::Invalid("error message".into()),
            },
        ];

        testing::using_test_db(|conn| {
            testing::using_temp_table(
                conn,
                "test_parsed_spec_to_sql",
                "id SERIAL PRIMARY KEY, s parsed_spec NOT NULL",
                |conn| {
                    let inserted = conn
                        .get_results(diesel::insert_into(test_parsed_spec_to_sql).values(&data))
                        .unwrap();
                    assert_eq!(data, inserted);

                    let filter_all = conn.load(test_parsed_spec_to_sql.filter(id.ge(1))).unwrap();
                    assert_eq!(data, filter_all);
                },
            );
        });
    }
}
