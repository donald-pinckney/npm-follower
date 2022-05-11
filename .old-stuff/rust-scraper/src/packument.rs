use serde_json::Map;
use chrono::Utc;
use chrono::DateTime;
use crate::version::Version;
use serde_json::Value;
use std::collections::HashMap;
use std::collections::HashSet;
use crate::utils;


#[derive(Debug)]
pub struct Dependencies {
    // These map dependency package name to (dependency idx, constraint)
    pub prod_dependencies: HashMap<String, (u64, String)>,
    pub dev_dependencies: HashMap<String, (u64, String)>,
    pub peer_dependencies: HashMap<String, (u64, String)>,
    pub optional_dependencies: HashMap<String, (u64, String)>
}

#[derive(Debug)]
pub struct VersionPackument {
    pub description: Option<String>,
    pub shasum: String,
    pub tarball: String,
    pub dependencies: Dependencies,
    pub extra_metadata: HashMap<String, Value>
}

#[derive(Debug)]
pub struct Packument {
    pub latest: Option<Version>,
    pub created: DateTime<Utc>,
    pub modified: DateTime<Utc>,
    pub version_times: HashMap<Version, DateTime<Utc>>,
    pub versions: HashMap<Version, VersionPackument>,
    pub other_dist_tags: Option<HashMap<String, Version>>
}



pub fn process_packument_blob(mut j: Map<String, Value>) -> Result<Packument, String> {
    
    let dist_tags_raw_maybe = j.remove("dist-tags").map(|dt| utils::unwrap_object(dt).unwrap());
    let mut dist_tags: Option<HashMap<String, Version>> = dist_tags_raw_maybe.map(|dist_tags_raw| 
        dist_tags_raw.into_iter().map(|(tag, v_str)| 
            (tag, Version::parse(utils::unwrap_string(v_str).unwrap()).unwrap())
        ).collect()
    );
    
    let latest = match &mut dist_tags {
        Some(dt) => dt.remove("latest"),
        None => None
    };

    let time_raw = utils::unwrap_object(j.remove("time").ok_or(format!("Expected time field: {:#?}", j))?).unwrap();
    // let time_raw = unwrap_object(j.remove("time").expect(&format!("Expected time field: {:#?}, pkg_name = {}", j, _pkg_name)));
    let mut times: HashMap<_, _> = time_raw.into_iter().flat_map(|(k, t_str)| 
        Some((k, parse_datetime(utils::unwrap_string(t_str).ok()?)))
    ).collect();
    let modified = times.remove("modified").unwrap();
    let created = times.remove("created").unwrap();

    let version_times: HashMap<_, _> = times.into_iter().flat_map(|(v_str, t)| 
        Some((Version::parse(v_str.clone())?, t))
    ).collect();

    let version_packuments_map = j.remove("versions").map(|x| utils::unwrap_object(x).unwrap()).unwrap_or_default(); //unwrap_object(j.remove("versions").unwrap());
    let version_packuments = version_packuments_map.into_iter().flat_map(|(v_str, blob)|
        Some((Version::parse(v_str).unwrap(), process_version(utils::unwrap_object(blob).unwrap())?))
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



fn process_version(mut version_blob: serde_json::Map<String, Value>) -> Option<VersionPackument> {
    // version_blob.remove("name");
    // version_blob.remove("version");
    let description = version_blob.remove("description").and_then(|x| utils::unwrap_string(x).ok());
    let prod_dependencies_raw = utils::unwrap_object(version_blob.remove("dependencies").unwrap_or(empty_object())).unwrap_or_default();
    let dev_dependencies_raw = utils::unwrap_object(version_blob.remove("devDependencies").unwrap_or(empty_object())).unwrap_or_default();
    let peer_dependencies_raw = utils::unwrap_object(version_blob.remove("peerDependencies").unwrap_or(empty_object())).unwrap_or_default();
    let optional_dependencies_raw = utils::unwrap_object(version_blob.remove("optionalDependencies").unwrap_or(empty_object())).unwrap_or_default();
    let mut dist = utils::unwrap_object(version_blob.remove("dist").unwrap()).unwrap();
    let shasum = utils::unwrap_string(dist.remove("shasum")?).expect(&format!("Expected a string. Blob = {:?}", version_blob));
    let tarball = utils::unwrap_string(dist.remove("tarball").unwrap()).unwrap();

    let prod_dependencies = prod_dependencies_raw.into_iter().enumerate().map(|(i, (p, c))| 
        (p, (u64::try_from(i).unwrap(), utils::unwrap_string(c).unwrap()))
    ).collect();

    let dev_dependencies = dev_dependencies_raw.into_iter().enumerate().map(|(i, (p, c))|
        (p, (u64::try_from(i).unwrap(), utils::unwrap_string(c).unwrap()))
    ).collect();

    let peer_dependencies = peer_dependencies_raw.into_iter().enumerate().map(|(i, (p, c))|
        (p, (u64::try_from(i).unwrap(), utils::unwrap_string(c).unwrap()))
    ).collect();

    let optional_dependencies = optional_dependencies_raw.into_iter().enumerate().map(|(i, (p, c))|
        (p, (u64::try_from(i).unwrap(), utils::unwrap_string(c).unwrap()))
    ).collect();


    Some(VersionPackument {
        description: description,
        shasum: shasum,
        tarball: tarball,
        dependencies: Dependencies {
            prod_dependencies: prod_dependencies,
            dev_dependencies: dev_dependencies,
            peer_dependencies: peer_dependencies,
            optional_dependencies: optional_dependencies,
        },
        extra_metadata: version_blob.into_iter().collect()
    })
}

fn empty_object() -> Value {
    Value::Object(serde_json::Map::new())
}