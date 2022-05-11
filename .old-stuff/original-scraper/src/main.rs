use indicatif::ProgressStyle;
use crate::packument::Dependencies;
use packument::Packument;
use packument::PackageReference;
use packument::VersionPackument;
use std::path::Path;
use std::collections::HashSet;
use std::collections::HashMap;
use std::io::BufReader;
use std::fs::File;
use serde_json::{Value};
use std::error::Error;
use std::fs;
use chrono::{DateTime, Utc};
use std::convert::TryFrom;
use version::Version;
use inserter::Inserter;
use indicatif::ProgressIterator;

mod version;
mod packument;
mod inserter;
mod sql_data;
mod sql_insertable;

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



fn read_all_packages() -> Result<HashSet<String>, Box<dyn Error>> {
    let file = File::open("../all_packages.json")?;
    let reader = BufReader::new(file);

    let mut j = unwrap_object(serde_json::from_reader(reader)?).unwrap();
    let rows = unwrap_array(j["rows"].take());
    let pkg_names: HashSet<String> = rows.into_iter().map(|r| r["key"].as_str().unwrap().to_owned()).collect();
    Ok(pkg_names)
    
}

fn read_downloads(all_packages: &HashSet<String>) -> Result<HashMap<&String, u64>, Box<dyn Error>> {
    let file = File::open("../all_downloads.json")?;
    let reader = BufReader::new(file);

    let j = unwrap_object(serde_json::from_reader(reader)?).unwrap();
    Ok(j.into_iter().flat_map(|(pkg_name, count_val)| {
        let pkg_ref = all_packages.get(&pkg_name)?;
        Some((pkg_ref, unwrap_number(count_val).as_u64().unwrap()))
    }).collect())
}



fn parse_datetime(x: String) -> DateTime<Utc> {
    let dt = DateTime::parse_from_rfc3339(&x)
        .or_else(|_| DateTime::parse_from_rfc3339(&(x + "Z"))).unwrap();
    dt.with_timezone(&Utc)
}

fn empty_object() -> Value {
    Value::Object(serde_json::Map::new())
}

fn process_version(all_packages: &HashSet<String>, mut version_blob: serde_json::Map<String, Value>) -> Option<VersionPackument> {
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

    let prod_dependencies = prod_dependencies_raw.into_iter().enumerate().map(|(i, (p, c))| 
        (PackageReference::lookup(all_packages, p), (u64::try_from(i).unwrap(), unwrap_string(c).ok()))
    ).collect();

    let dev_dependencies = dev_dependencies_raw.into_iter().enumerate().map(|(i, (p, c))|
        (PackageReference::lookup(all_packages, p), (u64::try_from(i).unwrap(), unwrap_string(c).ok()))
    ).collect();

    let peer_dependencies = peer_dependencies_raw.into_iter().enumerate().map(|(i, (p, c))|
        (PackageReference::lookup(all_packages, p), (u64::try_from(i).unwrap(), unwrap_string(c).ok()))
    ).collect();

    let optional_dependencies = optional_dependencies_raw.into_iter().enumerate().map(|(i, (p, c))|
        (PackageReference::lookup(all_packages, p), (u64::try_from(i).unwrap(), unwrap_string(c).ok()))
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

fn process_packument_blob(all_packages: &HashSet<String>, v: Value, _pkg_name: String) -> Result<Packument, String> {
    let mut j = unwrap_object(v).unwrap();
    
    let dist_tags_raw_maybe = j.remove("dist-tags").map(|dt| unwrap_object(dt).unwrap());
    let mut dist_tags: Option<HashMap<String, Version>> = dist_tags_raw_maybe.map(|dist_tags_raw| 
        dist_tags_raw.into_iter().map(|(tag, v_str)| 
            (tag, Version::parse(unwrap_string(v_str).unwrap()).unwrap())
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

    let version_times: HashMap<_, _> = times.into_iter().flat_map(|(v_str, t)| 
        Some((Version::parse(v_str.clone())?, t))
    ).collect();

    let version_packuments_map = j.remove("versions").map(|x| unwrap_object(x).unwrap()).unwrap_or_default(); //unwrap_object(j.remove("versions").unwrap());
    let version_packuments = version_packuments_map.into_iter().flat_map(|(v_str, blob)|
        Some((Version::parse(v_str).unwrap(), process_version(all_packages, unwrap_object(blob).unwrap())?))
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

fn read_packuments_file<P: AsRef<Path>>(all_packages: &HashSet<String>, path: P) -> 
    HashMap<&String, Result<Packument, String>> {
    let file = File::open(path).unwrap();
    let reader = BufReader::new(file);
    let packument_blobs: HashMap<String, Value> = serde_json::from_reader(reader).unwrap();
    // println!("  Processing JSON");
    let packuments: HashMap<&String, _> = packument_blobs.into_iter().flat_map(|(pkg, pak)| {
        let pkg_ref = all_packages.get(&pkg)?;
        Some((pkg_ref, process_packument_blob(all_packages, pak, pkg)))
    }).collect();
    packuments
}


fn main() {
    println!("Reading all packages.");
    let mut pkg_names = read_all_packages().unwrap();
    println!("{} packages loaded", pkg_names.len());

    let mut bad_pkg_names = HashSet::new();
    let mut ok_pkg_names = HashSet::new();

    let packuemnt_paths: Vec<_> = fs::read_dir("../outputs").unwrap().collect();

    let it = packuemnt_paths.iter().progress();
    it.progress.set_style(
        ProgressStyle::default_bar()
        .template("[{elapsed_precise}] {bar:100} {percent} [{pos:>7}/{len:7}] [{per_sec}] [{eta_precise}]")
    );
    for entry in it {
        let p = entry.as_ref().unwrap().path();
        println!("Validating packuments in {}", p.display());
        let packuments = read_packuments_file(&pkg_names, p);
        // println!("  {} packuments loaded.", packuments.len());

        for (pkg, pack) in packuments {
            if pack.is_err() {
                println!("    **** Error processing pkg: {}. Error: {}", pkg, pack.unwrap_err());
                bad_pkg_names.insert(pkg.clone());
            } else {
                ok_pkg_names.insert(pkg.clone());
            }
        }
    }

    pkg_names.retain(|x| !bad_pkg_names.contains(x) && ok_pkg_names.contains(x));
    println!("{} packages kept", pkg_names.len());


    println!("\n\nReading download metrics.");
    let downloads = read_downloads(&pkg_names).unwrap();
    println!("download metrics for {} packages loaded\n\n", downloads.len());

    

    let mut inserter = Inserter::new(&pkg_names, downloads);

    let it = packuemnt_paths.iter().progress();
    it.progress.set_style(
        ProgressStyle::default_bar()
        .template("[{elapsed_precise}] {bar:100} {pos:>7}/{len:7} [{percent}%] [{per_sec}] [{eta_precise} eta]")
    );
    for entry in it {
        let p = entry.as_ref().unwrap().path();

        println!("Inserting packuments in {}", p.display());
        let packuments = read_packuments_file(&pkg_names, p);
        // println!("  {} packuments loaded. Inserting them into DB...", packuments.len());

        for (pkg, pack) in packuments {
            inserter.insert_packument(pkg, pack.unwrap());
        }
    }

    println!("All done inserting data. Now building indexes, may take some time...");
    inserter.build_indexes();
    println!("All done!");
}
