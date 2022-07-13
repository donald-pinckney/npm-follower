use downloader::download_db::download_to_dest;
use utils::check_no_concurrent_processes;

pub fn main() {
    check_no_concurrent_processes("downloader");

    let args = std::env::args().collect::<Vec<_>>();
    if args.len() < 3 {
        eprintln!(
            "Usage: {} <destination directory> <number of parallel downloads> [optional: true/false for retrying failed downloads]",
            args[0]
        );
        std::process::exit(1);
    }

    let conn = postgres_db::connect();
    let dest = &args[1];
    let num_workers = args[2].parse::<usize>().unwrap();
    let retry = if args.len() > 3 {
        args[3] == "true"
    } else {
        false
    };

    // check that the directory exists
    if !std::path::Path::new(dest).exists() {
        eprintln!("Destination directory does not exist");
        std::process::exit(1);
    }

    download_to_dest(&conn, dest, num_workers, retry).expect("Failed to download");
}
