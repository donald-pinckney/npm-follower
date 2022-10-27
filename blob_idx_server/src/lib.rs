use std::sync::Arc;

use dashmap::DashMap;
use rand::Rng;
use redis::Commands;
use serde::{ser::SerializeStruct, Deserialize, Serialize};
use tokio::sync::{Mutex, Notify};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobStorageSlice {
    pub file_id: u32,
    pub byte_offset: u64,
    pub num_bytes: u64,
    pub needs_creation: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LockType {
    Read,
    Write,
}

#[derive(Debug, Clone)]
struct BlobStorageLock {
    node_id: String,
    lock_type: LockType,
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
    unlock_read_notify: Arc<Notify>,
    unlock_write_notify: Arc<Notify>,
    lock: Option<BlobStorageLock>,
}

impl Serialize for LockWrapper {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("LockWrapper", 2)?;
        state.serialize_field("slice", &self.slice)?;
        state.serialize_field("written", &self.written)?;
        // don't serialize the lock and unlock_notify
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
            unlock_read_notify: Arc::new(Notify::new()),
            unlock_write_notify: Arc::new(Notify::new()),
            lock: None,
        })
    }
}

#[derive(Debug, Clone)]
struct FileInfo {
    size: u64,
    file_id: u32,
    file_name: String,
    unlock_read_notify: Arc<Notify>,
    unlock_write_notify: Arc<Notify>,
}

impl Serialize for FileInfo {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("LockWrapper", 2)?;
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
            unlock_read_notify: Arc::new(Notify::new()),
            unlock_write_notify: Arc::new(Notify::new()),
        })
    }
}

