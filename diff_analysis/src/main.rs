use std::{collections::HashMap, sync::Arc};

use blob_idx_server::{
    errors::{BlobError, ClientError, HTTPError},
    http::{JobType, SubmitJobRequest},
    job::ClientResponse,
};
use diesel::QueryableByName;
use postgres_db::{
    connection::{DbConnection, QueryRunner},
    diff_analysis::{insert_diff_analysis, DiffAnalysisJobResult, FileDiff},
    download_tarball::{self, DownloadedTarball},
    internal_state,
};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Mutex};

#[derive(Serialize, Deserialize, QueryableByName, Debug)]
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
const NUM_WORKERS: usize = 10;

#[tokio::main]
async fn main() {
    utils::check_no_concurrent_processes("diff_analysis");
    dotenvy::dotenv().ok();
    let BLOB_API_URL = std::env::var("BLOB_API_URL").expect("BLOB_API_URL not set");
    let BLOB_API_KEY = std::env::var("BLOB_API_KEY").expect("BLOB_API_KEY not set");
    let mut conn: DbConnection = DbConnection::connect();
    let q = diesel::sql_query(QUERY);
    let res: Vec<QRes> = conn.load(q).unwrap();

    for chunk in res.chunks(CHUNK_SIZE) {
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
                binary: "/scratch/cassano.f/blob_bins/npm_tarball_diff.sh".to_string(),
                tarball_chunks,
            },
        };

        let client = reqwest::Client::new();
        println!("Submitting job");
        let resp = client
            .post(&format!("{}/job/submit", BLOB_API_URL))
            .header("Authorization", &BLOB_API_KEY)
            .json(&job)
            .send()
            .await
            .unwrap();
        println!("resp: {:?}", resp.text().await);
    }
}
