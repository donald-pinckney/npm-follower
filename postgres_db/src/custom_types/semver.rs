use diesel::pg::Pg;
use diesel::types::{ToSql, FromSql};
use diesel::deserialize;
use diesel::serialize::{self, Output, WriteTuple, IsNull};
use diesel::sql_types::{Record, Text, Nullable, Integer, Array};
use std::io::Write;
use super::sql_types::*;
use super::{Semver, PrereleaseTag};

// ---------- SemverSql <----> Semver




#[derive(Debug, PartialEq, FromSqlRow, AsExpression)]
#[sql_type = "PrereleaseTagTypeEnumSql"]
enum PrereleaseTagTypeEnum {
    String, Int
}

#[derive(SqlType)]
#[postgres(type_name = "prerelease_tag_type_enum")]
struct PrereleaseTagTypeEnumSql;

impl ToSql<PrereleaseTagStructSql, Pg> for PrereleaseTag {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        match self {
            PrereleaseTag::String(s) => {
                WriteTuple::<(PrereleaseTagTypeEnumSql, Nullable<Text>, Nullable<Integer>)>::write_tuple(&(PrereleaseTagTypeEnum::String, Some(s.as_str()), None as Option<i32>), out)
            },
            PrereleaseTag::Int(i) => {
                WriteTuple::<(PrereleaseTagTypeEnumSql, Nullable<Text>, Nullable<Integer>)>::write_tuple(&(PrereleaseTagTypeEnum::Int, None as Option<String>, Some(i)), out)
            }
        }
    }
}


impl ToSql<PrereleaseTagTypeEnumSql, Pg> for PrereleaseTagTypeEnum {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        match self {
            PrereleaseTagTypeEnum::String => out.write_all(b"string")?,
            PrereleaseTagTypeEnum::Int => out.write_all(b"int")?
        }
        Ok(IsNull::No)
    }
}

impl FromSql<PrereleaseTagStructSql, Pg> for PrereleaseTag {
    fn from_sql(bytes: Option<&[u8]>) -> deserialize::Result<Self> {
        let (tag_type, str_case, int_case) = FromSql::<Record<(PrereleaseTagTypeEnumSql, Nullable<Text>, Nullable<Integer>)>, Pg>::from_sql(bytes)?;
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
        let (major, minor, bug, prerelease, build) = FromSql::<Record<(Integer, Integer, Integer, Array<PrereleaseTagStructSql>, Array<PrereleaseTagStructSql>)>, Pg>::from_sql(bytes)?;
        Ok(Semver {
            major,
            minor,
            bug,
            prerelease,
            build
        })
    }
}

impl ToSql<SemverSql, Pg> for Semver {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        WriteTuple::<(Integer, Integer, Integer, Array<PrereleaseTagStructSql>, Array<PrereleaseTagStructSql>)>::write_tuple(&(self.major, self.major, self.bug, &self.prerelease, &self.build), out)
    }
}

