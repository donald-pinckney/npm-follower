use std::{collections::HashSet, sync::Arc};

use dashmap::DashMap;
use rand::{seq::SliceRandom, Rng};
use redis::Commands;
use serde::{ser::SerializeStruct, Deserialize, Serialize};
use tokio::sync::{Mutex, Notify};

/// A slice containing the information of a blob, linked to a key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobStorageSlice {
    pub file_id: u32,
    pub file_name: String,
    pub byte_offset: u64,
    pub num_bytes: u64,
}

/// A byte offset on which to write a blob.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobOffset {
    pub file_id: u32,
    pub file_name: String,
    pub byte_offset: u64,
    pub needs_creation: bool,
}

/// A key to number of bytes mapping.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobEntry {
    pub key: String,
    pub num_bytes: u64,
}

impl BlobEntry {
    pub fn new(key: String, num_bytes: u64) -> Self {
        Self { key, num_bytes }
    }
}

// CONCURRENCY NOTES:
// Read-read is always allowed.
// Write-write on the same chunk file is not allowed.
// Read-write on the same chunk file is allowed for the same node_id for different keys.
// Read-write on the same chunk file and same key is not allowed.

/// Wrapper for a blob storage slice. Has a notifier that is notified
/// when the slice is no longer locked.
#[derive(Debug, Clone)]
struct LockWrapper {
    slice: BlobStorageSlice,
    written: bool,
    lock: Option<String>,
}

impl Serialize for LockWrapper {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("LockWrapper", 2)?;
        state.serialize_field("slice", &self.slice)?;
        state.serialize_field("written", &self.written)?;
        // don't serialize the lock
        state.end()
    }
}

impl<'a> Deserialize<'a> for LockWrapper {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'a>,
    {
        #[derive(Deserialize)]
        struct LockWrapperHelper {
            slice: BlobStorageSlice,
            written: bool,
        }

        let helper = LockWrapperHelper::deserialize(deserializer)?;
        Ok(LockWrapper {
            slice: helper.slice,
            written: helper.written,
            lock: None,
        })
    }
}

#[derive(Debug, Clone)]
struct FileInfo {
    size: u64,
    file_id: u32,
    file_name: String,
    unlock_notify: Arc<Notify>,
}

impl Serialize for FileInfo {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("FileInfo", 3)?;
        state.serialize_field("size", &self.size)?;
        state.serialize_field("file_id", &self.file_id)?;
        state.serialize_field("file_name", &self.file_name)?;
        // don't serialize the unlock_notify
        state.end()
    }
}

impl<'a> Deserialize<'a> for FileInfo {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'a>,
    {
        #[derive(Deserialize)]
        struct FileInfoHelper {
            size: u64,
            file_id: u32,
            file_name: String,
        }

        let helper = FileInfoHelper::deserialize(deserializer)?;
        Ok(FileInfo {
            size: helper.size,
            file_id: helper.file_id,
            file_name: helper.file_name,
            unlock_notify: Arc::new(Notify::new()),
        })
    }
}

#[derive(Debug, Clone)]
struct FileLock {
    node_id: String,
    keys: Vec<String>, // locked keys
}

/// A thread-safe blob storage API.
/// Clone is cheap as it is reference counted.
#[derive(Clone)]
pub struct BlobStorage {
    redis: Arc<Mutex<redis::Connection>>,
    map: Arc<DashMap<String, LockWrapper>>, // map [key] -> [slice + lock]
    /// pool of all the files (locked or not)
    file_pool: Arc<DashMap<u32, FileInfo>>, // TODO: could do an array but i'm lazy now
    /// pool of all the files that are currently being written to.
    /// maps to a notifier that is notified when the file is write-unlocked.
    locked_files: Arc<DashMap<u32, FileLock>>,
    /// lock for picking a new file id
    file_lock: Arc<Mutex<()>>,
    /// The maximum number of chunk files to use.
    max_files: u32,
}

/// INFO: https://github.com/donald-pinckney/npm-follower/wiki/Design-of-the-Blob-Storage-Index-Server
impl BlobStorage {
    /// NOTE: with redis, on new we fully load the file pools.
    /// meanwhile for the k/v map, we lazily load it on first access.
    pub async fn init(max_files: u32) -> BlobStorage {
        dotenvy::dotenv().ok();
        let redis = redis::Client::open(std::env::var("BLOB_REDIS_URL").expect("BLOB_REDIS_URL"))
            .expect("redis client");
        let mut con = redis.get_connection().unwrap();
        // load file pool from redis
        let file_pool = {
            if con.hlen("__file_pool__") != Ok(0) {
                let set = con
                    .hgetall::<String, Vec<String>>("__file_pool__".to_string())
                    .unwrap();
                let file_pool = DashMap::new();
                for (i, v) in set.iter().enumerate() {
                    // skip evens because of how redis hashes work lol
                    if i % 2 == 1 {
                        let file_info: FileInfo = serde_json::from_str(v).unwrap();
                        file_pool.insert(file_info.file_id, file_info);
                    }
                }
                Arc::new(file_pool)
            } else {
                Arc::new(DashMap::new())
            }
        };

        BlobStorage {
            redis: Arc::new(Mutex::new(con)),
            map: Arc::new(DashMap::new()),
            file_pool,
            locked_files: Arc::new(DashMap::new()),
            file_lock: Arc::new(Mutex::new(())),
            max_files,
        }
    }

