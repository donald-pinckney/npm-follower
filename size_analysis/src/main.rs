use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use blob_idx_server::{
    http::{JobType, SubmitJobRequest},
    job::{ClientResponse, TarballResult},
};
use diesel::QueryableByName;
use postgres_db::{
    connection::{DbConnection, DbConnectionInTransaction, QueryRunner},
    download_tarball::{self, DownloadedTarball},
    internal_state,
};
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use size_analysis::SizeAnalysisTarball;
use tokio::{
    sync::{
        mpsc::{self, Receiver, Sender},
        Mutex,
    },
    task::JoinHandle,
};

#[derive(Serialize, Deserialize, QueryableByName, Debug, Clone)]
struct QRes {
    #[sql_type = "diesel::sql_types::Text"]
    tarball_url: String,
    #[sql_type = "diesel::sql_types::Text"]
    blob_storage_key: String,
}

#[derive(QueryableByName, Debug, Clone)]
struct QCount {
    #[sql_type = "diesel::sql_types::BigInt"]
    count: i64,
}

const QUERY: &str = r#"
SELECT * FROM size_analysis_to_compute
"#;

const COUNT_QUERY: &str = r#"
SELECT COUNT(*) FROM size_analysis_to_compute
"#;

const CHUNK_SIZE: usize = 5000;
const NUM_WORKERS: usize = 50;
const NUM_LOCAL_WORKERS: usize = 3;

#[tokio::main]
async fn main() {
    utils::check_no_concurrent_processes("size_analysis");
    dotenvy::dotenv().ok();
    let mut conn: DbConnection = DbConnection::connect();
    let q = diesel::sql_query(QUERY);
    // shuffle the result
    let mut res: Vec<QRes> = conn.load(q).unwrap();
    res.shuffle(&mut rand::thread_rng());

    let (data_tx, data_rx) = tokio::sync::mpsc::channel(NUM_WORKERS);
    let data_rx = Arc::new(Mutex::new(data_rx));
    let (db_tx, db_rx) = tokio::sync::mpsc::channel(NUM_WORKERS);

    let mut chunk_workers = Vec::new();
    for id in 0..NUM_LOCAL_WORKERS {
        chunk_workers.push(spawn_compute_worker(data_rx.clone(), db_tx.clone(), id));
    }
    let db_worker = spawn_db_worker(db_rx, DbConnection::connect());

    let total_count: Vec<QCount> = conn.load(diesel::sql_query(COUNT_QUERY)).unwrap();
    println!("[MANAGER] Total count: {}", total_count[0].count);

    let mut total = 0;
    for chunk in res.chunks(CHUNK_SIZE) {
        let chunk = chunk.to_vec();
        total += chunk.len();
        data_tx.send(chunk).await.unwrap();
        println!("[MANAGER] Progress: {}/{}", total, total_count[0].count);
    }

    println!("[MANAGER] DONE! Waiting for workers to finish...");
    drop(data_tx);

    for worker in chunk_workers {
        worker.await.unwrap();
    }

    drop(db_tx);
    db_worker.await.unwrap();
}

