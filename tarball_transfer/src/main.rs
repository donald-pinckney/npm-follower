use std::sync::Arc;

use blob_idx_server::{
    errors::{BlobError, ClientError, HTTPError},
    http::{JobType, SubmitJobRequest},
    job::ClientResponse,
};
use postgres_db::{
    connection::DbConnection,
    download_tarball::{self, DownloadedTarball},
    internal_state,
};
use tokio::sync::{mpsc, Mutex};

const PAGE_SIZE: i64 = 1024;

#[tokio::main]
async fn main() {
    utils::check_no_concurrent_processes("tarball_transfer");
    dotenvy::dotenv().ok();
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

    let mut first_ever_tb = Some(
        download_tarball::query_first_tarball_by_url(&mut conn)
            .expect("Error querying first tarball by url"),
    );
    let (mut last_url, mut queued_up_to) = internal_state::query_tarball_transfer_last(&mut conn)
        .unwrap_or((first_ever_tb.as_ref().unwrap().tarball_url.to_string(), 0));

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
        let mut tarballs =
            download_tarball::query_tarballs_after_url(&mut conn, &last_url, PAGE_SIZE);
        if queued_up_to == 0 {
            // first time, add the first tarball
            tarballs.insert(0, std::mem::take(&mut first_ever_tb).unwrap());
        }
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
    db_tx: mpsc::Sender<Vec<(String, String)>>,
    worker_id: usize,
) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn(async move {
        println!("Spawned transfer worker {}", worker_id);
        let discovery_scp = std::env::var("DISCOVERY_SCP").expect("DISCOVERY_SCP not set");
        let blob_api_url = std::env::var("BLOB_API_URL").expect("BLOB_API_URL not set");
        let blob_api_key = std::env::var("BLOB_API_KEY").expect("BLOB_API_KEY not set");
        let username = discovery_scp
            .split('@')
            .next()
            .expect("Invalid DISCOVERY_SCP");
        let mut tarballs = Vec::new();
        let mut retry = false;

        'o: loop {
            // unwraps, retries if None. yeah this is kinda nasty
            macro_rules! unwrap_or_retry {
                ($e:expr) => {
                    match $e {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("[{}] Error: {}", worker_id, e);
                            retry = true;
                            continue 'o;
                        }
                    }
                };
            }
            if retry {
                retry = false;
            } else {
                tarballs = {
                    let mut rx = rx.lock().await;
                    match rx.recv().await {
                        Some(t) => t,
                        None => {
                            println!("Transfer worker {} exiting", worker_id);
                            return;
                        }
                    }
                };
            }
            println!(
                "[{}] Got {} tarballs to transfer",
                worker_id,
                tarballs.len()
            );

            // firsly, move the files (locally) to a tmp dir
            let mut tmp_dir = std::env::temp_dir();
            tmp_dir.push(format!("tarball_transfer_{}", worker_id));
            if tmp_dir.exists() {
                unwrap_or_retry!(tokio::fs::remove_dir_all(&tmp_dir).await);
            }
            unwrap_or_retry!(tokio::fs::create_dir_all(&tmp_dir).await);
            let mut processed_tarballs = Vec::new();
            for tarball in &tarballs {
                if tarball.tgz_local_path.is_none() {
                    println!(
                        "[{}] Tarball {} has no local path, skipping",
                        worker_id, tarball.tarball_url
                    );
                    continue;
                }
                let local_path = tarball.tgz_local_path.as_ref().unwrap();
                // make into pathbuf
                let local_path = std::path::PathBuf::from(local_path);
                let filename = local_path.file_name();
                if filename.is_none() {
                    println!(
                        "[{}] Tarball {} has no filename, skipping",
                        worker_id, tarball.tarball_url
                    );
                    continue;
                }
                let filename = filename.unwrap().to_string_lossy().to_string();
                // symlink the file into the tmp dir
                let mut tmp_path = tmp_dir.clone();
                tmp_path.push(&filename);
                unwrap_or_retry!(tokio::fs::symlink(local_path, &tmp_path).await);

                processed_tarballs.push((tarball.tarball_url.clone(), filename));
            }

            // now, we rsync over all the files in the tmp dir
            let remote_dir = format!("/scratch/{}/tarballs{}/", username, worker_id);
            let cmd = format!(
                "rsync -KLr {}/* {}:{}",
                tmp_dir.to_string_lossy(),
                discovery_scp,
                remote_dir
            );

            println!("[{}] Running command: {}", worker_id, cmd);
            let output = unwrap_or_retry!(
                tokio::process::Command::new("bash")
                    .arg("-c")
                    .arg(cmd)
                    .output()
                    .await
            );

            if !output.status.success() {
                eprintln!(
                    "[{}] Command failed with status {:?}: {}",
                    worker_id,
                    output.status.code(),
                    String::from_utf8_lossy(&output.stderr)
                );
                retry = true;
                continue;
            }

            // call the blob api to update the tarball urls
            let client = reqwest::Client::new();
            let filepaths = processed_tarballs
                .iter()
                .map(|(_, filename)| format!("{}/{}", remote_dir, filename))
                .collect::<Vec<_>>();
            let mut body_data = SubmitJobRequest {
                job_type: JobType::StoreTarballs { filepaths },
            };
            loop {
                let res = client
                    .post(&format!("{}/job/submit", blob_api_url))
                    .header("Authorization", blob_api_key.clone())
                    .json(&body_data)
                    .send()
                    .await;
                match res {
                    Ok(res) => {
                        let txt = res.text().await.unwrap();
                        if txt.is_empty() {
                            // success
                            break;
                        }
                        println!("[{}] Got response: {}", worker_id, txt);
                        let obj: serde_json::Value = serde_json::from_str(&txt).unwrap();
                        let err: Result<ClientError, _> =
                            serde_json::from_value(obj["error"].clone());
                        match err {
                            Ok(ClientError::BlobError(BlobError::AlreadyExists(file))) => {
                                // rerun, by deleting the file name.
                                let path = format!("{}/{}", remote_dir, file);
                                match body_data {
                                    SubmitJobRequest {
                                        job_type: JobType::StoreTarballs { ref mut filepaths },
                                    } => {
                                        if filepaths.len() == 1 {
                                            // we're done. this is the only file, and it already exists
                                            break;
                                        }
                                        processed_tarballs
                                            .retain(|(_, filename)| filename != &file);
                                        filepaths.retain(|f| f != &path);
                                    }
                                    _ => unreachable!(),
                                }
                            }
                            _ => {
                                eprintln!("[{}] Error: {:?}", worker_id, txt);
                                retry = true;
                                continue 'o;
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("[{}] Error sending request to job: {}", worker_id, e);
                        retry = true;
                        continue 'o;
                    }
                }
            }

            if (db_tx.send(processed_tarballs).await).is_err() {
                return;
            }
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
            println!("Got {} tarballs to edit", tarballs.len());
            for (url, name) in tarballs {
                download_tarball::set_blob_storage_key(&mut conn, &url, &name);
            }
        }
    })
}
