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


