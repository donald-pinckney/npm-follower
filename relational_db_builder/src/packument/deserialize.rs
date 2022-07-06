use std::str::FromStr;
use chrono::{DateTime, Utc};
use serde_json::{Map, Value};
use std::collections::HashMap;

use postgres_db::custom_types::Semver;
use super::{Packument, VersionPackument, Dist, Spec};

use utils::{RemoveInto, FilterJsonCases};

fn deserialize_spec(c: Value) -> Spec {
    match c {
        Value::String(spec_str) => spec_str.parse().unwrap(),
        _ => {
            let err = format!("spec must be a string, received: {}", c);
            Spec { raw: c, parsed: postgres_db::custom_types::ParsedSpec::Invalid(err) }
        }
    }
}

fn deserialize_dependencies(version_blob: &mut Map<String, Value>, key: &'static str) -> Vec<(String, Spec)> {
    let dependencies_maybe_val = version_blob.remove(key)
                                                            .and_then(|x| x.null_to_none())
                                                            .and_then(|x| x.empty_array_to_none());

    if let Some(dependencies_val) = dependencies_maybe_val {
        match dependencies_val {
            Value::Array(xs) if xs.len() > 0 => {
                version_blob.insert(key.to_string(), Value::Array(xs));
                vec![]
            },
            Value::String(s) => {
                version_blob.insert(key.to_string(), Value::String(s));
                vec![]
            },
            Value::Object(dependencies_raw) => {
                dependencies_raw.into_iter().map(|(p, c)| {
                    (p, deserialize_spec(c))
                }).collect()
            },
            Value::Bool(_) | Value::Number(_) => {
                panic!("Invalid dependencies");
            },
            Value::Null | Value::Array(_) => unreachable!()
        }
    } else {
        vec![]
    }
}

fn deserialize_version_blob(mut version_blob: Map<String, Value>) -> VersionPackument {
    let prod_dependencies = deserialize_dependencies(&mut version_blob, "dependencies");
    let dev_dependencies = deserialize_dependencies(&mut version_blob, "devDependencies");
    let peer_dependencies = deserialize_dependencies(&mut version_blob, "peerDependencies");
    let optional_dependencies = deserialize_dependencies(&mut version_blob, "optionalDependencies");

    let mut dist = version_blob.remove_key_unwrap_type::<Map<String, Value>>("dist").unwrap();

    let sigs_maybe = dist.remove_key_unwrap_type::<Vec<Value>>("signatures");
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

    let repository_blob = version_blob.remove("repository")
                                                      .and_then(|x| x.null_to_none())
                                                      .map(|x| serde_json::from_value::<Value>(x).unwrap());
    
    VersionPackument {
        prod_dependencies,
        dev_dependencies,
        peer_dependencies,
        optional_dependencies,
        dist,
        repository: repository_blob,
        extra_metadata: version_blob.into_iter().collect()
    }
}


fn deserialize_times_normal(j: &mut Map<String, Value>) -> (DateTime<Utc>, DateTime<Utc>, HashMap<Result<Semver, String>, DateTime<Utc>>) {
    let time_raw = j.remove_key_unwrap_type::<Map<String, Value>>("time").unwrap();
    let mut times: HashMap<_, _> = time_raw.into_iter().flat_map(|(k, t_str)| 
        Some((k, parse_datetime(serde_json::from_value::<String>(t_str).unwrap())))
    ).collect();
    let created = times.remove("created").unwrap();
    let modified = times.remove("modified").unwrap();

    let version_times: HashMap<Result<Semver, String>, _> = times.into_iter().map(|(v_str, t)| 
        (semver_spec_serialization::parse_semver(&v_str).map_err(|_err| {
            v_str
        }), t)
    ).collect();

    return (created, modified, version_times)
}

fn deserialize_times_ctime(j: &mut Map<String, Value>) -> (DateTime<Utc>, DateTime<Utc>, HashMap<Result<Semver, String>, DateTime<Utc>>) {
    let created_raw = j.remove_key_unwrap_type::<String>("ctime").unwrap();
    let modified_raw = j.remove_key_unwrap_type::<String>("mtime").unwrap();

    let created = parse_datetime(created_raw);
    let modified = parse_datetime(modified_raw);

    let mut version_times = HashMap::new();

    let versions_map = j.get_mut("versions")
                                                 .unwrap()
                                                 .as_object_mut()
                                                 .unwrap();
    for (v_key, v_blob) in versions_map.iter_mut() {
        let v_obj = v_blob.as_object_mut().unwrap();

        let v_created_raw_maybe = v_obj.remove_key_unwrap_type::<String>("ctime");
        let v_modified_raw_maybe = v_obj.remove_key_unwrap_type::<String>("mtime");

        let v_time = match (v_created_raw_maybe, v_modified_raw_maybe) {
            (Some(v_created_raw), Some(v_modified_raw)) => {
                assert!(v_created_raw == v_modified_raw);
                parse_datetime(v_created_raw)
            },
            (None, None) => {
                let fake_time = parse_datetime("2015-01-01T00:00:00.000Z".to_string());
                fake_time
            },
            _ => panic!("Unknown ctime / mtime combination.")
        };

        let semver = semver_spec_serialization::parse_semver(v_key).unwrap();
        version_times.insert(Ok(semver), v_time);
    }

    (created, modified, version_times)
}


