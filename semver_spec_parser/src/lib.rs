use std::num::ParseIntError;

use postgres_db::custom_types::{Semver, ParsedSpec};

#[derive(Debug)]
pub enum ParseSemverError {
    MajorMinorBugParseIntError(ParseIntError),
    Other
}

pub fn parse_semver(s: &str) -> Result<Semver, ParseSemverError> {
    todo!()
}


#[derive(Debug)]
pub enum ParseSpecError {
    UnknownType(String),
    Other
}

pub fn parse_spec(s: &str) -> Result<ParsedSpec, ParseSpecError> {
    todo!()
}
