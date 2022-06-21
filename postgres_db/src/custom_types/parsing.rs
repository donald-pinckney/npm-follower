use std::num::ParseIntError;
use std::str::FromStr;

use super::{Semver, VersionComparator};


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
pub enum ParseVersionComparatorError {
    UnknownOp(String),
    Other
}

impl FromStr for VersionComparator {
    type Err = ParseVersionComparatorError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        todo!()
    }
}