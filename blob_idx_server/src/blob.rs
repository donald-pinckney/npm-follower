use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use bb8_redis::redis::AsyncCommands;
use dashmap::DashMap;
use rand::{seq::SliceRandom, Rng};
use serde::{ser::SerializeStruct, Deserialize, Serialize};
use tokio::sync::{mpsc::Sender, Mutex, Notify};

use crate::{errors::BlobError, http::BlobEntry};

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
    notify_unlock: Arc<Notify>,
}

#[derive(Debug)]
struct CleanerInstance {
    keep_alive: Sender<()>,
    task: tokio::task::JoinHandle<()>,
}

/// Configuration to initialize a blob storage.
#[derive(Debug, Clone)]
pub struct BlobStorageConfig {
    /// The url to the redis server.
    pub redis_url: String,
    /// The maximum number of chunk files to use.
    pub max_files: u32,
    /// How much time to wait before cleaning up a lock in seconds.
    pub lock_timeout: u64,
}

impl Default for BlobStorageConfig {
    fn default() -> Self {
        dotenvy::dotenv().ok();
        // max files is 10 on debug, and 1000 on release
        let max_files = if cfg!(debug_assertions) { 10 } else { 1000 };
        let redis_url = std::env::var("BLOB_REDIS_URL").expect("BLOB_REDIS_URL");
        Self {
            redis_url,
            max_files,
            lock_timeout: 30,
        }
    }
}

/// A thread-safe blob storage API.
pub struct BlobStorage {
    /// The configuration of the blob storage.
    config: BlobStorageConfig,
    redis: bb8_redis::bb8::Pool<bb8_redis::RedisConnectionManager>,
    map: DashMap<String, LockWrapper>, // map [key] -> [slice + lock]
    /// pool of all the files (locked or not)
    file_pool: DashMap<u32, FileInfo>, // TODO: could do an array but i'm lazy now
    /// pool of all the files that are currently being written to.
    /// maps to a notifier that is notified when the file is write-unlocked.
    locked_files: Arc<DashMap<u32, FileLock>>,
    /// lock for picking a new file id
    file_lock: Mutex<()>,
    /// The map of lock cleanup tasks.
    cleanup_tasks: DashMap<u32, CleanerInstance>,
}

/// INFO: https://github.com/donald-pinckney/npm-follower/wiki/Design-of-the-Blob-Storage-Index-Server
impl BlobStorage {
    /// NOTE: with redis, on new we fully load the file pools.
    /// meanwhile for the k/v map, we lazily load it on first access.
    pub async fn init(mut config: BlobStorageConfig) -> BlobStorage {
        let redis_bb8_manager =
            bb8_redis::RedisConnectionManager::new(config.redis_url.as_str()).unwrap();
        let pool = bb8_redis::bb8::Pool::builder()
            .build(redis_bb8_manager)
            .await
            .expect("Failed to create pool.");
        let mut con = pool.get().await.expect("Failed to get redis connection");
        // load file pool from redis
        let file_pool = {
            if con.hlen("__file_pool__").await != Ok(0) {
                let set = con
                    .hgetall::<String, Vec<String>>("__file_pool__".to_string())
                    .await
                    .unwrap();
                let file_pool = DashMap::new();
                for (i, v) in set.iter().enumerate() {
                    // skip evens because of how redis hashes work lol
                    if i % 2 == 1 {
                        let file_info: FileInfo = serde_json::from_str(v).unwrap();
                        file_pool.insert(file_info.file_id, file_info);
                    }
                }

                // adjust max_files if the file pool is bigger than max_files
                if file_pool.len() > config.max_files as usize {
                    config.max_files = file_pool.len() as u32;
                }

                file_pool
            } else {
                DashMap::new()
            }
        };
        drop(con); // need to drop for ownership reasons

        BlobStorage {
            config,
            redis: pool,
            map: DashMap::new(),
            file_pool,
            locked_files: Arc::new(DashMap::new()),
            file_lock: Mutex::new(()),
            cleanup_tasks: DashMap::new(),
        }
    }

