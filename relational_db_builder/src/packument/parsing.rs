use chrono::{DateTime, Utc};
use serde_json::Value;
use std::collections::HashMap;

use postgres_db::custom_types::Semver;
use super::{Packument, VersionPackument, PackumentDependencies};

fn process_version(mut version_blob: serde_json::Map<String, Value>) -> Option<VersionPackument> {
    // version_blob.remove("name");
    // version_blob.remove("version");
    let description = version_blob.remove("description").and_then(|x| unwrap_string(x).ok());
    let prod_dependencies_raw = unwrap_object(version_blob.remove("dependencies").unwrap_or(empty_object())).unwrap_or_default();
    let dev_dependencies_raw = unwrap_object(version_blob.remove("devDependencies").unwrap_or(empty_object())).unwrap_or_default();
    let peer_dependencies_raw = unwrap_object(version_blob.remove("peerDependencies").unwrap_or(empty_object())).unwrap_or_default();
    let optional_dependencies_raw = unwrap_object(version_blob.remove("optionalDependencies").unwrap_or(empty_object())).unwrap_or_default();
    let mut dist = unwrap_object(version_blob.remove("dist").unwrap()).unwrap();
    let shasum = unwrap_string(dist.remove("shasum")?).expect(&format!("Expected a string. Blob = {:?}", version_blob));
    let tarball = unwrap_string(dist.remove("tarball").unwrap()).unwrap();

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
    


    Some(VersionPackument {
        description: description,
        shasum: shasum,
        tarball: tarball,
        dependencies: PackumentDependencies {
            prod_dependencies: prod_dependencies,
            dev_dependencies: dev_dependencies,
            peer_dependencies: peer_dependencies,
            optional_dependencies: optional_dependencies,
        },
        extra_metadata: version_blob.into_iter().collect()
    })
}

fn process_packument_blob(v: Value, _pkg_name: String) -> Result<Packument, String> {
    let mut j = unwrap_object(v).unwrap();
    
    let dist_tags_raw_maybe = j.remove("dist-tags").map(|dt| unwrap_object(dt).unwrap());
    let mut dist_tags: Option<HashMap<String, Semver>> = dist_tags_raw_maybe.map(|dist_tags_raw| 
        dist_tags_raw.into_iter().map(|(tag, v_str)| 
            (tag, unwrap_string(v_str).unwrap().parse().unwrap())
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
        (v_str.clone().parse().unwrap(), t)
    ).collect();

    let version_packuments_map = j.remove("versions").map(|x| unwrap_object(x).unwrap()).unwrap_or_default(); //unwrap_object(j.remove("versions").unwrap());
    let version_packuments = version_packuments_map.into_iter().flat_map(|(v_str, blob)|
        Some((v_str.parse().unwrap(), process_version(unwrap_object(blob).unwrap())?))
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

fn empty_object() -> Value {
    Value::Object(serde_json::Map::new())
}

fn unwrap_array(v: Value) -> Vec<Value> {
    match v {
        Value::Array(a) => a,
        _ => panic!()
    }
}

fn unwrap_string(v: Value) -> Result<String, String> {
    match v {
        Value::String(s) => Ok(s),
        _ => Err(format!("Expected string, got: {:?}", v))
    }
}

fn unwrap_object(v: Value) -> Result<serde_json::Map<String, Value>, String> {
    match v {
        Value::Object(o) => Ok(o),
        _ => Err(format!("Expected object, got: {:?}", v))
    }
}

fn unwrap_number(v: Value) -> serde_json::Number {
    match v {
        Value::Number(n) => n,
        _ => panic!()
    }
}