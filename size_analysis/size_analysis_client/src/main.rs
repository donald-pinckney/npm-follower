use std::{
    collections::HashSet,
    os::unix::prelude::PermissionsExt,
    path::{Path, PathBuf},
};

use size_analysis::SizeAnalysisTarball;

#[tokio::main]
async fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() != 2 {
        eprintln!("Usage: {} <tarball>", args[0]);
        std::process::exit(1);
    }

    let dir = std::fs::canonicalize(&args[1]).unwrap();
    let dir_pkg = get_pkg_dir(&dir);

    // extract the tarballs
    let fdir = extract_tarball(&dir).unwrap();

    let mut total_size = 0;
    let mut total_files = 0;
    let mut total_size_code = 0;

    // get the size of each file
    for file in fdir {
        let metadata = std::fs::metadata(&file).unwrap();
        let size = metadata.len();
        total_size += size;
        total_files += 1;
        if filter_ext(&file) {
            total_size_code += size;
        }
    }

    let result = SizeAnalysisTarball {
        tarball_url: args[1].clone(),
        total_files,
        total_size,
        total_size_code,
    };

    // remove the extracted tarballs
    std::fs::remove_dir_all(dir_pkg).ok();

    let json = serde_json::to_string(&result).unwrap();
    println!("{json}");
}

fn get_pkg_dir(path: &Path) -> PathBuf {
    let dir = std::path::Path::new(path).parent().unwrap();
    // add /package to the end of the path
    dir.join("package")
}

// returns true is file ext is either one of: "js, ts, jsx, tsx, json"
fn filter_ext(file: &Path) -> bool {
    let ext = file.extension().unwrap_or_default();
    matches!(
        ext.to_str().unwrap_or_default(),
        "js" | "ts" | "jsx" | "tsx" | "json"
    )
}

// extracts a tarball using "tar -xzf $TAR -C $(dirname $TAR)"
// and returns a list of paths to all the files in the tarball (recursively)
pub fn extract_tarball(tarball: &Path) -> Result<HashSet<PathBuf>, std::io::Error> {
    let dir = std::path::Path::new(tarball).parent().unwrap();

    // set perms to the dir
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o777))?;

    let mut cmd = std::process::Command::new("tar");
    cmd.arg("-xf").arg(tarball).arg("-C").arg(dir);
    let output = cmd.output().unwrap();
    if !output.status.success() {
        eprintln!("tar failed: {}", String::from_utf8_lossy(&output.stderr));
        std::process::exit(1);
    }

    let mut files = HashSet::new();
    let pkg_dir = format!("{}/package", dir.to_str().unwrap());
    // create dir
    std::fs::create_dir_all(&pkg_dir)?;
    std::fs::set_permissions(&pkg_dir, std::fs::Permissions::from_mode(0o777))?;
    fn recurse(dir: &str, files: &mut HashSet<PathBuf>) {
        if let Ok(mut entries) = std::fs::read_dir(dir) {
            while let Some(Ok(entry)) = entries.next() {
                let path = entry.path();
                // do not recur into node_modules
                if path.to_str().unwrap_or_default().contains("node_modules") {
                    continue;
                }
                // set perms
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o777)).ok();
                if path.is_dir() {
                    recurse(path.to_str().unwrap(), files);
                } else if path.is_file() {
                    files.insert(path);
                }
            }
        }
    }
    recurse(&pkg_dir, &mut files);

    Ok(files)
}
