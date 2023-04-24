use blob_idx_client::*;
use blob_idx_server::job::ClientResponse;

// NOTE: we can print to stderr for debugging purposes, but we should not print to stdout
// because we rely on the output of the client to be JSON.

#[tokio::main]
async fn main() {
    // The .secret.env has higher priority than .env, so we load it first
    dotenvy::from_filename(".secret.env").expect("failed to load .secret.env. Please create it");
    dotenvy::dotenv().ok();

    let args: Vec<String> = std::env::args().collect();
    // args[1] is either "write" or "read"
    if args.len() < 3 {
        eprintln!("Usage: {} [write|read|cp|compute|store] ...", args[0]);
        std::process::exit(1);
    }
    let resp = match args[1].as_str() {
        "write" => match download_and_write(args).await {
            Ok(_) => ClientResponse::Message(serde_json::json!({})),
            Err(e) => ClientResponse::Error(e),
        },
        "store" => match store_from_local(args).await {
            Ok(_) => ClientResponse::Message(serde_json::json!({})),
            Err(e) => ClientResponse::Error(e),
        },
        "read" => match read_and_send_main(args).await {
            Ok(o) => ClientResponse::Message(serde_json::Value::String(o)),
            Err(e) => ClientResponse::Error(e),
        },
        "cp" => match cp_main(args).await {
            Ok(_) => ClientResponse::Message(serde_json::json!({})),
            Err(e) => ClientResponse::Error(e),
        },
        "compute" => match compute_run_bin(args).await {
            Ok(o) => ClientResponse::Message(serde_json::to_value(o).unwrap()),
            Err(e) => ClientResponse::Error(e),
        },
        "compute_multi" => match compute_run_bin_multi(args).await {
            Ok(o) => ClientResponse::Message(serde_json::to_value(o).unwrap()),
            Err(e) => ClientResponse::Error(e),
        },
        _ => {
            eprintln!("Usage: {} [write|read|compute] ...", args[0]);
            std::process::exit(1);
        }
    };
    println!("{}", serde_json::to_string(&resp).unwrap());
}
