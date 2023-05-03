use std::{
    net::SocketAddr,
    pin::Pin,
    str::FromStr,
    sync::Arc,
    task::{Context, Poll},
};

use futures::Future;
use hyper::{service::Service, Body, Request, Response, Server};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    blob::{BlobStorage, BlobStorageConfig},
    errors::JobError,
    job::JobManagerConfig,
};
use crate::{errors::HTTPError, job::JobManager};

pub struct HTTP {
    // the host and port for a http server
    host: String,
    port: String,
    api_key: String,
}

impl HTTP {
    pub fn new(host: String, port: String, api_key: String) -> Self {
        HTTP {
            host,
            port,
            api_key,
        }
    }

    pub async fn start(
        self,
        blob_config: BlobStorageConfig,
        job_config: JobManagerConfig,
        shutdown_signal: impl Future<Output = ()>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let addr = SocketAddr::from_str(&format!("{}:{}", self.host, self.port))?;

        let max_workers = job_config.max_comp_worker_jobs + job_config.max_xfer_worker_jobs;
        let job_manager = if max_workers > 0 {
            Some(Arc::new(JobManager::init(job_config).await))
        } else {
            None
        };

        let blob = Arc::new(BlobStorage::init(blob_config).await);

        let server = Server::bind(&addr).serve(MakeSvc {
            blob,
            job_manager,
            api_key: self.api_key,
        });

        println!("Listening on http://{addr}");

        server.with_graceful_shutdown(shutdown_signal).await?;
        Ok(())
    }
}

