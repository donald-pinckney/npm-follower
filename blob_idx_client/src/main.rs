use std::sync::Arc;

use blob_idx_server::{
    blob::BlobOffset,
    errors::ClientError,
    http::{BlobEntry, CreateAndLockRequest, CreateUnlockRequest, KeepAliveLockRequest},
    job::ClientResponse,
};
use tokio::{
    io::{AsyncSeekExt, AsyncWriteExt},
    sync::Semaphore,
    task::JoinHandle,
};

// NOTE: we can print to stderr for debugging purposes, but we should not print to stdout
// because we rely on the output of the client to be JSON.

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let args: Vec<String> = std::env::args().collect();
    // args[1] is either "write" or "read"
    if args.len() < 3 {
        eprintln!("Usage: {} [write|read] ...", args[0]);
        std::process::exit(1);
    }
    let res = match args[1].as_str() {
        "write" => download_and_write(args).await,
        "read" => todo!(),
        _ => {
            eprintln!("Usage: {} [write|read] ...", args[0]);
            std::process::exit(1);
        }
    };
    let resp = match res {
        Ok(_) => ClientResponse::Success,
        Err(e) => ClientResponse::Failure(e),
    };
    println!("{}", serde_json::to_string(&resp).unwrap());
}

fn spawn_keep_alive_loop(file_id: u32) -> JoinHandle<()> {
    tokio::task::spawn(async move {
        let blob_api_url = std::env::var("BLOB_API_URL").expect("BLOB_API_URL must be set");
        let blob_api_key = std::env::var("BLOB_API_KEY").expect("BLOB_API_KEY must be set");
        let req = KeepAliveLockRequest { file_id };
        let client = reqwest::Client::new();
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(300)).await;
            eprintln!("Sending keep-alive request");

            let res = client
                .post(format!("{}/blob/keep_alive_lock", blob_api_url))
                .header("Authorization", blob_api_key.clone())
                .json(&req)
                .send()
                .await;

            if res.is_err() {
                break;
            }
        }
    })
}

async fn download_and_write(args: Vec<String>) -> Result<(), ClientError> {
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

    let node_id: String = args[2].clone();
    let urls: Vec<String> = args[3].split(' ').map(|s| s.to_string()).collect();

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
            eprintln!("Downloading {}", url);
            let mut resp = match reqwest::get(&url).await {
                Ok(r) => r,
                Err(_) => return Err(url),
            };
            drop(_permit);
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
            while let Some(chunk) = match resp.chunk().await {
                Ok(c) => c,
                Err(_) => return Err(url), // presumably an io error
            } {
                bytes.extend_from_slice(&chunk);
            }

            Ok((url, bytes))
        }));
    }

    let mut failures = vec![];
    let mut blob_entries = vec![];
    let mut blob_bytes = vec![];
    for handle in handles {
        match handle.await.unwrap() {
            Ok((url, bytes)) => {
                let blob_entry = BlobEntry {
                    key: url.clone(),
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
        return Err(ClientError::DownloadFailed { urls: failures });
    }

    // ask the blob api to lock
    let req = CreateAndLockRequest {
        entries: blob_entries,
        node_id: node_id.clone(),
    };
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/blob/create_and_lock", blob_api_url))
        .header("Authorization", blob_api_key.clone())
        .json(&req)
        .send()
        .await?;

    // if we get a 200, we can continue
    if !resp.status().is_success() {
        return Err(ClientError::BlobCreateLockError);
    }

    // parse the response
    let blob: BlobOffset = resp
        .json()
        .await
        .map_err(|_| ClientError::BlobCreateLockError)?;

    let keep_alive = spawn_keep_alive_loop(blob.file_id);

    let path = std::path::Path::new(&blob_storage_dir).join(&blob.file_name);
    let offset_path = path.with_extension("offset");
    // if blob.needs_creation is true, we need to create the blob file
    let (mut blob_file, mut offset_file) = if blob.needs_creation {
        // check if the file exists already, if so panic
        if path.exists() || offset_path.exists() {
            panic!("Blob file already exists... this should never happen");
        }
        (
            tokio::fs::File::create(&path).await?,
            tokio::fs::File::create(&offset_path).await?,
        )
    } else {
        (
            // open in write mode.
            tokio::fs::OpenOptions::new()
                .write(true)
                .open(&path)
                .await?,
            // open in append mode
            tokio::fs::OpenOptions::new()
                .append(true)
                .open(&path.with_extension("offset"))
                .await?,
        )
    };
    // fseek to the offset given by the blob api
    blob_file
        .seek(std::io::SeekFrom::Start(blob.byte_offset))
        .await?;

    // write offset to the offset file
    offset_file
        .write_all(format!("\"{}\": {}\n", args[3], blob.byte_offset).as_bytes())
        .await?;

    // write files in order of the blob entries
    for bytes in blob_bytes {
        blob_file.write_all(&bytes).await?;
    }

    // unlock the blob
    let req = CreateUnlockRequest {
        file_id: blob.file_id,
        node_id,
    };

    let resp = client
        .post(format!("{}/blob/create_unlock", blob_api_url))
        .header("Authorization", blob_api_key)
        .json(&req)
        .send()
        .await?;

    // if we get a 200, we can continue
    if !resp.status().is_success() {
        return Err(ClientError::BlobUnlockError);
    }

    if !failures.is_empty() {
        return Err(ClientError::DownloadFailed { urls: failures });
    }

    keep_alive.abort();
    Ok(())
}
