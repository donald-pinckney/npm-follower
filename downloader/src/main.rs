use postgres_db::download_queue::download_to_dest;
use utils::check_no_concurrent_processes;

pub fn main() {
    check_no_concurrent_processes("downloader");

    let args = std::env::args().collect::<Vec<_>>();
    if args.len() != 2 {
        eprintln!("Usage: {} <destination directory>", args[0]);
        std::process::exit(1);
    }

    let conn = postgres_db::connect();
    let dest = &args[1];

    // check that the directory exists
    if !std::path::Path::new(dest).exists() {
        eprintln!("Destination directory does not exist");
    }

    download_to_dest(&conn, dest).expect("Failed to download");
}