fn spawn_compute_worker(
    data_rx: Arc<Mutex<Receiver<Vec<QRes>>>>,
    db_tx: Sender<Vec<SizeAnalysisTarball>>,
    worker_id: usize,
) -> JoinHandle<()> {
    let thunk = async move {
        let blob_api_url = std::env::var("BLOB_API_URL").expect("BLOB_API_URL not set");
        let blob_api_key = std::env::var("BLOB_API_KEY").expect("BLOB_API_KEY not set");

        loop {
            let chunk = {
                let mut data_rx = data_rx.lock().await;
                match data_rx.recv().await {
                    Some(c) => c,
                    None => return, // channel closed
                }
            };

            let mut tarball_chunks = vec![];
            let mut lookup = HashMap::new();
            for chunk in chunk.chunks(CHUNK_SIZE / NUM_WORKERS) {
                let mut tb_chunk = vec![];
                for qres in chunk {
                    tb_chunk.push(qres.blob_storage_key.clone());
                    lookup.insert(qres.blob_storage_key.clone(), qres.tarball_url.clone());
                }
                tarball_chunks.push(tb_chunk);
            }

            let job = SubmitJobRequest {
                job_type: JobType::Compute {
                    binary: "/scratch/cassano.f/blob_bins/size_analysis_client".to_string(),
                    tarball_chunks,
                    timeout: Some(600),
                },
            };

            let client = reqwest::Client::new();
            let time = chrono::Local::now();
            println!("[{worker_id} - {time}] Submitting job");
            let http_resp = client
                .post(&format!("{blob_api_url}/job/submit"))
                .header("Authorization", &blob_api_key)
                .json(&job)
                .send()
                .await
                .unwrap();
            let resps: Vec<ClientResponse> = http_resp.json().await.unwrap();
            let mut results = vec![];
            let mut dedup = HashSet::new();
            for resp in resps {
                match resp {
                    ClientResponse::Message(m) => {
                        let res = HashMap::<String, TarballResult>::deserialize(m)
                            .expect("Failed to deserialize");
                        for (tb, res) in res {
                            if !dedup.insert(tb.clone()) {
                                // this may happen for some reason?
                                eprintln!("[{worker_id}] Duplicate tb: {tb}");
                                continue;
                            }

                            if res.exit_code == 0 && !res.stdout.is_empty() {
                                let stdout = base64::decode(&res.stdout).expect("Failed to decode");
                                match serde_json::from_slice::<SizeAnalysisTarball>(&stdout) {
                                    Ok(mut res) => {
                                        let tarball_url = match lookup.get(&tb) {
                                            Some(u) => u,
                                            None => {
                                                eprintln!("[{worker_id}] Failed to find tarball url for {tb}");
                                                continue;
                                            }
                                        };
                                        res.tarball_url = tarball_url.clone();
                                        results.push(res)
                                    },
                                    Err(_) => {
                                        eprintln!(
                                            "[{worker_id}] Failed to deserialize stdout: {stdout:?}"
                                        );
                                    }
                                }
                            } else {
                                eprintln!("[{worker_id}] Failed to run size analysis: {res:?}");
                            };
                        }
                    }
                    ClientResponse::Error(e) => {
                        eprintln!("[{worker_id}] Client Error: {e}");
                    }
                };
            }

            db_tx.send(results).await.unwrap();
        }
    };
    tokio::task::spawn(thunk)
}

fn spawn_db_worker(
    mut db_rx: Receiver<Vec<SizeAnalysisTarball>>,
    mut conn: DbConnection,
) -> JoinHandle<()> {
    let thunk = async move {
        while let Some(results) = db_rx.recv().await {
            println!("[DB] Inserting {} results", results.len());
            if !results.is_empty() {
                conn.run_psql_transaction(|mut c| {
                    delete_rows_after_compute(&results, &mut c);
                    // batch insert

                    let mut insert = String::new();
                    for res in &results {
                        insert.push_str(&format!(
                            "({}, {}, {}, {}), ",
                            res.tarball_url, res.total_files, res.total_size, res.total_size_code
                        ));
                    }

                    // remove trailing comma and space
                    insert.pop();
                    insert.pop();

                    let query = format!(
                        "INSERT INTO size_analysis_tarball (tarball_url, total_files, total_size, total_size_code) VALUES {insert} ON CONFLICT DO NOTHING",
                    );

                    let q = diesel::sql_query(query);
                    c.execute(q).unwrap();

                    Ok(((), true))
                })
                .expect("Failed to insert results");
            }
            println!("[DB] Done");
        }
    };
    tokio::task::spawn(thunk)
}

fn delete_rows_after_compute(
    results: &[SizeAnalysisTarball],
    conn: &mut DbConnectionInTransaction,
) {
    let mut urls = String::new();

    for res in results {
        urls.push_str(&format!("'{}', ", res.tarball_url));
    }

    // we may have all Errs, in which case we don't need to delete anything
    if !urls.is_empty() {
        // remove trailing comma and space
        urls.pop();
        urls.pop();

        let query =
            format!("DELETE FROM size_analysis_to_compute WHERE (tarball_url) IN ({urls})");
        conn.execute(diesel::sql_query(query)).unwrap();
    }
}
