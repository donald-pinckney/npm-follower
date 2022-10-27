use std::sync::Arc;

use dashmap::{DashMap, DashSet};
use rand::Rng;
use redis::Commands;
use serde::{Deserialize, Serialize, __private::de::FlatInternallyTaggedAccess};
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobStorageSlice {
    pub file_id: u32,
    pub byte_offset: u64,
    pub num_bytes: u64,
    pub needs_creation: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LockWrapper {
    slice: BlobStorageSlice,
    /// Read-read is always allowed.
    /// Write-write on the same chunk file is not allowed.
    /// Read-write on the same chunk file is allowed for the same node_id for different keys.
    /// Read-write on the same chunk file and same key is not allowed.
    lock: Option<String>, // the string is the node_id
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileInfo {
    size: u64,
    node_id: String,
    file_id: u32,
    file_name: String,
}

/// A thread-safe blob storage API.
/// Clone is cheap as it is reference counted.
#[derive(Clone)]
pub struct BlobStorage {
    redis: Arc<Mutex<redis::Connection>>,
    map: Arc<DashMap<String, LockWrapper>>, // map [key] -> [slice + lock]
    /// pool of all the files
    file_pool: Arc<DashMap<u32, FileInfo>>,
    /// pool of all the files that are currently being written to
    locked_files: Arc<DashSet<u32>>,
    /// lock for picking a new file id
    file_lock: Arc<Mutex<()>>,
}

/// INFO: https://github.com/donald-pinckney/npm-follower/wiki/Design-of-the-Blob-Storage-Index-Server
impl BlobStorage {
    /// NOTE: with redis, on new we fully load the file pools.
    /// meanwhile for the k/v map, we lazily load it on first access.
    pub async fn new() -> BlobStorage {
        dotenvy::dotenv().ok();
        let redis = redis::Client::open(std::env::var("BLOB_REDIS_URL").expect("BLOB_REDIS_URL"))
            .expect("redis client");
        let con = redis.get_connection().unwrap();
        // TODO: actually load file pools
        BlobStorage {
            redis: Arc::new(Mutex::new(con)),
            map: Arc::new(DashMap::new()),
            file_pool: Arc::new(DashMap::new()),
            locked_files: Arc::new(DashSet::new()),
            file_lock: Arc::new(Mutex::new(())),
        }
    }

    async fn map_lookup(&self, key: &str) -> Result<LockWrapper, BlobError> {
        // there are two keys that are prohibited:
        // __file_pool__ and __locked_files__
        if key == "__file_pool__" || key == "__locked_files__" {
            return Err(BlobError::ProhibitedKey);
        }

        // first, check the in-memory map
        if let Some(v) = self.map.get(key) {
            return Ok(v.value().clone());
        }

        // if not found, check the redis map, and load it into the in-memory map
        let mut redis = self.redis.lock().await;
        let v: Option<String> = redis.get(key).unwrap();
        if let Some(v) = v {
            // serialize the string into a LockWrapper
            let v: LockWrapper = serde_json::from_str(&v).unwrap();
            self.map.insert(key.to_string(), v);
            Ok(self.map.get(key).unwrap().value().clone())
        } else {
            Err(BlobError::DoesNotExist)
        }
    }

    pub async fn create_and_lock(
        &self,
        key: String,
        num_bytes: u64,
        node_id: String,
    ) -> Result<BlobStorageSlice, BlobError> {
        // check that key does not exist already
        if self.map_lookup(&key).await.is_ok() {
            return Err(BlobError::AlreadyExists);
        }

        // picks random file id from 0 to 999, and checks if it is locked.
        // if it is locked, picks another one, then locks the file.
        // returns the id of the file and a boolean that represents if the file needs to be created.
        let (file_id, needs_creation) = {
            let _guard = self.file_lock.lock().await;
            let mut rng = rand::thread_rng();
            loop {
                let file_id = rng.gen_range(0..1000);
                if !self.locked_files.contains(&file_id) {
                    // we lock the file
                    self.locked_files.insert(file_id);
                    let _: () = self
                        .redis
                        .lock()
                        .await
                        .sadd("__locked_files__", file_id)
                        .unwrap();
                    // check if file exists already
                    if self.file_pool.contains_key(&file_id) {
                        break (file_id, false);
                    } else {
                        // if not, we create a new file
                        let file_name = format!("blob_{}.bin", file_id);
                        let file_info = FileInfo {
                            size: 0,
                            node_id: node_id.clone(),
                            file_id,
                            file_name,
                        };
                        self.file_pool.insert(file_id, file_info.clone());
                        let _: () = self
                            .redis
                            .lock()
                            .await
                            .sadd("__file_pool__", serde_json::to_string(&file_info).unwrap())
                            .unwrap();
                        break (file_id, true);
                    }
                }
            }
        };

        // get mut the file info
        let mut file_info = self.file_pool.get_mut(&file_id).unwrap();
        let byte_offset = file_info.value().size;
        let slice = BlobStorageSlice {
            file_id,
            byte_offset,
            num_bytes,
            needs_creation,
        };
        let lock_wrapper = LockWrapper {
            slice: slice.clone(),
            lock: Some(node_id),
        };
        file_info.size += num_bytes;

        // insert into the map
        self.map.insert(key.clone(), lock_wrapper.clone());
        let _: () = self
            .redis
            .lock()
            .await
            .set(key, serde_json::to_string(&lock_wrapper).unwrap())
            .unwrap();

        Ok(slice)
    }

    pub async fn keep_alive_create_lock(
        &self,
        key: String,
        node_id: String,
    ) -> Result<(), BlobError> {
        todo!()
    }

    pub async fn create_unlock(&self, key: String, node_id: String) -> Result<(), BlobError> {
        todo!()
    }

    pub async fn lookup(&self, key: String) -> Result<BlobStorageSlice, BlobError> {
        let v = self.map_lookup(&key).await?;
        Ok(v.slice)
    }

    pub async fn read_lock(&self, key: String, node_id: String) -> Result<(), BlobError> {
        todo!()
    }

    pub async fn keep_alive_read_lock(
        &self,
        key: String,
        node_id: String,
    ) -> Result<(), BlobError> {
        todo!()
    }

    pub async fn read_unlock(&self, key: String, node_id: String) -> Result<(), BlobError> {
        todo!()
    }
}

#[derive(Debug)]
pub enum BlobError {
    AlreadyExists,
    CreateNotLocked,
    DoesNotExist,
    ReadNotLocked,
    ProhibitedKey,
}

impl std::fmt::Display for BlobError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlobError::AlreadyExists => write!(f, "Blob already exists"),
            BlobError::CreateNotLocked => write!(f, "Blob create not locked"),
            BlobError::DoesNotExist => write!(f, "Blob does not exist"),
            BlobError::ReadNotLocked => write!(f, "Blob read not locked"),
            BlobError::ProhibitedKey => write!(f, "Blob key is prohibited"),
        }
    }
}

impl std::error::Error for BlobError {}