fn deserialize_times_missing_fake_it(j: &Map<String, Value>) -> (DateTime<Utc>, DateTime<Utc>, HashMap<Result<Semver, String>, DateTime<Utc>>) {
    let fake_time = parse_datetime("2015-01-01T00:00:00.000Z".to_string());

    let mut version_times = HashMap::new();

    let versions_map = j.get("versions")
                                             .unwrap()
                                             .as_object()
                                             .unwrap();
    for (v_key, v_blob) in versions_map.iter() {
        let v_obj = v_blob.as_object().unwrap();
        assert!(!v_obj.contains_key("time"));
        assert!(!v_obj.contains_key("ctime"));
        assert!(!v_obj.contains_key("mtime"));

        let semver = semver_spec_serialization::parse_semver(v_key).unwrap();
        version_times.insert(Ok(semver), fake_time);
    }

    (fake_time, fake_time, version_times)
}


fn deserialize_times(j: &mut Map<String, Value>) -> (DateTime<Utc>, DateTime<Utc>, HashMap<Result<Semver, String>, DateTime<Utc>>) {
    if j.contains_key("time") {
        assert!(!j.contains_key("ctime"));
        assert!(!j.contains_key("mtime"));

        deserialize_times_normal(j)
    } else if j.contains_key("ctime") {
        assert!(j.contains_key("mtime"));
        assert!(!j.contains_key("time"));
        
        deserialize_times_ctime(j)
    } else {
        deserialize_times_missing_fake_it(j)
    }
}

fn only_keep_ok_version_times(version_times: HashMap<Result<Semver, String>, DateTime<Utc>>) -> HashMap<Semver, DateTime<Utc>> {
    version_times
        .into_iter()
        .filter_map(|(vr, t)| vr.ok().map(|v| (v, t))).collect()
}

fn deserialize_latest_tag(dist_tags: &mut Map<String, Value>) -> Option<Semver> {
    dist_tags.remove_key_unwrap_type::<String>("latest")
             .and_then(|latest_str| {
                match semver_spec_serialization::parse_semver(&latest_str) {
                    Ok(v) => Some(v),
                    Err(_) => {
                        dist_tags.insert("latest".to_string(), Value::String(latest_str));
                        None
                    }
                }
            })
}

pub fn deserialize_packument_blob_normal(mut j: Map<String, Value>) -> Packument {
    
    let mut dist_tags = j.remove_key_unwrap_type::<Map<String, Value>>("dist-tags").unwrap();
    let latest_semver = deserialize_latest_tag(&mut dist_tags);
    
    let (created, modified, version_times) = deserialize_times(&mut j);

    let version_packuments_map = j.remove_key_unwrap_type::<Map<String, Value>>("versions").unwrap();
    let version_packuments = version_packuments_map.into_iter().map(|(v_str, blob)|
        (
            semver_spec_serialization::parse_semver(&v_str).unwrap(), 
            deserialize_version_blob(serde_json::from_value::<Map<String, Value>>(blob).unwrap())
        )
    ).collect();

    

    Packument::Normal {
        latest: latest_semver,
        created: created,
        modified: modified,
        other_dist_tags: dist_tags,
        version_times: only_keep_ok_version_times(version_times),
        versions: version_packuments
    }
}


pub fn deserialize_packument_blob_unpublished(mut j: Map<String, Value>) -> Packument {

    if j.contains_key("dist-tags") {
        panic!("Unpublished package shouldn't contain key: dist-tags");
    }

    if j.contains_key("versions") {
        panic!("Unpublished package shouldn't contain key: versions");
    }

    // Note that we have to remove the "unpublished" key from the times data, otherwise deserialize_times would fail to parse it.
    let unpublished_blob = j.get_mut("time").unwrap().as_object_mut().unwrap().remove_key_unwrap_type::<Value>("unpublished").unwrap();
    let (created, modified, extra_version_times) = deserialize_times(&mut j);

    Packument::Unpublished {
        created,
        modified,
        unpublished_blob,
        extra_version_times: only_keep_ok_version_times(extra_version_times)
    }
}




fn parse_datetime(x: String) -> DateTime<Utc> {
    let dt = DateTime::parse_from_rfc3339(&x)
        .or_else(|_| DateTime::parse_from_rfc3339(&(x + "Z"))).unwrap();
    dt.with_timezone(&Utc)
}


impl FromStr for Spec {
    type Err = semver_spec_serialization::ParseSpecError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Spec {
            raw: s.into(),
            parsed: semver_spec_serialization::parse_spec_via_node_cached(s)?
        })
    }
}