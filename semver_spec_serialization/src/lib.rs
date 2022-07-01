use std::string::FromUtf8Error;
use std::num::ParseIntError;

use cached::proc_macro::cached;

use lazy_regex::regex;

use postgres_db::custom_types::{Semver, ParsedSpec, PrereleaseTag};

#[derive(Debug)]
pub enum ParseSemverError {
    MajorMinorBugParseIntError(ParseIntError),
    RegexMatchFailed,
}

impl From<ParseIntError> for ParseSemverError {
    fn from(err: ParseIntError) -> Self {
        Self::MajorMinorBugParseIntError(err)
    }
}

fn parse_prerelease_tag(s: &str) -> PrereleaseTag {
    match s.parse::<i64>() {
        Ok(n) => PrereleaseTag::Int(n),
        Err(_) => PrereleaseTag::String(s.to_owned())
    }
}

fn parse_prerelease_tags(s: &str) -> Vec<PrereleaseTag> {
    s.split(".").map(|t| parse_prerelease_tag(t)).collect()
}

fn parse_build_tags(s: &str) -> Vec<String> {
    s.split(".").map(|t| t.to_owned()).collect()
}

pub fn parse_semver(v_str: &str) -> Result<Semver, ParseSemverError> {
    // Modified from: https://github.com/npm/node-semver/blob/main/internal/re.js
    // console.log(semver.src[semver.tokens['LOOSE']])
    let re = regex!(r"^[v=\s]*([0-9]+)\.([0-9]+)\.([0-9]+)(?:-?((?:[0-9]+|\d*[a-zA-Z-][a-zA-Z0-9-]*)(?:\.(?:[0-9]+|\d*[a-zA-Z-][a-zA-Z0-9-]*))*))?(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$");

    let m = re.captures_iter(v_str.trim()).next().ok_or(ParseSemverError::RegexMatchFailed)?;

    let m_1 = m.get(1).unwrap().as_str();
    let m_2 = m.get(2).unwrap().as_str();
    let m_3 = m.get(3).unwrap().as_str();

    let m_1: i64 = m_1.parse()?;
    let m_2: i64 = m_2.parse()?;
    let m_3: i64 = m_3.parse()?;

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
    Other(String),
    Encoding(FromUtf8Error),
    JsonParsing(serde_json::Error),
    IO(std::io::Error)
}

impl From<serde_json::Error> for ParseSpecError {
    fn from(err: serde_json::Error) -> Self {
        Self::JsonParsing(err)
    }
}

impl From<FromUtf8Error> for ParseSpecError {
    fn from(err: FromUtf8Error) -> Self {
        Self::Encoding(err)
    }
}

impl From<std::io::Error> for ParseSpecError {
    fn from(err: std::io::Error) -> Self {
        Self::IO(err)
    }
}



pub fn parse_spec_via_node(s: &str) -> Result<ParsedSpec, ParseSpecError> {
    use std::process::Command;

    let mut js_dir = std::env::current_dir()?;
    if js_dir.ends_with("semver_spec_serialization/") {
        js_dir.push("js_parser");
    } else if js_dir.ends_with("relational_db_builder/") {
        js_dir.push("..");
        js_dir.push("semver_spec_serialization");
        js_dir.push("js_parser");
    } else {
        js_dir.push("semver_spec_serialization");
        js_dir.push("js_parser");
    }

    let output = Command::new("node")
                                 .arg(js_dir)
                                 .arg(s)
                                 .output()?;

    if !output.status.success() {
        return Err(ParseSpecError::Other(format!("stdout:\n{}\n\nstderr:\n{}", String::from_utf8(output.stdout)?, String::from_utf8(output.stderr)?)));
    }

    let parsed: ParsedSpec = serde_json::from_slice(&output.stdout)?;
    Ok(parsed)
}


#[cached(size=500_000, result = true, key = "String", convert = r#"{ String::from(s) }"#)]
pub fn parse_spec_via_node_cached(s: &str) -> Result<ParsedSpec, ParseSpecError> {
    parse_spec_via_node(s)
}



pub fn parse_spec_via_rust(s: &str) -> Result<ParsedSpec, ParseSpecError> {
    Err(ParseSpecError::Other("todo".into()))
}

