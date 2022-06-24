use diesel::pg::Pg;
use diesel::types::{ToSql, FromSql};
use diesel::deserialize;
use diesel::serialize::{self, Output, WriteTuple, IsNull};
use diesel::sql_types::{Record, Array, Nullable, Text, Int8};
use std::io::Write;
use super::sql_types::*;
use super::{VersionConstraint, ParsedSpec, AliasSubspec};



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
    Nullable<Text>
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
    Option<String>
);

impl<'a> ToSql<ParsedSpecStructSql, Pg> for ParsedSpec {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        let record: ParsedSpecStructRecordRust = match self {
            ParsedSpec::Range(vc) => (SpecTypeEnum::Range, Some(vc.clone()), None, None, None, None, None, None, None, None, None, None),
            ParsedSpec::Tag(tag) => (SpecTypeEnum::Tag, None, Some(tag.clone()), None, None, None, None, None, None, None, None, None),
            ParsedSpec::Git(git) => (SpecTypeEnum::Git, None, None, Some(git.clone()), None, None, None, None, None, None, None, None),
            ParsedSpec::Remote(url) => (SpecTypeEnum::Remote, None, None, None, Some(url.clone()), None, None, None, None, None, None, None),
            ParsedSpec::Alias(a_name, a_id, AliasSubspec::Range(vc)) => 
                (SpecTypeEnum::Alias, None, None, None, None, Some(a_name.clone()), *a_id, Some(AliasSubspecTypeEnum::Range), Some(vc.clone()), None, None, None),
            ParsedSpec::Alias(a_name, a_id, AliasSubspec::Tag(tag)) => 
                (SpecTypeEnum::Alias, None, None, None, None, Some(a_name.clone()), *a_id, Some(AliasSubspecTypeEnum::Tag), None, Some(tag.clone()), None, None),
            ParsedSpec::File(path) => (SpecTypeEnum::File, None, None, None, None, None, None, None, None, None, Some(path.clone()), None),
            ParsedSpec::Directory(path) => (SpecTypeEnum::Directory, None, None, None, None, None, None, None, None, None, None, Some(path.clone()))
        };

        WriteTuple::<ParsedSpecStructRecordSql>::write_tuple(
            &record,
            out
        )
    }
}

impl<'a> FromSql<ParsedSpecStructSql, Pg> for ParsedSpec {
    fn from_sql(bytes: Option<&[u8]>) -> deserialize::Result<Self> {
        let tup: ParsedSpecStructRecordRust = FromSql::<Record<ParsedSpecStructRecordSql>, Pg>::from_sql(bytes)?;
        match tup {
            (SpecTypeEnum::Range, Some(vc), None, None, None, None, None, None, None, None, None, None) => Ok(ParsedSpec::Range(vc)),
            (SpecTypeEnum::Tag, None, Some(tag), None, None, None, None, None, None, None, None, None) => Ok(ParsedSpec::Tag(tag)),
            (SpecTypeEnum::Git, None, None, Some(git), None, None, None, None, None, None, None, None) => Ok(ParsedSpec::Git(git)),
            (SpecTypeEnum::Remote, None, None, None, Some(url), None, None, None, None, None, None, None) => Ok(ParsedSpec::Remote(url)),
            (SpecTypeEnum::Alias, None, None, None, None, Some(a_name), a_id, Some(AliasSubspecTypeEnum::Range), Some(vc), None, None, None) => Ok(ParsedSpec::Alias(a_name, a_id, AliasSubspec::Range(vc))),
            (SpecTypeEnum::Alias, None, None, None, None, Some(a_name), a_id, Some(AliasSubspecTypeEnum::Tag), None, Some(tag), None, None) => Ok(ParsedSpec::Alias(a_name, a_id, AliasSubspec::Tag(tag))),
            (SpecTypeEnum::File, None, None, None, None, None, None, None, None, None, Some(path), None) => Ok(ParsedSpec::File(path)),
            (SpecTypeEnum::Directory, None, None, None, None, None, None, None, None, None, None, Some(path)) => Ok(ParsedSpec::Directory(path)),
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
    Directory
}

#[derive(SqlType)]
#[postgres(type_name = "dependency_type_enum")]
struct SpecTypeEnumSql;


impl ToSql<SpecTypeEnumSql, Pg> for SpecTypeEnum {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        match self {
            SpecTypeEnum::Range => out.write_all(b"range")?,
            SpecTypeEnum::Tag => out.write_all(b"tag")?,
            SpecTypeEnum::Git => out.write_all(b"git")?,
            SpecTypeEnum::Remote => out.write_all(b"remote")?,
            SpecTypeEnum::Alias => out.write_all(b"alias")?,
            SpecTypeEnum::File => out.write_all(b"file")?,
            SpecTypeEnum::Directory => out.write_all(b"directory")?,                        
        }
        Ok(IsNull::No)
    }
}

