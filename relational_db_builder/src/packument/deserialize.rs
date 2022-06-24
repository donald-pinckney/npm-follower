use std::str::FromStr;
use chrono::{DateTime, Utc};
use serde_json::{Map, Value};
use std::collections::HashMap;

use postgres_db::custom_types::{Semver, Repository};
use semver_spec_parser;
use super::{Packument, VersionPackument, Dist, Spec};

use utils::RemoveInto;

fn deserialize_version_blob(mut version_blob: Map<String, Value>) -> VersionPackument {
    let description = version_blob.remove_key_unwrap_type::<String>("description");
    let prod_dependencies_raw = version_blob.remove_key_unwrap_type::<Map<String, Value>>("dependencies").unwrap_or_default();
    let dev_dependencies_raw = version_blob.remove_key_unwrap_type::<Map<String, Value>>("devDependencies").unwrap_or_default();
    let peer_dependencies_raw = version_blob.remove_key_unwrap_type::<Map<String, Value>>("peerDependencies").unwrap_or_default();
    let optional_dependencies_raw = version_blob.remove_key_unwrap_type::<Map<String, Value>>("optionalDependencies").unwrap_or_default();

    let mut dist = version_blob.remove_key_unwrap_type::<Map<String, Value>>("dist").unwrap();

    let sigs_maybe = version_blob.remove_key_unwrap_type::<Vec<Value>>("signatures");
    let sig0: Option<Map<String, Value>> = sigs_maybe.map(|mut sigs| 
        serde_json::from_value(sigs.remove(0)).unwrap()
    );
    let sig0_sig_keyid = sig0.map(|mut s| 
        (s.remove_key_unwrap_type::<String>("sig").unwrap(), s.remove_key_unwrap_type::<String>("keyid").unwrap())
    );
    let (sig0_sig, sig0_keyid) = match sig0_sig_keyid {
        Some((x, y)) => (Some(x), Some(y)),
        None => (None, None)
    };


    
    let prod_dependencies = prod_dependencies_raw.into_iter().map(|(p, c)|
        (p, unwrap_string(c).unwrap().parse().unwrap())
    ).collect();

    let dev_dependencies = dev_dependencies_raw.into_iter().map(|(p, c)|
        (p, unwrap_string(c).unwrap().parse().unwrap())
    ).collect();

    let peer_dependencies = peer_dependencies_raw.into_iter().map(|(p, c)|
        (p, unwrap_string(c).unwrap().parse().unwrap())
    ).collect();

    let optional_dependencies = optional_dependencies_raw.into_iter().map(|(p, c)|
        (p, unwrap_string(c).unwrap().parse().unwrap())
    ).collect();
    
    let dist = Dist {
        tarball_url: dist.remove_key_unwrap_type::<String>("tarball").unwrap(),
        shasum: dist.remove_key_unwrap_type::<String>("shasum"),
        unpacked_size: dist.remove_key_unwrap_type::<i64>("unpackedSize"),
        file_count: dist.remove_key_unwrap_type::<i64>("fileCount").map(|x| x.try_into().unwrap()),
        integrity: dist.remove_key_unwrap_type::<String>("integrity"),
        signature0_sig: sig0_sig,
        signature0_keyid: sig0_keyid,
        npm_signature: dist.remove_key_unwrap_type::<String>("npm-signature"),
    };

    let repository_map = version_blob.remove_key_unwrap_type::<Map<String, Value>>("repository");
    let repository = repository_map.map(|mut m| { 
        let t = m.remove_key_unwrap_type::<String>("type").unwrap();
        match t.as_str() {
            "git" => {
                Repository::Git(m.remove_key_unwrap_type::<String>("url").unwrap())
            },
            _ => {
                panic!("Unknown repository type: {}", t)
            }
        }
    });

    VersionPackument {
        prod_dependencies,
        dev_dependencies,
        peer_dependencies,
        optional_dependencies,
        dist,
        description,
        repository,
        extra_metadata: version_blob.into_iter().collect()
    }
}

pub fn deserialize_packument_blob(mut j: Map<String, Value>) -> Result<Packument, String> {
    
    let dist_tags_raw_maybe = j.remove("dist-tags").map(|dt| unwrap_object(dt).unwrap());
    let mut dist_tags: Option<HashMap<String, Semver>> = dist_tags_raw_maybe.map(|dist_tags_raw| 
        dist_tags_raw.into_iter().map(|(tag, v_str)| 
            (tag, semver_spec_parser::parse_semver(&unwrap_string(v_str).unwrap()).unwrap())
        ).collect()
    );
    
    let latest = match &mut dist_tags {
        Some(dt) => dt.remove("latest"),
        None => None
    };

    let time_raw = unwrap_object(j.remove("time").ok_or(format!("Expected time field: {:#?}", j))?).unwrap();
    // let time_raw = unwrap_object(j.remove("time").expect(&format!("Expected time field: {:#?}, pkg_name = {}", j, _pkg_name)));
    let mut times: HashMap<_, _> = time_raw.into_iter().flat_map(|(k, t_str)| 
        Some((k, parse_datetime(unwrap_string(t_str).ok()?)))
    ).collect();
    let modified = times.remove("modified").unwrap();
    let created = times.remove("created").unwrap();

    let version_times: HashMap<Semver, _> = times.into_iter().map(|(v_str, t)| 
        (semver_spec_parser::parse_semver(&v_str.clone()).unwrap(), t)
    ).collect();

    let version_packuments_map = j.remove("versions").map(|x| unwrap_object(x).unwrap()).unwrap_or_default(); //unwrap_object(j.remove("versions").unwrap());
    let version_packuments = version_packuments_map.into_iter().map(|(v_str, blob)|
        (semver_spec_parser::parse_semver(&v_str).unwrap(), deserialize_version_blob(unwrap_object(blob).unwrap()))
    ).collect();
    Ok(Packument {
        latest: latest,
        created: created,
        modified: modified,
        version_times: version_times,
        other_dist_tags: dist_tags,
        versions: version_packuments
    })
}


fn parse_datetime(x: String) -> DateTime<Utc> {
    let dt = DateTime::parse_from_rfc3339(&x)
        .or_else(|_| DateTime::parse_from_rfc3339(&(x + "Z"))).unwrap();
    dt.with_timezone(&Utc)
}

fn unwrap_string(v: Value) -> Result<String, String> {
    match v {
        Value::String(s) => Ok(s),
        _ => Err(format!("Expected string, got: {:?}", v))
    }
}

fn unwrap_object(v: Value) -> Result<Map<String, Value>, String> {
    match v {
        Value::Object(o) => Ok(o),
        _ => Err(format!("Expected object, got: {:?}", v))
    }
}



impl FromStr for Spec {
    type Err = semver_spec_parser::ParseSpecError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        todo!()
    }
}