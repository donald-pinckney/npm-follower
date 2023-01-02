use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use postgres_db::diff_analysis::FileDiff;
use tokio::io::AsyncReadExt;

#[tokio::main]
async fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() != 3 {
        eprintln!("Usage: {} <old npm tarball> <new npm tarball>", args[0]);
        std::process::exit(1);
    }
    let dir_old = std::fs::canonicalize(&args[1]).unwrap();
    let dir_old_pkg = get_pkg_dir(&dir_old);
    let dir_new = std::fs::canonicalize(&args[2]).unwrap();
    let dir_new_pkg = get_pkg_dir(&dir_new);

    // extract the tarballs
    let fdir_old = extract_tarball(&dir_old).unwrap();
    let mut fdir_new = extract_tarball(&dir_new).unwrap();

    // if either one of the directories has more than 200 files, fail
    if fdir_old.len() > 200 || fdir_new.len() > 200 {
        // TODO: make better errors
        eprintln!("Too many files in the tarball");
        std::process::exit(103);
    }

    for file_old in fdir_old {
        let display_name = file_old.strip_prefix(&dir_old_pkg).unwrap();
        let file_new = Path::new(&dir_new_pkg).join(display_name);
        let (new_lines_count, old_lines_count, avg_width) =
            calculate_proportions(&file_old, &file_new).await.unwrap();
        if fdir_new.remove(&file_new) {
            let (num_added, num_removed) = run_diff(&file_old, &file_new).await.unwrap();
            println!(
                "{} {} {} {:?} {:?} {}",
                display_name.display(),
                num_added,
                num_removed,
                new_lines_count,
                old_lines_count,
                avg_width
            );
        } else {
            println!(
                "{} {} {} {:?} {:?} {}",
                display_name.display(),
                0,
                0,
                new_lines_count,
                old_lines_count,
                avg_width
            );
        }
    }

    for file_new in fdir_new {
        let display_name = file_new.strip_prefix(&dir_new_pkg).unwrap();
        let file_old = Path::new(&dir_old_pkg).join(display_name);
        let (new_lines_count, old_lines_count, avg_width) =
            calculate_proportions(&file_old, &file_new).await.unwrap();
        println!(
            "{} {} {} {:?} {:?} {}",
            display_name.display(),
            0,
            0,
            new_lines_count,
            old_lines_count,
            avg_width
        );
    }
}

// calculates the length and average line width of a file
async fn calculate_proportions(
    file_old: &Path,
    file_new: &Path,
) -> Result<(Option<usize>, Option<usize>, f64), std::io::Error> {
    let mut file_old = tokio::fs::File::open(file_old).await.ok();
    let mut file_new = tokio::fs::File::open(file_new).await.ok();

    let mut buf_old = String::new();
    let mut buf_new = String::new();

    if let Some(file_old) = file_old.as_mut() {
        file_old.read_to_string(&mut buf_old).await?;
    }
    if let Some(file_new) = file_new.as_mut() {
        file_new.read_to_string(&mut buf_new).await?;
    }

    let new_lines = buf_new.lines();
    let old_lines = buf_old.lines();

    let new_lines_count = new_lines.clone().count();
    let old_lines_count = old_lines.clone().count();
    let total_width =
        new_lines.map(|l| l.len()).sum::<usize>() + old_lines.map(|l| l.len()).sum::<usize>();
    let avg_width = total_width as f64 / (new_lines_count + old_lines_count) as f64;

    Ok((
        file_new.map(|_| new_lines_count),
        file_old.map(|_| old_lines_count),
        avg_width,
    ))
}

// runs the diff command on the given two files and returns the number of lines added and removed
async fn run_diff(file_old: &Path, file_new: &Path) -> Result<(usize, usize), std::io::Error> {
    let mut cmd = tokio::process::Command::new("diff");
    cmd.arg(file_old).arg(file_new);
    let output = cmd.output().await?;
    let stdout = String::from_utf8(output.stdout).unwrap();
    // count the number of lines that start with '>' and '<'.
    let mut lines_added = 0;
    let mut lines_removed = 0;
    for line in stdout.lines() {
        if line.starts_with('>') {
            lines_added += 1;
        } else if line.starts_with('<') {
            lines_removed += 1;
        }
    }
    Ok((lines_added, lines_removed))
}

fn get_pkg_dir(path: &Path) -> PathBuf {
    let dir = std::path::Path::new(path).parent().unwrap();
    // add /package to the end of the path
    dir.join("package")
}

// returns true is file ext is either one of: "js, ts, jsx, tsx, json, wat, wast"
fn filter_ext(file: &Path) -> bool {
    let ext = file.extension().unwrap_or_default();
    matches!(
        ext.to_str().unwrap_or_default(),
        "js" | "ts" | "jsx" | "tsx" | "json" | "wat" | "wast"
    )
}

// extracts a tarball using "tar -xzf $TAR -C $(dirname $TAR)"
// and returns a list of paths to all the files in the tarball (recursively)
pub fn extract_tarball(tarball: &Path) -> Result<HashSet<PathBuf>, std::io::Error> {
    let dir = std::path::Path::new(tarball).parent().unwrap();
    let mut cmd = std::process::Command::new("tar");
    cmd.arg("-xzf").arg(tarball).arg("-C").arg(dir);
    let output = cmd.output().unwrap();
    if !output.status.success() {
        eprintln!("tar failed: {}", String::from_utf8_lossy(&output.stderr));
        std::process::exit(1);
    }

    let mut files = HashSet::new();
    let pkg_dir = format!("{}/package", dir.to_str().unwrap());
    fn recurse(dir: &str, files: &mut HashSet<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                recurse(path.to_str().unwrap(), files);
            } else if path.is_file() && filter_ext(&path) {
                files.insert(path);
            }
        }
    }
    recurse(&pkg_dir, &mut files);

    Ok(files)
}
