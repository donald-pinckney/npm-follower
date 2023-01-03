use std::{collections::HashMap, sync::Arc};

use blob_idx_server::{
    errors::{BlobError, ClientError, HTTPError},
    http::{JobType, SubmitJobRequest},
    job::{ClientResponse, TarballResult},
};
use diesel::QueryableByName;
use postgres_db::{
    connection::{DbConnection, DbConnectionInTransaction, QueryRunner},
    diff_analysis::{self, insert_diff_analysis, DiffAnalysis, DiffAnalysisJobResult, FileDiff},
    download_tarball::{self, DownloadedTarball},
    internal_state,
};
use serde::{Deserialize, Serialize};
use tokio::{
    sync::{
        mpsc::{self, Receiver, Sender},
        Mutex,
    },
    task::JoinHandle,
};

#[derive(Serialize, Deserialize, QueryableByName, Debug, Clone)]
struct QRes {
    #[sql_type = "diesel::sql_types::BigInt"]
    from_id: i64,
    #[sql_type = "diesel::sql_types::BigInt"]
    to_id: i64,
    #[sql_type = "diesel::sql_types::Text"]
    from_url: String,
    #[sql_type = "diesel::sql_types::Text"]
    to_url: String,
    #[sql_type = "diesel::sql_types::Text"]
    from_key: String,
    #[sql_type = "diesel::sql_types::Text"]
    to_key: String,
}

const QUERY: &str = r#"
SELECT * FROM analysis.diffs_to_compute
"#;

const CHUNK_SIZE: usize = 2500;
const NUM_WORKERS: usize = 50;
const NUM_LOCAL_WORKERS: usize = 3;
const TOTAL_NUM_DIFFS: usize = 16542717; // hardcoded... but only used for progress bar

#[tokio::main]
async fn main() {
    utils::check_no_concurrent_processes("diff_analysis");
    dotenvy::dotenv().ok();
    let mut conn: DbConnection = DbConnection::connect();
    let q = diesel::sql_query(QUERY);
    let res: Vec<QRes> = conn.load(q).unwrap();

    let (data_tx, data_rx) = tokio::sync::mpsc::channel(NUM_WORKERS);
    let data_rx = Arc::new(Mutex::new(data_rx));
    let (db_tx, db_rx) = tokio::sync::mpsc::channel(NUM_WORKERS);

    let mut chunk_workers = Vec::new();
    for id in 0..NUM_LOCAL_WORKERS {
        chunk_workers.push(spawn_diff_worker(data_rx.clone(), db_tx.clone(), id));
    }
    let db_worker = spawn_db_worker(db_rx, DbConnection::connect());

    let mut total = 0;
    for chunk in res.chunks(CHUNK_SIZE) {
        let chunk = chunk.to_vec();
        total += chunk.len();
        data_tx.send(chunk).await.unwrap();
        println!("[MANAGER] Progress: {}/{}", total, TOTAL_NUM_DIFFS);
    }

    println!("[MANAGER] DONE! Waiting for workers to finish...");
    drop(data_tx);

    for worker in chunk_workers {
        worker.await.unwrap();
    }

    drop(db_tx);
    db_worker.await.unwrap();
}

