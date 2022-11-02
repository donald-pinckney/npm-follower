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

use crate::blob::{BlobError, BlobStorage, BlobStorageConfig};

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
        shutdown_signal: impl Future<Output = ()>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let addr = SocketAddr::from_str(&format!("{}:{}", self.host, self.port))?;

        let server = Server::bind(&addr).serve(MakeSvc {
            session: Arc::new(BlobStorage::init(blob_config).await),
            api_key: self.api_key,
        });

        println!("Listening on http://{}", addr);

        server.with_graceful_shutdown(shutdown_signal).await?;
        Ok(())
    }
}

/// Represents a service for the hyper http server
struct Svc {
    session: Arc<BlobStorage>,
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

        let cloned_session = self.session.clone();
        let api_key = self.api_key.clone();
        // routes:
        //  - POST:
        //     - /create_and_lock
        //       - body: {"entries": [{"key": "some_key", "num_bytes": 100}, ...], "node_id": "some_node_id"}
        //       - returns: BlobOffset or error
        //     - /create_unlock
        //       - body: {"key": "some_key", "node_id": "some_node_id"}
        //       - returns: empty or error
        //     - /keep_alive_lock
        //       - body: {"file_id": 100}
        //       - returns: empty or error
        //  - GET:
        //     - /lookup
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
                match method.as_str() {
                    "POST" => {
                        // get the path
                        let path = req.uri().path().to_string();
                        match path.as_str() {
                            "/create_and_lock" => {
                                let body: CreateAndLockRequest = try_from_str(&body)?;
                                let res = cloned_session
                                    .create_and_lock(body.entries, body.node_id)
                                    .await?;
                                #[cfg(debug_assertions)]
                                cloned_session.debug_print("ran create_and_lock").await;
                                Ok(serde_json::to_string(&res)?)
                            }
                            "/create_unlock" => {
                                let body: CreateUnlockRequest = try_from_str(&body)?;
                                cloned_session
                                    .create_unlock(body.file_id, body.node_id)
                                    .await?;
                                #[cfg(debug_assertions)]
                                cloned_session.debug_print("ran create_unlock").await;
                                Ok("".to_string())
                            }
                            "/keep_alive_lock" => {
                                let body: KeepAliveLockRequest = try_from_str(&body)?;
                                cloned_session.keep_alive_lock(body.file_id).await?;
                                #[cfg(debug_assertions)]
                                cloned_session.debug_print("ran keep_alive_lock").await;
                                Ok("".to_string())
                            }
                            p => Err(HTTPError::InvalidPath(p.to_string())),
                        }
                    }
                    "GET" => match req.uri().path().to_string().as_str() {
                        "/lookup" => {
                            let body: LookupRequest = try_from_str(&body)?;
                            let res = cloned_session.lookup(body.key).await?;
                            #[cfg(debug_assertions)]
                            cloned_session.debug_print("ran lookup").await;
                            Ok(serde_json::to_string(&res)?)
                        }
                        p => Err(HTTPError::InvalidPath(p.to_string())),
                    },
                    _ => Err(HTTPError::InvalidMethod(method)),
                }
            };
            match thunk.await {
                Ok(s) => mk_res(s),
                Err(HTTPError::Blob(e)) => {
                    mk_error(json!({"error": e.to_string()}).to_string(), 400)
                }
                Err(e) => mk_error(json!({"error": e.to_string()}).to_string(), 500),
            }
        })
    }
}

/// Represents a maker for a service for the hyper http server

struct MakeSvc {
    session: Arc<BlobStorage>,
    api_key: String,
}

impl<T> Service<T> for MakeSvc {
    type Response = Svc;
    type Error = hyper::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _: T) -> Self::Future {
        let session = self.session.clone();
        let api_key = self.api_key.clone();
        let fut = async move { Ok(Svc { session, api_key }) };
        Box::pin(fut)
    }
}

#[derive(Debug)]
pub enum HTTPError {
    Hyper(hyper::Error),
    Io(std::io::Error),
    Blob(BlobError),
    Serde(serde_json::Error),
    InvalidBody(String), // missing a field in the body
    InvalidMethod(String),
    InvalidKey,
    InvalidPath(String),
}

impl From<hyper::Error> for HTTPError {
    fn from(e: hyper::Error) -> Self {
        HTTPError::Hyper(e)
    }
}

impl From<std::io::Error> for HTTPError {
    fn from(e: std::io::Error) -> Self {
        HTTPError::Io(e)
    }
}

impl From<BlobError> for HTTPError {
    fn from(e: BlobError) -> Self {
        HTTPError::Blob(e)
    }
}

impl From<serde_json::Error> for HTTPError {
    fn from(e: serde_json::Error) -> Self {
        HTTPError::Serde(e)
    }
}

impl std::fmt::Display for HTTPError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            HTTPError::Hyper(e) => write!(f, "Hyper error: {}", e),
            HTTPError::Io(e) => write!(f, "IO error: {}", e),
            HTTPError::Blob(e) => write!(f, "Blob error: {}", e),
            HTTPError::InvalidBody(e) => write!(f, "Invalid body: {}", e),
            HTTPError::InvalidMethod(e) => write!(f, "Invalid method: {}", e),
            HTTPError::InvalidPath(e) => write!(f, "Invalid path: {}", e),
            HTTPError::Serde(e) => write!(f, "Serde error: {}", e),
            HTTPError::InvalidKey => write!(f, "Invalid api key"),
        }
    }
}
