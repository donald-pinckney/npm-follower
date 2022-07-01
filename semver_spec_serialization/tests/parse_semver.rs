use postgres_db::custom_types::{Semver, PrereleaseTag};

use test_case::test_case;

use semver_spec_serialization::parse_semver;


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



fn semver_simple(major: i64, minor: i64, bug: i64) -> Semver {
    Semver { major, minor, bug, prerelease: vec![], build: vec![] }
}

fn semver(major: i64, minor: i64, bug: i64, prerelease: Vec<PrereleaseTag>, build: Vec<String>) -> Semver {
    Semver { major, minor, bug, prerelease, build }
}