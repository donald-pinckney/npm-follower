use utils::check_no_concurrent_processes;

pub fn main() {
    check_no_concurrent_processes("downloader");

    let args = std::env::args().collect::<Vec<_>>();
    if args.len() != 2 {
        eprintln!("Usage: {} <destination directory>", args[0]);
        std::process::exit(1);
    }

    download_to_dest(&args[0]).unwrap();
}

/// Downloads all present tasks to the given directory. Inserts each task completed in the
/// downloaded_tarballs table, and removes the completed tasks from the download_tasks table.
pub fn download_to_dest(dest: &str) -> std::io::Result<()> {
    let conn = postgres_db::connect();

    // check that the directory exists
    if !std::path::Path::new(dest).exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Destination directory does not exist",
        ));
    }

    // get the number of tasks to download

    Ok(())
}

pub fn retrieve_num_of_tasks() -> usize {
    1337
}