    /// spawns a lock cleaner, that cleans the lock of the file after 10 minutes.
    /// If a file is sent over the keep_alive_ch channel, the timer is reset.
    fn spawn_lock_cleaner(&self, file_id: u32) {
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        let locked_files = self.locked_files.clone();
        let lock_timeout = self.config.lock_timeout;
        let task = tokio::spawn(async move {
            let mut keep_alive_ch = rx;
            let duration = std::time::Duration::from_secs(lock_timeout);
            let mut timer = tokio::time::interval(duration);
            timer.tick().await; // start ticking
            loop {
                tokio::select! {
                    _ = timer.tick() => { // 10 min timer expired
                        if let Some((_, file_lock)) = locked_files.remove(&file_id) {
                            println!("cleaning up lock for file {}", file_id);
                            file_lock.notify_unlock.notify_waiters();
                            keep_alive_ch.close();
                            return;
                        } else {
                            // means the file was manually unlocked before the timer finished
                            keep_alive_ch.close();
                            return;
                        }
                    }
                    Some(()) = keep_alive_ch.recv() => {
                        // reset the timer
                        println!("resetting timer for file {}", file_id);
                        timer.reset();
                    }
                }
            }
        });

        self.cleanup_tasks.insert(
            file_id,
            CleanerInstance {
                keep_alive: tx,
                task,
            },
        );
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

        println!("\tlocked_files:\n{}", locked_files_str);

        let mut cleaners_str = String::new();
        for a in self.cleanup_tasks.iter() {
            cleaners_str.push_str(&format!("{} ", a.key()));
        }

        println!("\tcleaners: {}\n", cleaners_str);
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
        let mut redis = self.redis.get().await.unwrap();
        let v: Option<String> = redis.get(key).await.unwrap();
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
            let mut redis = self.redis.get().await.unwrap();
            redis.get(key).await.unwrap()
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
            .get()
            .await
            .unwrap()
            .hset(
                "__file_pool__",
                file_info.file_id,
                serde_json::to_string(file_info).unwrap(),
            )
            .await
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
            if let Ok(l) = self.map_lookup(key).await {
                // if the key is written, we can't write to it
                // but if the key is not written, we can overwrite it if the file is unlocked
                if l.written {
                    return Err(BlobError::AlreadyExists);
                }

                // check that the file is not locked (may be a cleaned key)
                if self.locked_files.contains_key(&l.slice.file_id) {
                    return Err(BlobError::AlreadyExists);
                }
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
            if self.file_pool.len() as u32 != self.config.max_files {
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
                        rng.gen_range(0..self.config.max_files)
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
                notify_unlock: self
                    .file_pool
                    .get(&file_id)
                    .unwrap()
                    .value()
                    .unlock_notify
                    .clone(),
            },
        );

        self.spawn_lock_cleaner(file_id);

        // release file picking lock
        drop(_guard);

        if needs_creation {
            // add to redis file pool
            let val = self.file_pool.get(&file_id).unwrap().value().clone();
            self.add_to_redis_filepool(&val).await;
        }

        // get mut the file info
        let (byte_offset, f_info) = {
            let mut file_info = self.file_pool.get_mut(&file_id).unwrap();
            let prev_size = file_info.size;
            for entry in entries.iter() {
                file_info.size += entry.num_bytes;
            }

            (prev_size, file_info.value().clone())
        };
        // set new file into __file_pool__ at idx file_id
        self.add_to_redis_filepool(&f_info).await;

        {
            let mut offset = byte_offset;
            let mut to_set_in_redis = vec![];
            for entry in entries {
                let slice = BlobStorageSlice {
                    file_id,
                    file_name: f_info.file_name.clone(),
                    byte_offset: offset,
                    num_bytes: entry.num_bytes,
                };
                let lock_wrapper = LockWrapper {
                    slice,
                    written: false,
                    lock: Some(node_id.clone()),
                };
                // insert into the map
                to_set_in_redis.push((
                    entry.key.clone(),
                    serde_json::to_string(&lock_wrapper).unwrap(),
                ));
                self.map.insert(entry.key, lock_wrapper);
                offset += entry.num_bytes;
            }
            let mut redis = self.redis.get().await.unwrap();
            let _: () = redis.set_multiple(&to_set_in_redis).await.unwrap();
        }

        let blob_offset = BlobOffset {
            file_name: f_info.file_name,
            file_id,
            needs_creation,
            byte_offset,
        };

        Ok(blob_offset)
    }

    pub async fn keep_alive_lock(&self, file_id: u32) -> Result<(), BlobError> {
        if self.locked_files.contains_key(&file_id) {
            let cleaner = self.cleanup_tasks.get(&file_id).unwrap();
            cleaner
                .keep_alive
                .send(())
                .await
                .map_err(|_| BlobError::LockExpired)?;
            Ok(())
        } else {
            Err(BlobError::CreateNotLocked)
        }
    }

    pub async fn create_unlock(&self, file_id: u32, node_id: String) -> Result<(), BlobError> {
        if !self.locked_files.contains_key(&file_id) {
            return Err(BlobError::CreateNotLocked);
        }
        {
            let lock = self.locked_files.remove(&file_id).unwrap().1;
            if lock.node_id != node_id {
                return Err(BlobError::WrongNode);
            }
            // unlock the keys and mark as written
            let mut to_set_in_redis = vec![];
            for key in lock.keys.iter() {
                let mut entry = self.map_lookup_mut(key).await.unwrap();
                let value = entry.value_mut();
                value.lock = None;
                value.written = true;
                to_set_in_redis.push((key.clone(), serde_json::to_string(value).unwrap()));
            }
            // set into redis
            let mut redis = self.redis.get().await.unwrap();
            let _: () = redis.set_multiple(&to_set_in_redis).await.unwrap();
        }

        // remove the cleanup task
        {
            if let Some((_, i)) = self.cleanup_tasks.remove(&file_id) {
                i.task.abort();
            }
        }

        // notify all the keys that are waiting for this file to be unlocked
        // that they
        let file_info = self.file_pool.get(&file_id).unwrap();
        file_info.value().unlock_notify.notify_waiters();

        Ok(())
    }

    pub async fn lookup(&self, key: String) -> Result<BlobStorageSlice, BlobError> {
        let v = self.map_lookup(&key).await?;
        if !v.written {
            return Err(BlobError::NotWritten);
        }
        Ok(v.slice)
    }

    /// Waits for all locks to be released
    pub async fn shutdown(&self) {
        let _guard = self.file_lock.lock().await;
        for lock in self.locked_files.iter() {
            lock.value().notify_unlock.notified().await;
        }
    }
}
