use std::{collections::HashMap, sync::Arc};

use blob_idx_server::{
    errors::{BlobError, ClientError, HTTPError},
    http::{JobType, SubmitJobRequest},
    job::ClientResponse,
};
use diesel::QueryableByName;
use postgres_db::{
    connection::{DbConnection, QueryRunner},
    diff_analysis::{insert_diff_analysis, DiffAnalysis, DiffAnalysisJobResult, FileDiff},
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
SELECT * FROM analysis.diffs_to_compute LIMIT(5000)
"#;

const CHUNK_SIZE: usize = 500;
const NUM_WORKERS: usize = 50;
const NUM_LOCAL_WORKERS: usize = 5;

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

    for chunk in res.chunks(CHUNK_SIZE) {
        let chunk = chunk.to_vec();
        data_tx.send(chunk).await.unwrap();
    }

    println!("DONE! Waiting for workers to finish...");
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
            let resp = client
                .post(&format!("{}/job/submit", blob_api_url))
                .header("Authorization", &blob_api_key)
                .json(&job)
                .send()
                .await
                .unwrap();
            println!("[{}] resp: {:?}", worker_id, resp.text().await);
        }
    };
    tokio::task::spawn(thunk)
}

fn spawn_db_worker(mut db_rx: Receiver<Vec<DiffAnalysis>>, mut db: DbConnection) -> JoinHandle<()> {
    let thunk = async move {
        while let Some(diffs) = db_rx.recv().await {
            println!("Inserting {} diffs", diffs.len());
        }
    };
    tokio::task::spawn(thunk)
}
