use sql_types::*;
use diesel::sql_types::Array;

#[derive(Debug, PartialEq, FromSqlRow, AsExpression)]
#[sql_type = "RepositorySql"]
pub enum Repository {
    Git(String)
}

#[derive(Debug, PartialEq, FromSqlRow, AsExpression, Clone, Eq, Hash)]
#[sql_type = "SemverSql"]
pub struct Semver {
    major: i32,
    minor: i32,
    bug: i32,
    prerelease: Vec<PrereleaseTag>,
    build: Vec<PrereleaseTag>
}

#[derive(Debug, PartialEq, FromSqlRow, AsExpression, Clone, Eq, Hash)]
#[sql_type = "PrereleaseTagStructSql"]
pub enum PrereleaseTag {
    String(String),
    Int(i32)
}


#[derive(Debug, PartialEq, FromSqlRow, AsExpression)]
#[sql_type = "ParsedSpecStructSql"]
pub enum ParsedSpec {
    Range(VersionConstraint),
    Tag(String),
    Git(String),
    Remote(String),
    Alias(String, Option<i64>, AliasSubspec),
    File(String),
    Directory(String)
}

#[derive(Debug, PartialEq)]
pub enum AliasSubspec {
    Range(VersionConstraint),
    Tag(String)
}

#[derive(Debug, PartialEq, FromSqlRow, AsExpression, Clone)]
#[sql_type = "VersionComparatorSql"]
pub enum VersionComparator {
    Any,
    Eq(Semver),
    Gt(Semver),
    Gte(Semver),
    Lt(Semver),
    Lte(Semver)
}

#[derive(Debug, PartialEq, FromSqlRow, AsExpression, Clone)]
#[sql_type = "Array<ConstraintConjunctsSql>"]
pub struct VersionConstraint(Vec<Vec<VersionComparator>>);


pub mod sql_types {
    #[derive(SqlType, QueryId)]
    #[postgres(type_name = "repository_struct")] // or should it be repository (domain)?
    pub struct RepositorySql;

    #[derive(SqlType, QueryId)]
    #[postgres(type_name = "semver_struct")] // or should it be semver (domain)?
    pub struct SemverSql;

    #[derive(SqlType)]
    #[postgres(type_name = "prerelease_tag_struct")] // or should it be prerelease_tag (domain)?
    pub struct PrereleaseTagStructSql;

    #[derive(SqlType, QueryId)]
    #[postgres(type_name = "version_comparator_struct")] // or should it be version_comparator (domain)?
    pub struct VersionComparatorSql;

    #[derive(SqlType)]
    #[postgres(type_name = "constraint_conjuncts_struct")] // or should it be constraint_conjuncts (domain)?
    pub struct ConstraintConjunctsSql;

    #[derive(SqlType)]
    #[postgres(type_name = "parsed_spec_struct")] // or should it be parsed_spec (domain)?
    pub struct ParsedSpecStructSql;
}


#[allow(non_camel_case_types)]
pub mod sql_type_names {
    pub type Repository_struct = super::sql_types::RepositorySql;
    pub type Semver_struct = super::sql_types::SemverSql;
    pub type Version_comparator = super::sql_types::VersionComparatorSql;
    pub type Constraint_conjuncts_struct = super::sql_types::ConstraintConjunctsSql;
    pub type Parsed_spec_struct = super::sql_types::ParsedSpecStructSql;
}


mod repository;
mod semver;
mod version_comparator;
mod version_constraint;
mod parsing;
mod parsed_spec;
