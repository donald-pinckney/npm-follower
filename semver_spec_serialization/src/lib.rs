#[macro_use]
extern crate lazy_static;

use std::io::{BufRead, BufReader, Read, Write};
use std::string::FromUtf8Error;
use std::{num::ParseIntError, os::unix::net::UnixStream};

use cached::proc_macro::cached;

use lazy_regex::regex;

use postgres_db::custom_types::{ParsedSpec, PrereleaseTag, Semver};

lazy_static! {
    static ref SOCK_PATH: String = {
        let tmpdir = std::env::temp_dir();
        format!(
            "{}/specsrv-{}.sock",
            tmpdir.to_str().unwrap(),
            std::process::id()
        )
    };
    static ref SPEC_PROC_CHILD: std::process::Child = {
        use std::process::Command;
        use std::process::Stdio;

        let mut js_dir = std::env::current_dir().unwrap();
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

        Command::new("node")
            .arg(js_dir)
            .arg(SOCK_PATH.to_string())
            .arg(std::process::id().to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .spawn()
            .expect("Couldn't spawn spec parsing daemon")
    };
}

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
    match s.parse::<i32>() {
        Ok(n) => PrereleaseTag::Int(n),
        Err(_) => PrereleaseTag::String(s.to_owned()),
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
    let re = regex!(
        r"^v?(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-((?:0|[1-9]\d*|\d*[a-zA-Z-][a-zA-Z0-9-]*)(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][a-zA-Z0-9-]*))*))?(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$"
    );

    let m = re
        .captures_iter(v_str.trim())
        .next()
        .ok_or(ParseSemverError::RegexMatchFailed)?;

    let m_1 = m.get(1).unwrap().as_str();
    let m_2 = m.get(2).unwrap().as_str();
    let m_3 = m.get(3).unwrap().as_str();

    let m_1: i32 = m_1.parse()?;
    let m_2: i32 = m_2.parse()?;
    let m_3: i32 = m_3.parse()?;

    let m_4 = m
        .get(4)
        .map(|x| parse_prerelease_tags(x.as_str()))
        .unwrap_or_default();
    let m_5 = m
        .get(5)
        .map(|x| parse_build_tags(x.as_str()))
        .unwrap_or_default();

    Ok(Semver {
        major: m_1,
        minor: m_2,
        bug: m_3,
        prerelease: m_4,
        build: m_5,
    })
}

#[derive(Debug)]
pub enum ParseSpecError {
    UnknownType(String),
    Other(String),
    Encoding(FromUtf8Error),
    JsonParsing(serde_json::Error),
    IO(std::io::Error),
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
    // edge case for empty string, which cannot be transmitted via socket
    if s.is_empty() {
        return Ok(ParsedSpec::Tag("latest".to_string()));
    }

    // hacky way to execute lazy code to spawn the daemon
    let _specchild = SPEC_PROC_CHILD.id();

    let stream_res = UnixStream::connect(SOCK_PATH.to_string());
    let mut stream = match stream_res {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // sleep for a bit to let the daemon start up. if this was a real error, it will throw
            // again. although, this error should never happen in practice.
            std::thread::sleep(std::time::Duration::from_millis(200));
            UnixStream::connect(SOCK_PATH.to_string())
        }
        _ => stream_res,
    }?;
    stream.write_all(s.as_bytes())?;
    let mut res = String::new();
    let mut reader = BufReader::new(stream);
    reader.read_line(&mut res).unwrap();
    let parsed: ParsedSpec = serde_json::from_str(&res)?;

    Ok(parsed)
}

#[cached(
    size = 500_000,
    result = true,
    key = "String",
    convert = r#"{ String::from(s) }"#
)]
pub fn parse_spec_via_node_cached(s: &str) -> Result<ParsedSpec, ParseSpecError> {
    parse_spec_via_node(s)
}
