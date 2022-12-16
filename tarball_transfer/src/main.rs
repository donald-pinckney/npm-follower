use std::sync::Arc;

use postgres_db::{
    connection::DbConnection,
    download_tarball::{self, DownloadedTarball},
    internal_state,
};
use tokio::sync::{mpsc, Mutex};

const PAGE_SIZE: i64 = 10; // TODO: increase this to 1024

#[tokio::main]
async fn main() {
    utils::check_no_concurrent_processes("tarball_transfer");
    let mut conn = DbConnection::connect();

    let args = std::env::args().collect::<Vec<_>>();
    if args.len() != 2 {
        panic!("Usage: tarball_transfer <num_workers>");
    }
    let num_workers = args[1].parse::<usize>().unwrap();

    let mut workers = Vec::new();
    let (tb_tx, tb_rx) = mpsc::channel(num_workers);
    let (db_tx, mut db_rx) = mpsc::channel(num_workers);
    let tb_rx = Arc::new(tokio::sync::Mutex::new(tb_rx));
    for id in 0..num_workers {
        let spawned = spawn_transfer_worker(tb_rx.clone(), db_tx.clone(), id);
        workers.push(spawned);
    }

    let db_worker_conn = DbConnection::connect(); // double the connections, double the fun
    let mut db_worker = spawn_db_worker(db_rx, db_worker_conn);

    let (mut last_url, mut queued_up_to) = internal_state::query_tarball_transfer_last(&mut conn)
        .unwrap_or((
            download_tarball::query_first_tarball_by_url(&mut conn)
                .expect("Error querying first tarball by url")
                .tarball_url,
            0,
        ));

    let num_tarballs_total = download_tarball::num_total_downloaded_tarballs(&mut conn);
    let mut num_tarballs_so_far = 0;

    loop {
        println!(
            "Fetching seq > {}, starting url = {}, page size = {} ({:.1}%)",
            queued_up_to,
            last_url,
            PAGE_SIZE,
            100.0 * (num_tarballs_so_far as f64) / (num_tarballs_total as f64)
        );
        let tarballs = download_tarball::query_tarballs_after_url(&mut conn, &last_url, PAGE_SIZE);
        let num_tarballs = tarballs.len() as i64;
        num_tarballs_so_far += num_tarballs;
        if num_tarballs == 0 {
            break;
        }

        let last_url_in_page = tarballs.last().unwrap().tarball_url.to_string();

        tb_tx.send(tarballs).await.unwrap();
        last_url = last_url_in_page;
        queued_up_to += num_tarballs;
        internal_state::set_tarball_transfer_last(last_url.clone(), queued_up_to, &mut conn);
    }

    // close channels to signal workers to exit and wait for them to exit
    drop(tb_tx);
    drop(db_tx);
    for worker in workers {
        worker.await.unwrap();
    }
    db_worker.await.unwrap();
}

pub fn spawn_transfer_worker(
    rx: Arc<Mutex<mpsc::Receiver<Vec<DownloadedTarball>>>>, // if we close this channel, the workers will exit
    tx: mpsc::Sender<Vec<(String, String)>>,
    worker_id: usize,
) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn(async move {
        println!("Spawned transfer worker {}", worker_id);
        loop {
            let tarballs = {
                let mut rx = rx.lock().await;
                match rx.recv().await {
                    Some(t) => t,
                    None => {
                        println!("Transfer worker {} exiting", worker_id);
                        return;
                    }
                }
            };
            println!(
                "[{}] Got {} tarballs to transfer",
                worker_id,
                tarballs.len()
            );
            tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
            tx.send(
                tarballs
                    .into_iter()
                    .map(|tb| (tb.tarball_url, "placeholder".to_string()))
                    .collect(),
            )
            .await
            .unwrap();
        }
    })
}

pub fn spawn_db_worker(
    mut rx: mpsc::Receiver<Vec<(String, String)>>,
    mut conn: DbConnection,
) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn(async move {
        println!("Spawned db worker");
        loop {
            let tarballs = match rx.recv().await {
                Some(t) => t,
                None => {
                    println!("DB worker exiting");
                    return;
                }
            };
            println!("Got {} tarballs to insert", tarballs.len());
        }
    })
}
