use postgres_db::custom_types::{Semver, PrereleaseTag, ParsedSpec, VersionConstraint, VersionComparator};

use test_case::test_case;

use semver_spec_serialization::parse_spec;


#[test_case("1.2.3", ParsedSpec::Range(VersionConstraint(vec![vec![VersionComparator::Eq(semver_simple(1, 2, 3))]])))]
fn test_parse_spec_success(spec_str: &str, answer: ParsedSpec) {
    assert_eq!(parse_spec(spec_str, false).unwrap(), answer)
}



fn semver_simple(major: i32, minor: i32, bug: i32) -> Semver {
    Semver { major, minor, bug, prerelease: vec![], build: vec![] }
}

fn semver(major: i32, minor: i32, bug: i32, prerelease: Vec<PrereleaseTag>, build: Vec<String>) -> Semver {
    Semver { major, minor, bug, prerelease, build }
}