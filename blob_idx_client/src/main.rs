use std::sync::Arc;

use blob_idx_server::{
    blob::BlobOffset,
    http::{BlobEntry, CreateAndLockRequest},
};
use tokio::{io::AsyncWriteExt, sync::Semaphore, task::JoinHandle};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let args: Vec<String> = std::env::args().collect();
    // args[1] is either "write" or "read"
    if args.len() < 3 {
        eprintln!("Usage: {} [write|read] ...", args[0]);
        std::process::exit(1);
    }
    match args[1].as_str() {
        "write" => download_and_write(args).await,
        "read" => todo!(),
        _ => {
            eprintln!("Usage: {} [write|read] ...", args[0]);
            std::process::exit(1);
        }
    };
}

async fn download_and_write(args: Vec<String>) {
    if args.len() != 4 {
        eprintln!(
            "Usage: {} write <discovery node id> <tarball urls, separated by spaces>",
            args[0]
        );
        std::process::exit(1);
    }

    let blob_api_url = std::env::var("BLOB_API_URL").expect("BLOB_API_URL must be set");
    let blob_api_key = std::env::var("BLOB_API_KEY").expect("BLOB_API_KEY must be set");
    let blob_storage_dir = std::env::var("BLOB_STORAGE_DIR").expect("BLOB_STORAGE_DIR must be set");

    let node_id: String = args[1].clone();
    let urls: Vec<String> = args[2].split(' ').map(|s| s.to_string()).collect();

    // download all tarballs
    let sem = Arc::new(Semaphore::new(10)); // max 10 concurrent downloads

    // the join handles will hold the name of the file and it's contents
    // the result, if failed, will return the url that failed
    let mut handles: Vec<JoinHandle<Result<(String, Vec<u8>), String>>> = vec![];

    for url in urls {
        let sem = Arc::clone(&sem);
        let url = url.clone();
        handles.push(tokio::task::spawn(async move {
            let _permit = sem.acquire().await.unwrap();
            let filename = url.split('/').last().unwrap();

            let mut resp = reqwest::get(&url).await.unwrap();
            // check if the response is not an error
            if !resp.status().is_success() {
                return Err(url);
            }

            let mut bytes = {
                // if we know the size of the response, we can pre-allocate the buffer, otherwise
                // we'll just use the default Vec::new()
                if let Some(size) = resp.content_length() {
                    Vec::with_capacity(size as usize)
                } else {
                    Vec::new()
                }
            };
            while let Some(chunk) = resp.chunk().await.unwrap() {
                bytes.extend_from_slice(&chunk);
            }

            Ok((filename.to_string(), bytes))
        }));
    }

    let mut failures = vec![];
    let mut blob_entries = vec![];
    let mut blob_bytes = vec![];
    for handle in handles {
        match handle.await.unwrap() {
            Ok((filename, bytes)) => {
                let blob_entry = BlobEntry {
                    key: filename.clone(),
                    num_bytes: bytes.len() as u64,
                };
                blob_entries.push(blob_entry);
                blob_bytes.push(bytes);
            }
            Err(url) => {
                failures.push(url);
            }
        }
    }

    // if we have 0 successes, we can't continue
    if blob_bytes.is_empty() {
        // TODO: print out the failed urls in json
        return;
    }

    // ask the blob api to lock
    let req = CreateAndLockRequest {
        entries: blob_entries,
        node_id,
    };
    let client = reqwest::Client::new();
    // TODO: implement keep-alive mechanism
    let resp = client
        .post(format!("{}/blob/create_and_lock", blob_api_url))
        .header("Authorization", blob_api_key)
        .json(&req)
        .send()
        .await
        .unwrap();

    // if we get a 200, we can continue
    if !resp.status().is_success() {
        // TODO: idk what to do here... maybe differentiate errors and print them out
        return;
    }

    // parse the response
    let blob: BlobOffset = resp.json().await.unwrap();

    // if blob.needs_creation is true, we need to create the blob file
    let mut file = if blob.needs_creation {
        let path = std::path::Path::new(&blob_storage_dir).join(&blob.file_name);
        // check if the file exists already, if so panic
        if path.exists() {
            panic!("Blob file already exists... this should never happen");
        }
        tokio::fs::File::create(&path).await.unwrap()
    } else {
        let path = std::path::Path::new(&blob_storage_dir).join(&blob.file_name);
        // open in append mode
        tokio::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .await
            .unwrap()
    };

    // write files in order of the blob entries
    for bytes in blob_bytes {
        file.write_all(&bytes).await.unwrap();
    }
}
