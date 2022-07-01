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

    let time_raw = j.remove_key_unwrap_type::<Map<String, Value>>("time").unwrap();
    let mut times: HashMap<_, _> = time_raw.into_iter().flat_map(|(k, t_str)| 
        Some((k, parse_datetime(serde_json::from_value::<String>(t_str).unwrap())))
    ).collect();
    let modified = times.remove("modified").unwrap();
    let created = times.remove("created").unwrap();

    let version_times: HashMap<Semver, _> = times.into_iter().map(|(v_str, t)| 
        (semver_spec_serialization::parse_semver(&v_str).unwrap(), t)
    ).collect();

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