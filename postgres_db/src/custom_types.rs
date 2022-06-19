use diesel::pg::Pg;
use diesel::types::{ToSql, FromSql};
use diesel::deserialize;
use diesel::serialize::{self, Output, WriteTuple, IsNull};
use diesel::sql_types::{Integer, Record, Text, Nullable, Array};
use std::io::Write;
use sql_types::*;


#[allow(non_camel_case_types)]
pub mod sql_types {
    #[derive(SqlType)]
    #[postgres(type_name = "repository_struct")] // or should it be repository (domain)?
    pub struct RepositorySql;

    #[derive(SqlType)]
    #[postgres(type_name = "semver_struct")] // or should it be semver (domain)?
    pub struct SemverSql;

    #[derive(SqlType)]
    #[postgres(type_name = "version_comparator_struct")] // or should it be version_comparator (domain)?
    pub struct VersionComparatorSql;
}


#[allow(non_camel_case_types)]
pub mod sql_type_names {
    pub type Repository_struct = super::sql_types::RepositorySql;
    pub type Semver_struct = super::sql_types::SemverSql;
    pub type Version_comparator = super::sql_types::VersionComparatorSql;
}



// ---------- VersionComparatorSql <----> VersionComparator


#[derive(Debug, PartialEq, FromSqlRow, AsExpression)]
#[sql_type = "VersionComparatorSql"]
pub enum VersionComparator {
    Any,
    Eq(Semver),
    Gt(Semver),
    Gte(Semver),
    Lt(Semver),
    Lte(Semver)
}

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



// ---------- SemverSql <----> Semver

#[derive(Debug, PartialEq, FromSqlRow, AsExpression)]
#[sql_type = "SemverSql"]
pub struct Semver {
    major: i32,
    minor: i32,
    bug: i32,
    prerelease: Vec<PrereleaseTag>,
    build: Vec<PrereleaseTag>
}

#[derive(Debug, PartialEq, FromSqlRow, AsExpression)]
#[sql_type = "PrereleaseTagStructSql"]
pub enum PrereleaseTag {
    String(String),
    Int(i32)
}

#[derive(SqlType)]
#[postgres(type_name = "prerelease_tag_struct")] // or should it be prerelease_tag (domain)?
struct PrereleaseTagStructSql;

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






// ---------- RepositorySql <----> Repository

#[derive(Debug, PartialEq, FromSqlRow, AsExpression)]
#[sql_type = "RepositorySql"]
pub enum Repository {
    Git(String)
}


impl ToSql<RepositorySql, Pg> for Repository {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        match self {
            Repository::Git(url) => {
                WriteTuple::<(RepositoryTypeSql, Text)>::write_tuple(&(RepositoryType::Git, url.as_str()), out)
            }
        }
    }
}

impl FromSql<RepositorySql, Pg> for Repository {
    fn from_sql(bytes: Option<&[u8]>) -> deserialize::Result<Self> {
        let (repo_type, url) = FromSql::<Record<(RepositoryTypeSql, Text)>, Pg>::from_sql(bytes)?;
        match repo_type {
            RepositoryType::Git => Ok(Repository::Git(url)),
        }
    }
}



#[derive(SqlType)]
#[postgres(type_name = "repository_type_enum")]
struct RepositoryTypeSql;

#[derive(Debug, PartialEq, FromSqlRow, AsExpression)]
#[sql_type = "RepositoryTypeSql"]
enum RepositoryType {
    Git,
}

impl ToSql<RepositoryTypeSql, Pg> for RepositoryType {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        match *self {
            RepositoryType::Git => out.write_all(b"git")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<RepositoryTypeSql, Pg> for RepositoryType {
    fn from_sql(bytes: Option<&[u8]>) -> deserialize::Result<Self> {
        match not_none!(bytes) {
            b"git" => Ok(RepositoryType::Git),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}