/// Represents a service for the hyper http server
struct Svc {
    blob_store: Arc<BlobStorage>,
    job_manager: Option<Arc<JobManager>>,
    api_key: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CreateAndLockRequest {
    pub entries: Vec<BlobEntry>,
    pub node_id: String,
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

#[derive(Clone, Serialize, Deserialize)]
pub struct CreateUnlockRequest {
    pub file_id: u32,
    pub node_id: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct KeepAliveLockRequest {
    pub file_id: u32,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct LookupRequest {
    pub key: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SubmitJobRequest {
    pub job_type: JobType,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum JobType {
    DownloadURLs {
        urls: Vec<String>,
    },
    ReadKey {
        key: String,
    },
    Compute {
        binary: String,
        tarball_chunks: Vec<Vec<String>>,
        timeout: Option<u64>, // defaults to 10 minutes
    },
    ComputeMulti {
        binary: String,
        tarball_chunks: Vec<Vec<Vec<String>>>,
        timeout: Option<u64>, // defaults to 10 minutes
    },
    StoreTarballs {
        filepaths: Vec<String>,
    },
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SubmitJobReadKeyResponse {
    pub filepath: String,
}

fn try_from_str<'a, T>(s: &'a str) -> Result<T, HTTPError>
where
    T: Deserialize<'a>,
{
    serde_json::from_str(s).map_err(|e| HTTPError::InvalidBody(format!("Invalid json: {}", e)))
}

impl Service<Request<Body>> for Svc {
    type Response = Response<Body>;
    type Error = hyper::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, mut req: Request<Body>) -> Self::Future {
        fn mk_error(s: String, code: u16) -> Result<Response<Body>, hyper::Error> {
            Ok(Response::builder()
                .status(code)
                .body(Body::from(s))
                .unwrap())
        }
        fn mk_res(s: String) -> Result<Response<Body>, hyper::Error> {
            Ok(Response::builder().body(Body::from(s)).unwrap())
        }

        let blob_store = self.blob_store.clone();
        let job_manager = self.job_manager.clone();
        let api_key = self.api_key.clone();
        // routes:
        //  - POST:
        //     - /blob/create_and_lock
        //       - body: {"entries": [{"key": "some_key", "num_bytes": 100}, ...], "node_id": "some_node_id"}
        //       - returns: BlobOffset or error
        //     - /blob/create_unlock
        //       - body: {"key": "some_key", "node_id": "some_node_id"}
        //       - returns: empty or error
        //     - /blob/keep_alive_lock
        //       - body: {"file_id": 100}
        //       - returns: empty or error
        //     - /job/submit
        //       - body: { "job_type": { "type": "download_urls", "urls": ["url1", "url2"] } }
        //       - returns: depends on job type
        //  - GET:
        //     - /blob/lookup
        //       - body: { "key": "some_key" }
        //       - returns: BlobStorageSlice or error
        Box::pin(async move {
            let thunk = async move {
                // get the body
                let body = hyper::body::to_bytes(req.body_mut()).await?;
                let body = String::from_utf8(body.to_vec()).expect("invalid utf8");

                // get auth header
                let auth_header = req.headers().get("Authorization");

                // check api key equals to the one in the header
                if auth_header.is_none()
                    || auth_header.unwrap().to_str().is_err()
                    || auth_header.unwrap().to_str().unwrap() != api_key
                {
                    return Err(HTTPError::InvalidKey);
                }

                // get the method
                let method = req.method().to_string();
                // get the path
                let path = req.uri().path().to_string();
                let path = path.trim_start_matches('/').to_string();
                match method.as_str() {
                    "POST" => match path.as_str() {
                        "blob/create_and_lock" => {
                            routes::blob::create_and_lock(blob_store, try_from_str(&body)?).await
                        }
                        "blob/create_unlock" => {
                            routes::blob::create_unlock(blob_store, try_from_str(&body)?).await
                        }
                        "blob/keep_alive_lock" => {
                            routes::blob::keep_alive_lock(blob_store, try_from_str(&body)?).await
                        }
                        "job/submit" => match job_manager {
                            Some(man) => routes::job::submit_job(man, try_from_str(&body)?).await,
                            None => Err(HTTPError::Job(JobError::NoJobManager)),
                        },
                        p => Err(HTTPError::InvalidPath(p.to_string())),
                    },
                    "GET" => match path.as_str() {
                        "blob/lookup" => {
                            routes::blob::lookup(blob_store, try_from_str(&body)?).await
                        }
                        p => Err(HTTPError::InvalidPath(p.to_string())),
                    },
                    _ => Err(HTTPError::InvalidMethod(method)),
                }
            };
            match thunk.await {
                Ok(s) => mk_res(s),
                Err(HTTPError::Blob(e)) => {
                    let json_val = serde_json::to_value(e).unwrap();
                    mk_error(json!({ "error": json_val }).to_string(), 400)
                }
                Err(HTTPError::Job(JobError::ClientError(e))) => {
                    let json_val = serde_json::to_value(e).unwrap();
                    mk_error(json!({ "error": json_val }).to_string(), 400)
                }
                Err(HTTPError::Job(e)) => {
                    mk_error(json!({"error": e.to_string()}).to_string(), 400)
                }
                Err(e) => mk_error(json!({"error": e.to_string()}).to_string(), 500),
            }
        })
    }
}

mod routes {
    use std::sync::Arc;

    use crate::blob::BlobStorage;

    use super::*;

    pub(super) mod job {
        use super::*;
        pub(crate) async fn submit_job(
            job_manager: Arc<JobManager>,
            req: SubmitJobRequest,
        ) -> Result<String, HTTPError> {
            match req.job_type {
                JobType::DownloadURLs { urls } => {
                    job_manager.submit_download_job(urls).await?;
                    Ok("".to_string())
                }
                JobType::ReadKey { key } => {
                    let fp = job_manager.submit_read_job(key).await?;
                    Ok(serde_json::to_string(&SubmitJobReadKeyResponse {
                        filepath: fp,
                    })?)
                }
                JobType::Compute {
                    binary,
                    tarball_chunks,
                    timeout,
                } => {
                    let res = job_manager
                        .submit_compute(binary, tarball_chunks, timeout.unwrap_or(600))
                        .await?;
                    Ok(serde_json::to_string(&res)?)
                }
                JobType::ComputeMulti {
                    binary,
                    tarball_chunks,
                    timeout,
                } => {
                    let res = job_manager
                        .submit_compute_multi(binary, tarball_chunks, timeout.unwrap_or(600))
                        .await?;
                    Ok(serde_json::to_string(&res)?)
                }
                JobType::StoreTarballs { filepaths } => {
                    job_manager.submit_store_tarballs(filepaths).await?;
                    Ok("".to_string())
                }
            }
        }
    }

    pub(super) mod blob {

        use super::*;
        pub(crate) async fn lookup(
            blob: Arc<BlobStorage>,
            body: LookupRequest,
        ) -> Result<String, HTTPError> {
            let res = blob.lookup(body.key).await?;
            Ok(serde_json::to_string(&res)?)
        }

        pub(crate) async fn keep_alive_lock(
            blob: Arc<BlobStorage>,
            body: KeepAliveLockRequest,
        ) -> Result<String, HTTPError> {
            blob.keep_alive_lock(body.file_id).await?;
            Ok("".to_string())
        }

        pub(crate) async fn create_unlock(
            blob: Arc<BlobStorage>,
            body: CreateUnlockRequest,
        ) -> Result<String, HTTPError> {
            blob.create_unlock(body.file_id, body.node_id).await?;
            Ok("".to_string())
        }

        pub(crate) async fn create_and_lock(
            blob: Arc<BlobStorage>,
            body: CreateAndLockRequest,
        ) -> Result<String, HTTPError> {
            let res = blob.create_and_lock(body.entries, body.node_id).await?;
            Ok(serde_json::to_string(&res)?)
        }
    }
}

/// Represents a maker for a service for the hyper http server

#[derive(Clone)]
struct MakeSvc {
    blob: Arc<BlobStorage>,
    job_manager: Option<Arc<JobManager>>,
    api_key: String,
}

impl From<MakeSvc> for Svc {
    fn from(m: MakeSvc) -> Self {
        Svc {
            blob_store: m.blob,
            job_manager: m.job_manager,
            api_key: m.api_key,
        }
    }
}

impl<T> Service<T> for MakeSvc {
    type Response = Svc;
    type Error = hyper::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _: T) -> Self::Future {
        let svc = self.clone();
        let fut = async move { Ok(svc.into()) };
        Box::pin(fut)
    }
}
