use blob_idx_server::{blob, http::HTTP};

#[tokio::main]
async fn main() {
    let http = HTTP::new("127.0.0.1".to_string(), "8080".to_string());
    let (tx, mut shutdown_signal) = tokio::sync::mpsc::channel::<()>(1);
    http.start(blob::BlobStorageConfig::default(), async move {
        shutdown_signal.recv().await;
    })
    .await
    .expect("Failed to start http server");
}
