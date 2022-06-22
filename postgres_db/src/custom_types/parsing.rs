use std::num::ParseIntError;
use std::str::FromStr;

use super::{Semver, VersionConstraint};


#[derive(Debug)]
pub enum ParseSemverError {
    MajorMinorBugParseIntError(ParseIntError),
    Other
}

impl FromStr for Semver {
    type Err = ParseSemverError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        todo!()
    }
}


#[derive(Debug)]
pub enum ParseVersionConstraintError {
    UnknownOp(String),
    Other
}

impl FromStr for VersionConstraint {
    type Err = ParseVersionConstraintError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        todo!()
    }
}