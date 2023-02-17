use downloader::download_db::download_to_cluster;
use postgres_db::connection::DbConnection;
use utils::check_no_concurrent_processes;

#[tokio::main]
pub async fn main() {
    check_no_concurrent_processes("cluster_downloader");

    let args = std::env::args().collect::<Vec<_>>();
    if args.len() < 2 {
        eprintln!(
            "Usage: {} <number of parallel download requests> [optional: true/false for retrying failed downloads]",
            args[0]
        );
        std::process::exit(1);
    }

    let mut conn = DbConnection::connect();
    let num_parallel_dl = args[1].parse::<usize>().unwrap();
    let retry = if args.len() > 2 {
        args[2] == "true"
    } else {
        false
    };

    download_to_cluster(&mut conn, num_parallel_dl, retry)
        .await
        .expect("Failed to download");
}
