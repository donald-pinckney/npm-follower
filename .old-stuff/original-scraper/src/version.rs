use std::fmt;
use lazy_static::lazy_static;
use regex::Regex;

#[derive(PartialEq, Eq, Hash, Debug)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub bug: u64,
    pub prerelease: Option<String>,
    pub build: Option<String>
}

impl fmt::Display for Version {
    
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
        write!(f, "{}.{}.{}", self.major, self.minor, self.bug)?;
        if let Some(pre) = &self.prerelease {
            write!(f, "-{}", pre)?;
        }
        if let Some(b) = &self.build {
            write!(f, "+{}", b)?;
        }
        Ok(())
    }
}

impl Version {
    pub fn parse(v_str: String) -> Option<Version> {
        lazy_static! {
            static ref RE: Regex = Regex::new(r"^v?(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-((?:0|[1-9]\d*|\d*[a-zA-Z-][a-zA-Z0-9-]*)(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][a-zA-Z0-9-]*))*))?(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$").unwrap();
        }

        // let v_str = x.trim();
        let m = RE.captures_iter(v_str.trim()).next()?;

        let m_1 = m.get(1).unwrap().as_str();
        let m_2 = m.get(2).unwrap().as_str();
        let m_3 = m.get(3).unwrap().as_str();

        let m_1: u64 = m_1.parse().ok()?;//.expect(&format!("bad number: {}", m_1));
        let m_2: u64 = m_2.parse().ok()?;//.expect(&format!("bad number: {}", m_2));
        let m_3: u64 = m_3.parse().ok()?;//.expect(&format!("bad number: {}", m_3));

        let m_4 = m.get(4).map(|x| x.as_str().to_owned());
        let m_5 = m.get(5).map(|x| x.as_str().to_owned());

        Some(Version {
            major: m_1,
            minor: m_2,
            bug: m_3,
            prerelease: m_4,
            build: m_5
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        assert_eq!(Version::parse("3.23.12".to_string()), Version { major: 3, minor: 23, bug: 12, prerelease: None, build: None });
    }
}