    pub(crate) async fn debug_print(&self, prefix: &str) {
        println!("{}", prefix);
        let mut map_str = String::new();
        for a in self.map.iter() {
            map_str.push_str(&format!(
                "\t\t{}: ({}, {})\n",
                a.key(),
                serde_json::to_string(a.value()).unwrap(),
                if a.value().lock.is_some() {
                    a.value().lock.as_ref().unwrap().to_string()
                } else {
                    "free".to_string()
                }
            ));
        }
        println!("\tmap:\n{}", map_str);

        let mut file_pool_str = String::new();
        for a in self.file_pool.iter() {
            file_pool_str.push_str(&format!(
                "\t\t{}: {}\n",
                a.key(),
                serde_json::to_string(a.value()).unwrap()
            ));
        }

        println!("\tfile_pool:\n{}", file_pool_str);

        let mut locked_files_str = String::new();
        for a in self.locked_files.iter() {
            locked_files_str.push_str(&format!("\t\t{}: {:?}\n", a.key(), a.value()));
        }

        println!("\tlocked_files:\n{}\n", locked_files_str);
    }

    async fn map_lookup(&self, key: &str) -> Result<LockWrapper, BlobError> {
        // there is a key that is prohitibed from being used:
        // - __file_pool__
        if key == "__file_pool__" {
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

    async fn map_lookup_mut(
        &self,
        key: &str,
    ) -> Result<dashmap::mapref::one::RefMut<'_, String, LockWrapper>, BlobError> {
        if key == "__file_pool__" {
            return Err(BlobError::ProhibitedKey);
        }

        // first, check the in-memory map
        if self.map.contains_key(key) {
            return Ok(self.map.get_mut(key).unwrap());
        }

        // if not found, check the redis map, and load it into the in-memory map
        let v: Option<String> = {
            let mut redis = self.redis.lock().await;
            redis.get(key).unwrap()
        };
        if let Some(v) = v {
            // serialize the string into a LockWrapper
            let v: LockWrapper = serde_json::from_str(&v).unwrap();
            self.map.insert(key.to_string(), v);
            Ok(self.map.get_mut(key).unwrap())
        } else {
            Err(BlobError::DoesNotExist)
        }
    }

    /// Adds/sets the file info in the file pool in redis.
    async fn add_to_redis_filepool(&self, file_info: &FileInfo) {
        let _: () = self
            .redis
            .lock()
            .await
            .hset(
                "__file_pool__",
                file_info.file_id,
                serde_json::to_string(file_info).unwrap(),
            )
            .unwrap();
    }

    pub async fn create_and_lock(
        &self,
        entries: Vec<BlobEntry>,
        node_id: String,
    ) -> Result<BlobOffset, BlobError> {
        let keys = entries.iter().map(|e| e.key.clone()).collect::<Vec<_>>();
        // check that keys does not exist already
        for key in keys.iter() {
            if self.map_lookup(key).await.is_ok() {
                return Err(BlobError::AlreadyExists);
            }
        }

        // check that keys are unique
        {
            let keys_set = keys.iter().collect::<HashSet<_>>();
            if keys_set.len() != keys.len() {
                return Err(BlobError::DuplicateKeys);
            }
        }

        let notify = Arc::new(Notify::new());

        // lock file picking
        let _guard = self.file_lock.lock().await;
        let (file_id, needs_creation) = {
            // possible cases:
            // - if file_pool is not at full capacity, we create a new file
            // - otherwise, if we are at full capacity we filter locked files from pool:
            //  - if filter is empty, we wait for a random file to be unlocked
            //  - if filter is not empty, pick a random one from intersection
            if self.file_pool.len() as u32 != self.max_files {
                // we create a new file
                let file_id = self.file_pool.len() as u32;
                let file_name = format!("blob_{}.bin", file_id);
                let file_info = FileInfo {
                    size: 0,
                    file_id,
                    file_name,
                    unlock_notify: notify.clone(),
                };
                self.file_pool.insert(file_id, file_info);
                (file_id, true)
            } else {
                let filter = self
                    .file_pool
                    .iter()
                    .filter(|a| !self.locked_files.contains_key(a.key()))
                    .map(|a| *a.key())
                    .collect::<Vec<_>>();
                if filter.is_empty() {
                    // wait for a random file to be unlocked
                    let file_id = {
                        let mut rng = rand::thread_rng();
                        rng.gen_range(0..self.max_files)
                    };
                    let notify = self
                        .file_pool
                        .get(&file_id)
                        .unwrap()
                        .value()
                        .unlock_notify
                        .clone();
                    notify.notified().await;
                    (file_id, false)
                } else {
                    // pick a random one from intersection
                    let file_id = {
                        let mut rng = rand::thread_rng();
                        *filter.choose(&mut rng).unwrap()
                    };
                    (file_id, false)
                }
            }
        };
        // lock file
        self.locked_files.insert(
            file_id,
            FileLock {
                node_id: node_id.clone(),
                keys: keys.clone(),
            },
        );

        // release file picking lock
        drop(_guard);

        if needs_creation {
            // add to redis file pool
            self.add_to_redis_filepool(self.file_pool.get(&file_id).unwrap().value())
                .await;
        }

        let file_name = self
            .file_pool
            .get(&file_id)
            .unwrap()
            .value()
            .file_name
            .clone();

        // get mut the file info
        let byte_offset = {
            let mut file_info = self.file_pool.get_mut(&file_id).unwrap();
            let prev_size = file_info.size;
            for entry in entries.iter() {
                file_info.size += entry.num_bytes;
            }
            // set new file into __file_pool__ at idx file_id
            self.add_to_redis_filepool(file_info.value()).await;

            prev_size
        };

        {
            let mut redis = self.redis.lock().await;
            let mut offset = byte_offset;
            for entry in entries {
                let slice = BlobStorageSlice {
                    file_id,
                    file_name: file_name.clone(),
                    byte_offset: offset,
                    num_bytes: entry.num_bytes,
                };
                let lock_wrapper = LockWrapper {
                    slice,
                    written: false,
                    lock: Some(node_id.clone()),
                };
                // insert into the map
                let _: () = redis
                    .set(&entry.key, serde_json::to_string(&lock_wrapper).unwrap())
                    .unwrap();
                self.map.insert(entry.key, lock_wrapper);
                offset += entry.num_bytes;
            }
        }

        let blob_offset = BlobOffset {
            file_name,
            file_id,
            needs_creation,
            byte_offset,
        };

        Ok(blob_offset)
    }

    pub async fn keep_alive_lock(&self, key: String, node_id: String) -> Result<(), BlobError> {
        todo!()
    }

    pub async fn create_unlock(&self, file_id: u32, node_id: String) -> Result<(), BlobError> {
        if !self.locked_files.contains_key(&file_id) {
            return Err(BlobError::CreateNotLocked);
        }
        {
            let lock = self.locked_files.get_mut(&file_id).unwrap();
            if lock.node_id != node_id {
                return Err(BlobError::WrongNode);
            }
            let mut redis = self.redis.lock().await;
            // unlock the keys and mark as written
            for key in lock.keys.iter() {
                let mut entry = self.map_lookup_mut(key).await.unwrap();
                let value = entry.value_mut();
                value.lock = None;
                value.written = true;
                // set into redis
                let _: () = redis
                    .set(key, serde_json::to_string(&value).unwrap())
                    .unwrap();
            }
        }

        // remove the file lock
        self.locked_files.remove(&file_id);

        // notify all the keys that are waiting for this file to be unlocked
        // that they
        let file_info = self.file_pool.get(&file_id).unwrap();
        file_info.value().unlock_notify.notify_waiters();

        Ok(())
    }

    pub async fn lookup(&self, key: String) -> Result<BlobStorageSlice, BlobError> {
        println!("lookup: {:?}", key);
        let v = self.map_lookup(&key).await?;
        if !v.written {
            return Err(BlobError::NotWritten);
        }
        Ok(v.slice)
    }
}

#[derive(Debug)]
pub enum BlobError {
    AlreadyExists,
    CreateNotLocked,
    DuplicateKeys,
    DoesNotExist,
    NotWritten,
    WrongNode,
    ProhibitedKey,
}

impl std::fmt::Display for BlobError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlobError::AlreadyExists => write!(f, "Blob already exists"),
            BlobError::CreateNotLocked => write!(f, "Blob create not locked"),
            BlobError::DuplicateKeys => write!(f, "Blob duplicate keys"),
            BlobError::DoesNotExist => write!(f, "Blob does not exist"),
            BlobError::ProhibitedKey => write!(f, "Blob key is prohibited"),
            BlobError::WrongNode => write!(f, "Blob is locked by another node"),
            BlobError::NotWritten => write!(f, "Blob is not written"),
        }
    }
}

impl std::error::Error for BlobError {}
