#[macro_use]
extern crate quickcheck;
#[macro_use]
extern crate lazy_static;

use postgres_db::custom_types::{Semver, PrereleaseTag, ParsedSpec, VersionConstraint, VersionComparator};
use semver_spec_serialization::{parse_spec_via_node, parse_spec_via_rust};


lazy_static! {
    static ref SUCCESS_CASES: Vec<(&'static str, ParsedSpec)> = vec![
        ("1.2.3", ParsedSpec::Range(VersionConstraint(vec![vec![VersionComparator::Eq(semver_simple(1, 2, 3))]])))
    ];

    static ref FAILURE_CASES: Vec<&'static str> = vec![
        "ht://stuff.cat"
    ];
}


#[test]
fn test_parse_spec_via_node_success_cases() {
    for (input, answer) in SUCCESS_CASES.iter() {
        println!("testing {}", input);
        assert_eq!(parse_spec_via_node(input).unwrap(), *answer)
    }
}

#[test]
fn test_parse_spec_via_node_failure_cases() {
    for input in FAILURE_CASES.iter() {
        println!("testing {}", input);
        assert_eq!(parse_spec_via_node(input).is_err(), true)
    }
}




fn equivalent_results<T, E>(x: Result<T, E>, y: Result<T, E>) -> bool where T: PartialEq {
    match (x, y) {
        (Ok(xr), Ok(yr)) => xr == yr,
        (Err(_), Err(_)) => true,
        _ => false
    }
}

fn node_rust_same_result(s: String) -> bool {
    let node_result = parse_spec_via_node(&s);
    let rust_result = parse_spec_via_rust(&s);
    equivalent_results(node_result, rust_result)
}


#[test]
fn test_parse_spec_node_rust_equivalent_success_cases() {
    for (input, _) in SUCCESS_CASES.iter() {
        println!("testing {}", input);
        assert!(node_rust_same_result(input.to_string()));
    }
}

#[test]
fn test_parse_spec_node_rust_equivalent_failure_cases() {
    for input in FAILURE_CASES.iter() {
        println!("testing {}", input);
        assert!(node_rust_same_result(input.to_string()));
    }
}


quickcheck! {
    fn test_node_rust_same_result(s: String) -> bool {
        node_rust_same_result(s)
    }
}



fn semver_simple(major: i32, minor: i32, bug: i32) -> Semver {
    Semver { major, minor, bug, prerelease: vec![], build: vec![] }
}

fn semver(major: i32, minor: i32, bug: i32, prerelease: Vec<PrereleaseTag>, build: Vec<String>) -> Semver {
    Semver { major, minor, bug, prerelease, build }
}