#[derive(Debug, Clone)]
struct FileLock {
    lock_type: LockType,
    node_id: String,
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
            locked_files: Arc::new(DashMap::new()),
            file_lock: Arc::new(Mutex::new(())),
        }
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
        let mut redis = self.redis.lock().await;
        let v: Option<String> = redis.get(key).unwrap();
        if let Some(v) = v {
            // serialize the string into a LockWrapper
            let v: LockWrapper = serde_json::from_str(&v).unwrap();
            self.map.insert(key.to_string(), v);
            Ok(self.map.get_mut(key).unwrap())
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

        let read_notify = Arc::new(Notify::new());
        let write_notify = Arc::new(Notify::new());

        // picks random file id from 0 to 999, and checks if it is locked.
        // if it is locked, picks another one, then locks the file.
        // returns the id of the file and a boolean that represents if the file needs to be created.
        let (file_id, needs_creation) = {
            // lock file picking
            let _guard = self.file_lock.lock().await;
            let mut rng = rand::thread_rng();
            // TODO: this is dumb, make this better:
            // - do intersection of pools, finding ones that are not locked
            // - if intersection is empty, we wait for a random file to be unlocked
            // - if intersection is not empty, pick a random one from intersection
            loop {
                let file_id = rng.gen_range(0..1000);
                if !self.locked_files.contains_key(&file_id) {
                    // we lock the file
                    self.locked_files.insert(
                        file_id,
                        FileLock {
                            lock_type: LockType::Write,
                            node_id: node_id.clone(),
                        },
                    );
                    // check if file exists already
                    if self.file_pool.contains_key(&file_id) {
                        break (file_id, false);
                    } else {
                        // if not, we create a new file
                        // TODO: actually create the file on disk
                        let file_name = format!("blob_{}.bin", file_id);
                        let file_info = FileInfo {
                            size: 0,
                            file_id,
                            file_name,
                            unlock_read_notify: read_notify.clone(),
                            unlock_write_notify: write_notify.clone(),
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
        let byte_offset = {
            let mut file_info = self.file_pool.get_mut(&file_id).unwrap();
            file_info.size += num_bytes;
            file_info.value().size
        };
        let slice = BlobStorageSlice {
            file_id,
            byte_offset,
            num_bytes,
            needs_creation,
        };
        let lock = BlobStorageLock {
            node_id,
            lock_type: LockType::Write,
        };
        let lock_wrapper = LockWrapper {
            slice: slice.clone(),
            written: false,
            unlock_read_notify: read_notify,
            unlock_write_notify: write_notify,
            lock: Some(lock),
        };

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
        let (lock, notify, file_id) = {
            let chunk = self.map_lookup(&key).await?;
            let lock = chunk
                .lock
                .as_ref()
                .ok_or(BlobError::CreateNotLocked)?
                .clone();
            let file_id = chunk.slice.file_id;
            (lock, chunk.unlock_write_notify, file_id)
        };
        if lock.lock_type != LockType::Write {
            return Err(BlobError::CreateNotLocked);
        }
        if lock.node_id != node_id {
            return Err(BlobError::WrongNode);
        }
        {
            let file_lock = self
                .locked_files
                .get(&file_id)
                .ok_or(BlobError::CreateNotLocked)?;

            if file_lock.node_id != node_id {
                return Err(BlobError::WrongNode);
            }
        }

        // notify that write lock is released
        notify.notify_one();
        {
            let mut entry = self.map_lookup_mut(&key).await?;
            let chunk_wrap = entry.value_mut();
            chunk_wrap.lock = None;
            chunk_wrap.written = true;
            // set written to true in redis
            let _: () = self
                .redis
                .lock()
                .await
                .set(key, serde_json::to_string(&chunk_wrap).unwrap())
                .unwrap();
        }
        self.locked_files.remove(&file_id);

        Ok(())
    }

    pub async fn lookup(&self, key: String) -> Result<BlobStorageSlice, BlobError> {
        let v = self.map_lookup(&key).await?;
        if !v.written {
            return Err(BlobError::NotWritten);
        }
        Ok(v.slice)
    }

    pub async fn read_lock(&self, key: String, node_id: String) -> Result<(), BlobError> {
        let chunk_wrap = {
            let val = self.map_lookup(&key).await?;
            val.clone()
        };

        if chunk_wrap.lock.is_none() {
            // check if file is locked
            let maybe_file_lock = self.locked_files.get(&chunk_wrap.slice.file_id);
            if maybe_file_lock.is_some() {
                let (lock_node_id, lock_type) = {
                    let file_lock = maybe_file_lock.unwrap();
                    let node_id = file_lock.node_id.clone();
                    let lock_type = file_lock.lock_type.clone();
                    drop(file_lock);
                    (node_id, lock_type)
                };
                // check if it is write locked, if not we can read lock
                if lock_type == LockType::Write {
                    // if it's write locked, we need to check if it's locked by us
                    if lock_node_id != node_id {
                        // if not, we wait for the lock to be released
                        // (chunk_wrap.unlock_notify is the same lock as the lock on the file)
                        chunk_wrap.unlock_write_notify.notified().await;

                        // then we lock it as ours
                        self.locked_files.insert(
                            chunk_wrap.slice.file_id,
                            FileLock {
                                lock_type: LockType::Read,
                                node_id: node_id.clone(),
                            },
                        );
                    }
                }
            }
        } else if let Some(lock) = chunk_wrap.lock.as_ref() {
            // if it's already locked, we check if it's write locked. if so we wait for it to be
            // unlocked
            if lock.lock_type == LockType::Write {
                chunk_wrap.unlock_write_notify.notified().await;
            }
        }

        let mut entry = self.map_lookup_mut(&key).await?;
        let mut chunk_wrap = entry.value_mut();
        // read lock the chunk
        chunk_wrap.lock = Some(BlobStorageLock {
            node_id,
            lock_type: LockType::Read,
        });

        Ok(())
    }

    pub async fn keep_alive_read_lock(
        &self,
        key: String,
        node_id: String,
    ) -> Result<(), BlobError> {
        todo!()
    }

    pub async fn read_unlock(&self, key: String, node_id: String) -> Result<(), BlobError> {
        let (lock, notify, file_id) = {
            let chunk = self.map_lookup(&key).await?;
            let lock = chunk
                .lock
                .as_ref()
                .ok_or(BlobError::CreateNotLocked)?
                .clone();
            let file_id = chunk.slice.file_id;
            (lock, chunk.unlock_read_notify, file_id)
        };
        if lock.lock_type != LockType::Write {
            return Err(BlobError::CreateNotLocked);
        }
        if lock.node_id != node_id {
            return Err(BlobError::WrongNode);
        }
        {
            let file_lock = self
                .locked_files
                .get(&file_id)
                .ok_or(BlobError::ReadNotLocked)?;

            if file_lock.node_id != node_id {
                return Err(BlobError::WrongNode);
            }
        }

        // notify that read lock is released
        notify.notify_one();
        {
            let mut entry = self.map_lookup_mut(&key).await?;
            let chunk_wrap = entry.value_mut();
            chunk_wrap.lock = None;
            chunk_wrap.written = true;
            // set written to true in redis
            let _: () = self
                .redis
                .lock()
                .await
                .set(key, serde_json::to_string(&chunk_wrap).unwrap())
                .unwrap();
        }
        self.locked_files.remove(&file_id);

        Ok(())
    }
}

#[derive(Debug)]
pub enum BlobError {
    AlreadyExists,
    CreateNotLocked,
    DoesNotExist,
    NotWritten,
    WrongNode,
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
            BlobError::WrongNode => write!(f, "Blob is locked by another node"),
            BlobError::NotWritten => write!(f, "Blob is not written"),
        }
    }
}

impl std::error::Error for BlobError {}
