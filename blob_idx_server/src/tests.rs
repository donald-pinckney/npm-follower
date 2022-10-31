use lazy_static::lazy_static;
use tokio::task::JoinHandle;

use crate::{
    blob::{BlobOffset, BlobStorageConfig, BlobStorageSlice},
    http::{BlobEntry, CreateAndLockRequest, CreateUnlockRequest, LookupRequest, HTTP},
};

lazy_static! {
    static ref GLOBAL_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::new(());
}

macro_rules! blob_test {
    ($body:block, $cfg:expr) => {
        let _lock = GLOBAL_LOCK.lock().await;
        redis_cleanup();
        let server = run_test_server($cfg).await; // we have 1 file max

        // wait for the server to start
        while let Err(_) = reqwest::get("http://127.0.0.1:1337/").await {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }

        $body;
        server.shutdown().await;
        drop(_lock);
    };
    // default config
    ($body:block) => {
        blob_test!($body, make_config(1, 5));
    };
}

fn make_config(max_files: u32, lock_timeout: u64) -> BlobStorageConfig {
    BlobStorageConfig {
        redis_url: "redis://127.0.0.1/5".to_string(), // NOTE: we use db 5 for testing
        max_files,
        lock_timeout,
    }
}

fn simple_config() -> BlobStorageConfig {
    make_config(2, 5)
}

fn redis_cleanup() {
    let client = redis::Client::open("redis://127.0.0.1/5").unwrap();
    let mut con = client.get_connection().unwrap();
    redis::cmd("FLUSHDB").query::<()>(&mut con).unwrap();
}

struct TestServer {
    shutdown_signal: tokio::sync::mpsc::Sender<()>,
    handle: JoinHandle<()>,
}

impl TestServer {
    async fn shutdown(self) {
        self.shutdown_signal.try_send(()).unwrap();
        self.handle.await.unwrap();
    }
}

async fn run_test_server(cfg: BlobStorageConfig) -> TestServer {
    let http = HTTP::new("127.0.0.1".to_string(), "1337".to_string());
    let (tx, mut rx) = tokio::sync::mpsc::channel::<()>(1);

    let task = tokio::spawn(async move {
        http.start(cfg.clone(), async move {
            rx.recv().await;
        })
        .await
        .unwrap()
    });

    TestServer {
        shutdown_signal: tx,
        handle: task,
    }
}

async fn send_create_and_lock_request(
    client: &reqwest::Client,
    req: CreateAndLockRequest,
) -> Result<BlobOffset, String> {
    let resp = client
        .post("http://127.0.0.1:1337/create_and_lock")
        .body(serde_json::to_string(&req).unwrap())
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    serde_json::from_str::<BlobOffset>(&resp).map_err(|_| resp)
}

async fn send_create_unlock_request(
    client: &reqwest::Client,
    req: CreateUnlockRequest,
) -> (reqwest::StatusCode, String) {
    let resp = client
        .post("http://127.0.0.1:1337/create_unlock")
        .body(serde_json::to_string(&req).unwrap())
        .send()
        .await
        .unwrap();

    (resp.status(), resp.text().await.unwrap())
}

async fn send_lookup_request(
    client: &reqwest::Client,
    req: LookupRequest,
) -> Result<BlobStorageSlice, String> {
    let resp = client
        .get("http://127.0.0.1:1337/lookup")
        .body(serde_json::to_string(&req).unwrap())
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    serde_json::from_str::<BlobStorageSlice>(&resp).map_err(|_| resp)
}

