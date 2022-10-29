use blob_idx_server::http::HTTP;

#[tokio::main]
async fn main() {
    let http = HTTP::new("127.0.0.1".to_string(), "8080".to_string());
    // low value for testing
    http.start(2).await.expect("Failed to start http server");
}
