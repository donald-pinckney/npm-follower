use std::{num::ParseIntError, str::FromStr};
use std::fmt;

use lazy_regex::regex;

use postgres_db::custom_types::{Semver, ParsedSpec, PrereleaseTag};

#[derive(Debug)]
pub enum ParseSemverError {
    MajorMinorBugParseIntError(ParseIntError),
    Other
}

impl From<ParseIntError> for ParseSemverError {
    fn from(err: ParseIntError) -> Self {
        Self::MajorMinorBugParseIntError(err)
    }
}

fn parse_prerelease_tag(s: String) -> PrereleaseTag {
    match s.parse::<i32>() {
        Ok(n) => PrereleaseTag::Int(n),
        Err(_) => PrereleaseTag::String(s)
    }
}

fn parse_prerelease_tags(s: &str) -> Vec<PrereleaseTag> {
    // TODO: split on .
    todo!()
}

fn parse_build_tags(s: &str) -> Vec<String> {
    // TODO: split on .
    todo!()
}

pub fn parse_semver(v_str: &str) -> Result<Semver, ParseSemverError> {
    let RE = regex!(r"^v?(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-((?:0|[1-9]\d*|\d*[a-zA-Z-][a-zA-Z0-9-]*)(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][a-zA-Z0-9-]*))*))?(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$");

    // let v_str = x.trim();
    let m = RE.captures_iter(v_str.trim()).next().unwrap(); // why next?

    let m_1 = m.get(1).unwrap().as_str();
    let m_2 = m.get(2).unwrap().as_str();
    let m_3 = m.get(3).unwrap().as_str();

    let m_1: i32 = m_1.parse()?;
    let m_2: i32 = m_2.parse()?;
    let m_3: i32 = m_3.parse()?;

    let m_4 = m.get(4).map(|x| parse_prerelease_tags(x.as_str())).unwrap_or_default();
    let m_5 = m.get(5).map(|x| parse_build_tags(x.as_str())).unwrap_or_default();

    Ok(Semver {
        major: m_1,
        minor: m_2,
        bug: m_3,
        prerelease: m_4,
        build: m_5
    })
}


#[derive(Debug)]
pub enum ParseSpecError {
    UnknownType(String),
    Other
}

pub fn parse_spec(s: &str) -> Result<ParsedSpec, ParseSpecError> {
    todo!()
}
