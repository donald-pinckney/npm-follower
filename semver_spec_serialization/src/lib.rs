use std::string::FromUtf8Error;
use std::num::ParseIntError;

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
    match s.parse::<i32>() {
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
    let re = regex!(r"^v?(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-((?:0|[1-9]\d*|\d*[a-zA-Z-][a-zA-Z0-9-]*)(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][a-zA-Z0-9-]*))*))?(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$");

    let m = re.captures_iter(v_str.trim()).next().ok_or(ParseSemverError::RegexMatchFailed)?;

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




pub fn parse_spec(s: &str) -> Result<ParsedSpec, ParseSpecError> {
    use std::process::Command;

    let mut js_dir = std::env::current_dir()?;
    if js_dir.ends_with("semver_spec_serialization/") {
        js_dir.push("js_parser");
    } else {
        js_dir.push("semver_spec_serialization");
        js_dir.push("js_parser");
    }

    let output = Command::new("node")
                                 .arg(js_dir)
                                 .arg(s)
                                 .output()
                                 .expect("failed to execute parser subprocess");
    if !output.status.success() {
        return Err(ParseSpecError::Other(String::from_utf8(output.stdout)?));
    }

    let parsed: ParsedSpec = serde_json::from_slice(&output.stdout)?;

    Ok(parsed)
}



#[cfg(test)]
mod tests {
    use postgres_db::custom_types::{Semver, PrereleaseTag, ParsedSpec, VersionConstraint, VersionComparator};

    use super::{parse_semver, parse_spec};
    use test_case::test_case;

    #[test_case("1.2.3", semver_simple(1, 2, 3))]
    #[test_case("0.0.0", semver_simple(0, 0, 0))]
    #[test_case("83.12.0", semver_simple(83, 12, 0))]
    #[test_case("1.0.0", semver_simple(1, 0, 0))]
    #[test_case("0.2.0", semver_simple(0, 2, 0))]
    #[test_case("1.2.3-alpha1", semver(1, 2, 3, vec![PrereleaseTag::String("alpha1".into())], vec![]))]
    #[test_case("1.2.3-alpha1.6", semver(1, 2, 3, vec![PrereleaseTag::String("alpha1".into()), PrereleaseTag::Int(6)], vec![]))]
    #[test_case("1.2.3-7.alpha1", semver(1, 2, 3, vec![PrereleaseTag::Int(7), PrereleaseTag::String("alpha1".into())], vec![]))]
    #[test_case("1.2.3-7.alpha1+9.beta3", semver(1, 2, 3, vec![PrereleaseTag::Int(7), PrereleaseTag::String("alpha1".into())], vec!["9".into(), "beta3".into()]))]
    #[test_case("1.2.3+9.beta3", semver(1, 2, 3, vec![], vec!["9".into(), "beta3".into()]))]
    #[test_case("1.2.3+9.beta3-7.alpha1", semver(1, 2, 3, vec![], vec!["9".into(), "beta3-7".into(), "alpha1".into()]))]
    #[test_case("1.2.3+-", semver(1, 2, 3, vec![], vec!["-".into()]))]
    #[test_case("1.2.3-beta-1-2-3", semver(1, 2, 3, vec![PrereleaseTag::String("beta-1-2-3".into())], vec![]))]
    fn test_parse_semver_success(v_str: &str, answer: Semver) {
        assert_eq!(parse_semver(v_str).unwrap(), answer)
    }
    
    #[test_case("1.a.3")]
    #[test_case("1.3")]
    #[test_case("1.3.4.5")]
    #[test_case("1")]
    #[test_case("1.2..3")]
    #[test_case("-2.2.3")]
    #[test_case("2.-3.3")]
    #[test_case("3.2.-3")]
    #[test_case("4.2.3-")]
    #[test_case("4.2.6+")]
    #[test_case("+" ; "just plus")]
    #[test_case("-" ; "just minus")]
    #[test_case("" ; "empty str")]
    #[test_case("1.2.3-+" ; "bad prerelease")]
    #[test_case("1.2.3-." ; "bad prerelease2")]
    #[test_case("1.2.3+." ; "bad build")]
    fn test_parse_semver_err(v_str: &str) {
        assert!(parse_semver(v_str).is_err())
    }


    #[test_case("1.2.3", ParsedSpec::Range(VersionConstraint(vec![vec![VersionComparator::Eq(semver_simple(1, 2, 3))]])))]
    fn test_parse_spec_success(spec_str: &str, answer: ParsedSpec) {
        assert_eq!(parse_spec(spec_str).unwrap(), answer)
    }



    fn semver_simple(major: i32, minor: i32, bug: i32) -> Semver {
        Semver { major, minor, bug, prerelease: vec![], build: vec![] }
    }

    fn semver(major: i32, minor: i32, bug: i32, prerelease: Vec<PrereleaseTag>, build: Vec<String>) -> Semver {
        Semver { major, minor, bug, prerelease, build }
    }
}