#[tokio::test]
async fn test_simple_get_slice_unlock_lookup() {
    let client = reqwest::Client::new();
    blob_test!({
        let offset = send_create_and_lock_request(
            &client,
            CreateAndLockRequest {
                entries: vec![
                    BlobEntry::new("k1".to_string(), 1),
                    BlobEntry::new("k2".to_string(), 2),
                ],
                node_id: "n1".to_string(),
            },
        )
        .await
        .unwrap();
        assert_eq!(offset.file_id, 0); // picks the first file
        assert_eq!(offset.byte_offset, 0); // starts at 0

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        // unlock
        let resp = send_create_unlock_request(
            &client,
            CreateUnlockRequest {
                file_id: offset.file_id,
                node_id: "n1".to_string(),
            },
        )
        .await;
        println!("resp: {}", resp.1);
        assert_eq!(resp.0, 200);

        // lookup
        let slice = send_lookup_request(
            &client,
            LookupRequest {
                key: "k1".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(slice.file_id, 0);
        assert_eq!(slice.byte_offset, 0);
        assert_eq!(slice.num_bytes, 1);

        // lock again
        let offset = send_create_and_lock_request(
            &client,
            CreateAndLockRequest {
                entries: vec![
                    BlobEntry::new("k3".to_string(), 1),
                    BlobEntry::new("k4".to_string(), 2),
                ],
                node_id: "n1".to_string(),
            },
        )
        .await
        .unwrap();
        assert_eq!(offset.file_id, 0); // picks the first file
        assert_eq!(offset.byte_offset, 3); // starts at 3

        // unlock
        let resp = send_create_unlock_request(
            &client,
            CreateUnlockRequest {
                file_id: offset.file_id,
                node_id: "n1".to_string(),
            },
        )
        .await;
        println!("resp: {}", resp.1);
        assert_eq!(resp.0, 200);

        // lookup
        let slice = send_lookup_request(
            &client,
            LookupRequest {
                key: "k4".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(slice.file_id, 0);
        assert_eq!(slice.byte_offset, 4);
        assert_eq!(slice.num_bytes, 2);
    });
}

#[tokio::test]
/// This tests what happens when all files are locked, and multiple nodes try to lock
async fn test_lock_wait() {
    let client = reqwest::Client::new();
    blob_test!({
        // n1 initially locks, gets the lock
        let client1 = client.clone();
        let handle1 = tokio::task::spawn(async move {
            let now = std::time::Instant::now();
            let res = send_create_and_lock_request(
                &client1,
                CreateAndLockRequest {
                    entries: vec![BlobEntry::new("k1".to_string(), 1)],
                    node_id: "n1".to_string(),
                },
            )
            .await;
            (res, now.elapsed())
        });

        // n2 waits for lock
        let client2 = client.clone();
        let handle2 = tokio::task::spawn(async move {
            let now = std::time::Instant::now();
            let res = send_create_and_lock_request(
                &client2,
                CreateAndLockRequest {
                    entries: vec![BlobEntry::new("k2".to_string(), 3)],
                    node_id: "n2".to_string(),
                },
            )
            .await;
            (res, now.elapsed())
        });

        // n3 waits for lock
        let client3 = client.clone();
        let handle3 = tokio::task::spawn(async move {
            let now = std::time::Instant::now();
            let res = send_create_and_lock_request(
                &client3,
                CreateAndLockRequest {
                    entries: vec![BlobEntry::new("k3".to_string(), 10)],
                    node_id: "n3".to_string(),
                },
            )
            .await;
            (res, now.elapsed())
        });

        // wait for first
        let (o1, time1) = handle1.await.unwrap();
        let o1 = o1.unwrap();

        // unlock first
        let resp = send_create_unlock_request(
            &client,
            CreateUnlockRequest {
                file_id: 0,
                node_id: "n1".to_string(),
            },
        )
        .await;

        println!("resp: {}", resp.1);
        assert_eq!(resp.0, 200);

        // wait for second
        let (o2, time2) = handle2.await.unwrap();
        let o2 = o2.unwrap();

        // unlock n2
        let resp = send_create_unlock_request(
            &client,
            CreateUnlockRequest {
                file_id: 0,
                node_id: "n2".to_string(),
            },
        )
        .await;

        println!("resp: {}", resp.1);
        assert_eq!(resp.0, 200);

        // wait for third
        let (o3, time3) = handle3.await.unwrap();
        let o3 = o3.unwrap();

        // unlock n3
        let resp = send_create_unlock_request(
            &client,
            CreateUnlockRequest {
                file_id: 0,
                node_id: "n3".to_string(),
            },
        )
        .await;

        // check that they indeed waited
        assert!(time1 < time2);
        assert!(time2 < time3);
        println!("time1: {:?}", time1);
        println!("time2: {:?}", time2);
        println!("time3: {:?}", time3);

        println!("resp: {}", resp.1);
        assert_eq!(resp.0, 200);

        // check offsets (needs creation)
        assert_eq!(o1.file_id, 0);
        assert!(o1.needs_creation);
        assert_eq!(o1.byte_offset, 0);

        assert_eq!(o2.file_id, 0);
        assert!(!o2.needs_creation);
        assert_eq!(o2.byte_offset, 1);

        assert_eq!(o3.file_id, 0);
        assert!(!o3.needs_creation);
        assert_eq!(o3.byte_offset, 4);

        // -------------------------------------------------------
        // now that everything is unlocked, we redo the same thing
        // -------------------------------------------------------

        // n1 initially locks, gets the lock
        let client1 = client.clone();
        let handle1 = tokio::task::spawn(async move {
            send_create_and_lock_request(
                &client1,
                CreateAndLockRequest {
                    entries: vec![BlobEntry::new("k4".to_string(), 1)],
                    node_id: "n1".to_string(),
                },
            )
            .await
        });

        // n2 waits for lock
        let client2 = client.clone();
        let handle2 = tokio::task::spawn(async move {
            send_create_and_lock_request(
                &client2,
                CreateAndLockRequest {
                    entries: vec![BlobEntry::new("k5".to_string(), 3)],
                    node_id: "n2".to_string(),
                },
            )
            .await
        });

        // n3 waits for lock
        let client3 = client.clone();
        let handle3 = tokio::task::spawn(async move {
            send_create_and_lock_request(
                &client3,
                CreateAndLockRequest {
                    entries: vec![BlobEntry::new("k6".to_string(), 10)],
                    node_id: "n3".to_string(),
                },
            )
            .await
        });

        // wait for first
        let o1 = handle1.await.unwrap().unwrap();

        // unlock first
        let resp = send_create_unlock_request(
            &client,
            CreateUnlockRequest {
                file_id: 0,
                node_id: "n1".to_string(),
            },
        )
        .await;

        println!("resp: {}", resp.1);
        assert_eq!(resp.0, 200);

        // wait for second
        let o2 = handle2.await.unwrap().unwrap();

        // unlock n2
        let resp = send_create_unlock_request(
            &client,
            CreateUnlockRequest {
                file_id: 0,
                node_id: "n2".to_string(),
            },
        )
        .await;

        println!("resp: {}", resp.1);
        assert_eq!(resp.0, 200);

        // wait for third
        let o3 = handle3.await.unwrap().unwrap();

        // unlock n3
        let resp = send_create_unlock_request(
            &client,
            CreateUnlockRequest {
                file_id: 0,
                node_id: "n3".to_string(),
            },
        )
        .await;

        println!("resp: {}", resp.1);
        assert_eq!(resp.0, 200);

        // check offsets (now doesn't need creation)
        assert_eq!(o1.file_id, 0);
        assert!(!o1.needs_creation);
        assert_eq!(o1.byte_offset, 14);

        assert_eq!(o2.file_id, 0);
        assert!(!o2.needs_creation);
        assert_eq!(o2.byte_offset, 15);

        assert_eq!(o3.file_id, 0);
        assert!(!o3.needs_creation);
        assert_eq!(o3.byte_offset, 18);
    });
}