impl FromSql<SpecTypeEnumSql, Pg> for SpecTypeEnum {
    fn from_sql(bytes: Option<&[u8]>) -> deserialize::Result<Self> {
        match not_none!(bytes) {
            b"range" => Ok(SpecTypeEnum::Range),
            b"tag" => Ok(SpecTypeEnum::Tag),
            b"git" => Ok(SpecTypeEnum::Git),
            b"remote" => Ok(SpecTypeEnum::Remote),
            b"alias" => Ok(SpecTypeEnum::Alias),
            b"file" => Ok(SpecTypeEnum::File),
            b"directory" => Ok(SpecTypeEnum::Directory),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}



// ---------- AliasSubspecTypeEnumSql <----> AliasSubspecTypeEnum

#[derive(Debug, PartialEq, FromSqlRow, AsExpression)]
#[sql_type = "AliasSubspecTypeEnumSql"]
enum AliasSubspecTypeEnum {
    Range, Tag
}

#[derive(SqlType)]
#[postgres(type_name = "alias_subdependency_type_enum")]
struct AliasSubspecTypeEnumSql;


impl ToSql<AliasSubspecTypeEnumSql, Pg> for AliasSubspecTypeEnum {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        match self {
            AliasSubspecTypeEnum::Range => out.write_all(b"range")?,
            AliasSubspecTypeEnum::Tag => out.write_all(b"tag")?,           
        }
        Ok(IsNull::No)
    }
}

impl FromSql<AliasSubspecTypeEnumSql, Pg> for AliasSubspecTypeEnum {
    fn from_sql(bytes: Option<&[u8]>) -> deserialize::Result<Self> {
        match not_none!(bytes) {
            b"range" => Ok(AliasSubspecTypeEnum::Range),
            b"tag" => Ok(AliasSubspecTypeEnum::Tag),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}




// Unit tests
#[cfg(test)]
mod tests {
    use diesel::prelude::*;
    use diesel::RunQueryDsl;
    use crate::custom_types::{Semver, VersionComparator, PrereleaseTag, VersionConstraint, ParsedSpec, AliasSubspec};
    use crate::testing;

    table! {
        use diesel::sql_types::*;
        use crate::custom_types::sql_type_names::Parsed_spec_struct;

        test_parsed_spec_to_sql {
            id -> Integer,
            s -> Parsed_spec_struct,
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

        let vc = VersionConstraint(vec![vec![c1.clone(), c2.clone(), c3.clone()]]);




        let data = vec![
            TestParsedSpecToSql {
                id: 1,
                s: ParsedSpec::Range(vc.clone())
            },
            TestParsedSpecToSql {
                id: 2,
                s: ParsedSpec::Tag("some_tag".into())
            },
            TestParsedSpecToSql {
                id: 3,
                s: ParsedSpec::Git("https://some/github.stuff".into())
            },
            TestParsedSpecToSql {
                id: 4,
                s: ParsedSpec::Remote("https://some/tarball.tgz".into())
            },
            TestParsedSpecToSql {
                id: 5,
                s: ParsedSpec::Alias("bar".into(), None, AliasSubspec::Range(vc.clone()))
            },
            TestParsedSpecToSql {
                id: 6,
                s: ParsedSpec::Alias("bar".into(), None, AliasSubspec::Tag("some_tag".into()))
            },
            TestParsedSpecToSql {
                id: 7,
                s: ParsedSpec::Alias("bar".into(), Some(75), AliasSubspec::Range(vc))
            },
            TestParsedSpecToSql {
                id: 8,
                s: ParsedSpec::Alias("bar".into(), Some(75), AliasSubspec::Tag("some_tag".into()))
            },
            TestParsedSpecToSql {
                id: 9,
                s: ParsedSpec::File("some/path.tgz".into())
            },
            TestParsedSpecToSql {
                id: 10,
                s: ParsedSpec::Directory("../some/package/directory".into())
            },
        ];



        let conn = testing::test_connect();
        let _temp_table = testing::TempTable::new(&conn, "test_parsed_spec_to_sql", "id SERIAL PRIMARY KEY, s parsed_spec NOT NULL");

        let inserted = diesel::insert_into(test_parsed_spec_to_sql).values(&data).get_results(&conn.conn).unwrap();
        assert_eq!(data, inserted);

        let filter_all = test_parsed_spec_to_sql
            .filter(id.ge(1))
            .load(&conn.conn)
            .unwrap();
        assert_eq!(data, filter_all);
    }
}