fn spawn_diff_worker(
    data_rx: Arc<Mutex<Receiver<Vec<QRes>>>>,
    db_tx: Sender<Vec<DiffAnalysis>>,
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

            let mut id_lookup: HashMap<&str, i64> = HashMap::new();
            let mut tarball_chunks = vec![];
            for chunk in chunk.chunks(CHUNK_SIZE / NUM_WORKERS) {
                let mut tb_chunk = vec![];
                for qres in chunk {
                    tb_chunk.push(vec![qres.from_key.to_string(), qres.to_key.to_string()]);
                    id_lookup.insert(&qres.from_key, qres.from_id);
                    id_lookup.insert(&qres.to_key, qres.to_id);
                }
                tarball_chunks.push(tb_chunk);
            }

            let job = SubmitJobRequest {
                job_type: JobType::ComputeMulti {
                    binary: "/scratch/cassano.f/blob_bins/diff_analysis_client".to_string(),
                    tarball_chunks,
                },
            };

            let client = reqwest::Client::new();
            println!("[{}] Submitting job", worker_id);
            let http_resp = client
                .post(&format!("{}/job/submit", blob_api_url))
                .header("Authorization", &blob_api_key)
                .json(&job)
                .send()
                .await
                .unwrap();
            let resps: Vec<ClientResponse> = http_resp.json().await.unwrap();
            let mut diffs = vec![];
            for resp in resps {
                match resp {
                    ClientResponse::Message(m) => {
                        let res = HashMap::<String, TarballResult>::deserialize(m)
                            .expect("Failed to deserialize");
                        for (tb_split, res) in res {
                            let (tb_old, tb_new) = tb_split.split_once('&').expect("Invalid split");
                            let from_id = id_lookup[&tb_old];
                            let to_id = id_lookup[&tb_new];
                            let job_result = if res.exit_code == 0 && !res.stdout.is_empty() {
                                let stdout = base64::decode(&res.stdout).expect("Failed to decode");
                                match serde_json::from_slice::<HashMap<String, FileDiff>>(&stdout) {
                                    Ok(res) => DiffAnalysisJobResult::Diff(res),
                                    Err(_) => DiffAnalysisJobResult::ErrUnParseable,
                                }
                            } else if res.exit_code == 103 {
                                let stderr = String::from_utf8(
                                    base64::decode(&res.stderr).expect("Failed to decode"),
                                )
                                .expect("Failed to decode");
                                let (old, new) =
                                    stderr.split_once(',').expect("Invalid comma split");
                                let old = old.parse::<usize>().expect("Failed to parse");
                                let new = new.parse::<usize>().expect("Failed to parse");
                                DiffAnalysisJobResult::ErrTooManyFiles(old, new)
                            } else {
                                DiffAnalysisJobResult::ErrClient(res.stderr)
                            };
                            diffs.push(DiffAnalysis {
                                from_id,
                                to_id,
                                job_result,
                            })
                        }
                    }
                    ClientResponse::Error(e) => {
                        eprintln!("[{}] Client Error: {}", worker_id, e);
                    }
                };
            }
            db_tx.send(diffs).await.unwrap();
        }
    };
    tokio::task::spawn(thunk)
}

fn spawn_db_worker(
    mut db_rx: Receiver<Vec<DiffAnalysis>>,
    mut conn: DbConnection,
) -> JoinHandle<()> {
    let thunk = async move {
        while let Some(diffs) = db_rx.recv().await {
            println!("[DB] Inserting {} diffs", diffs.len());
            if !diffs.is_empty() {
                conn.run_psql_transaction(|mut c| {
                    delete_rows_after_compute(&diffs, &mut c);
                    diff_analysis::insert_batch_diff_analysis(&mut c, diffs)?;
                    Ok(((), true))
                })
                .expect("Failed to insert diffs");
            }
            println!("[DB] Done");
        }
    };
    tokio::task::spawn(thunk)
}

fn delete_rows_after_compute(diffs: &[DiffAnalysis], conn: &mut DbConnectionInTransaction) {
    let mut pairs = String::new();
    for diff in diffs.iter() {
        if matches!(
            diff.job_result,
            DiffAnalysisJobResult::Diff(_) | DiffAnalysisJobResult::ErrTooManyFiles(_, _)
        ) {
            pairs.push_str(&format!("({}, {})", diff.from_id, diff.to_id));
            pairs.push_str(", ");
        } else {
            println!("[DB] Skipping delete for {:?}", diff);
        }
    }

    // we may have all Errs, in which case we don't need to delete anything
    if !pairs.is_empty() {
        // remove trailing comma and space
        pairs.pop();
        pairs.pop();

        // we have to delete (from_id, to_id) pairs, as alone they are not unique
        let query = format!(
            "DELETE FROM analysis.diffs_to_compute WHERE (from_id, to_id) IN ({})",
            pairs
        );
        conn.execute(diesel::sql_query(query)).unwrap();
    }
}
