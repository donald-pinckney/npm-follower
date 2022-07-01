use std::str::FromStr;
use chrono::{DateTime, Utc};
use serde_json::{Map, Value};
use std::collections::HashMap;

use postgres_db::custom_types::Semver;
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
        (p, serde_json::from_value::<String>(c).unwrap().parse().unwrap())
    ).collect();

    let dev_dependencies = dev_dependencies_raw.into_iter().map(|(p, c)|
        (p, serde_json::from_value::<String>(c).unwrap().parse().unwrap())
    ).collect();

    let peer_dependencies = peer_dependencies_raw.into_iter().map(|(p, c)|
        (p, serde_json::from_value::<String>(c).unwrap().parse().unwrap())
    ).collect();

    let optional_dependencies = optional_dependencies_raw.into_iter().map(|(p, c)|
        (p, serde_json::from_value::<String>(c).unwrap().parse().unwrap())
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

    let repository_blob = version_blob.remove_key_unwrap_type::<Value>("repository");

    VersionPackument {
        prod_dependencies,
        dev_dependencies,
        peer_dependencies,
        optional_dependencies,
        dist,
        description,
        repository: repository_blob,
        extra_metadata: version_blob.into_iter().collect()
    }
}


fn deserialize_times_normal(j: &mut Map<String, Value>) -> (DateTime<Utc>, DateTime<Utc>, HashMap<Semver, DateTime<Utc>>) {
    let time_raw = j.remove_key_unwrap_type::<Map<String, Value>>("time").unwrap();
    let mut times: HashMap<_, _> = time_raw.into_iter().flat_map(|(k, t_str)| 
        Some((k, parse_datetime(serde_json::from_value::<String>(t_str).unwrap())))
    ).collect();
    let created = times.remove("created").unwrap();
    let modified = times.remove("modified").unwrap();

    let version_times: HashMap<Semver, _> = times.into_iter().map(|(v_str, t)| 
        (semver_spec_serialization::parse_semver(&v_str).unwrap(), t)
    ).collect();

    return (created, modified, version_times)
}

fn deserialize_times_ctime(j: &mut Map<String, Value>) -> (DateTime<Utc>, DateTime<Utc>, HashMap<Semver, DateTime<Utc>>) {
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
        let v_created_raw = v_obj.remove_key_unwrap_type::<String>("ctime").unwrap();
        let v_modified_raw = v_obj.remove_key_unwrap_type::<String>("mtime").unwrap();
        // If these aren't the same, we need to figure out which to choose
        assert!(v_created_raw == v_modified_raw);
        let v_time = parse_datetime(v_created_raw);

        let semver = semver_spec_serialization::parse_semver(v_key).unwrap();
        version_times.insert(semver, v_time);
    }

    (created, modified, version_times)
}


fn deserialize_times_missing_fake_it(j: &Map<String, Value>) -> (DateTime<Utc>, DateTime<Utc>, HashMap<Semver, DateTime<Utc>>) {
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
        version_times.insert(semver, fake_time);
    }

    (fake_time, fake_time, version_times)
}


fn deserialize_times(j: &mut Map<String, Value>) -> (DateTime<Utc>, DateTime<Utc>, HashMap<Semver, DateTime<Utc>>) {
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

pub fn deserialize_packument_blob_normal(mut j: Map<String, Value>) -> Packument {
    
    // TODO: remove useless optionals here
    let dist_tags_raw_maybe = Some(j.remove_key_unwrap_type::<Map<String, Value>>("dist-tags").unwrap());
    let mut dist_tags: Option<HashMap<String, Semver>> = dist_tags_raw_maybe.map(|dist_tags_raw| 
        dist_tags_raw.into_iter().map(|(tag, v_str)| 
            (tag, semver_spec_serialization::parse_semver(&serde_json::from_value::<String>(v_str).unwrap()).unwrap())
        ).collect()
    );
    
    let latest = match &mut dist_tags {
        Some(dt) => dt.remove("latest"),
        None => None
    };

    let (created, modified, version_times) = deserialize_times(&mut j);

    let version_packuments_map = j.remove_key_unwrap_type::<Map<String, Value>>("versions").unwrap();
    let version_packuments = version_packuments_map.into_iter().map(|(v_str, blob)|
        (
            semver_spec_serialization::parse_semver(&v_str).unwrap(), 
            deserialize_version_blob(serde_json::from_value::<Map<String, Value>>(blob).unwrap())
        )
    ).collect();
    Packument::Normal {
        latest: latest,
        created: created,
        modified: modified,
        other_dist_tags: dist_tags.unwrap(),
        version_times: version_times,
        versions: version_packuments
    }
}


pub fn deserialize_packument_blob_unpublished(mut j: Map<String, Value>) -> Packument {

    if j.contains_key("dist-tags") {
        panic!("Unpublished package shouldn't contain key: dist-tags");
    }



    let mut time_raw = j.remove_key_unwrap_type::<Map<String, Value>>("time").unwrap();
    let unpublished_blob = time_raw.remove_key_unwrap_type::<Value>("unpublished").unwrap();
    
    let mut times: HashMap<_, _> = time_raw.into_iter().flat_map(|(k, t_str)| 
        Some((k, parse_datetime(serde_json::from_value::<String>(t_str).unwrap())))
    ).collect();
    let modified = times.remove("modified").unwrap();
    let created = times.remove("created").unwrap();

    let extra_version_times: HashMap<Semver, _> = times.into_iter().map(|(v_str, t)| 
        (semver_spec_serialization::parse_semver(&v_str).unwrap(), t)
    ).collect();

    if j.contains_key("versions") {
        panic!("Unpublished package shouldn't contain key: versions");
    }

    Packument::Unpublished {
        created,
        modified,
        unpublished_blob,
        extra_version_times
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