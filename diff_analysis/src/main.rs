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
    let mut conn: DbConnection = DbConnection::connect();
    let q = diesel::sql_query(QUERY);
    let res: Vec<QRes> = conn.load(q).unwrap();

    // let mut lookup: HashMap<(i64, i64), (String, String)> = HashMap::new();
    let mut i = 0;
    for chunk in res.chunks(CHUNK_SIZE) {
        println!("Starting chunk {} - {}", i, chunk.len());
        i += 1;
    }
}
