use std::{
    net::SocketAddr,
    pin::Pin,
    str::FromStr,
    task::{Context, Poll},
};

use futures::Future;
use hyper::{service::Service, Body, Request, Response, Server};
use serde_json::json;

use crate::blob::{BlobError, BlobStorage};

pub struct HTTP {
    // the host and port for a http server
    host: String,
    port: String,
}

impl HTTP {
    pub fn new(host: String, port: String) -> Self {
        HTTP { host, port }
    }

    pub async fn start(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let addr = SocketAddr::from_str(&format!("{}:{}", self.host, self.port))?;

        let server = Server::bind(&addr).serve(MakeSvc {
            session: BlobStorage::init().await,
        });

        println!("Listening on http://{}", addr);

        server.await?;
        Ok(())
    }
}

/// Represents a service for the hyper http server
struct Svc {
    session: BlobStorage,
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
        // routes:
        //  - POST:
        //     - /create_and_lock
        //       - body: { "key": "some_key", "num_bytes": 100, "node_id": "some_node_id" }
        //       - returns: BlobStorageSlice or error
        //     - /create_unlock
        //       - body: { "key": "some_key", "node_id": "some_node_id" }
        //       - returns: empty or error
        //     - /read_lock
        //       - body: { "key": "some_key", "node_id": "some_node_id" }
        //       - returns: empty or error
        //     - /read_unlock
        //       - body: { "key": "some_key", "node_id": "some_node_id" }
        //       - returns: empty or error
        //  - GET:
        //     - /lookup
        //       - body: { "key": "some_key" }
        //       - returns: BlobStorageSlice or error
        Box::pin(async move {
            fn get_key<'a>(
                body: &'a serde_json::Value,
                key: &str,
            ) -> Result<&'a serde_json::Value, HTTPError> {
                body.get(key)
                    .ok_or_else(|| HTTPError::InvalidBody(key.to_string()))
            }

            let thunk = async move {
                // get the body
                let body = hyper::body::to_bytes(req.body_mut()).await?;
                let body = String::from_utf8(body.to_vec()).expect("invalid utf8");
                let body: serde_json::Value = serde_json::from_str(&body)?;

                // get the method
                let method = req.method().to_string();
                match method.as_str() {
                    "POST" => {
                        // get the path
                        let path = req.uri().path().to_string();
                        match path.as_str() {
                            "/create_and_lock" => {
                                let key = get_key(&body, "key")?.to_string();
                                let num_bytes =
                                    get_key(&body, "num_bytes")?.as_u64().ok_or_else(|| {
                                        HTTPError::InvalidBody("num_bytes".to_string())
                                    })?;
                                let node_id = get_key(&body, "node_id")?.to_string();
                                let res = cloned_session
                                    .create_and_lock(key, num_bytes, node_id)
                                    .await?;
                                #[cfg(debug_assertions)]
                                cloned_session.debug_print("ran create_and_lock").await;
                                Ok(serde_json::to_string(&res)?)
                            }
                            "/create_unlock" => {
                                let key = get_key(&body, "key")?.to_string();
                                let node_id = get_key(&body, "node_id")?.to_string();
                                cloned_session.create_unlock(key, node_id).await?;
                                #[cfg(debug_assertions)]
                                cloned_session.debug_print("ran create_unlock").await;
                                Ok("".to_string())
                            }
                            "/read_lock" => {
                                let key = get_key(&body, "key")?.to_string();
                                let node_id = get_key(&body, "node_id")?.to_string();
                                cloned_session.read_lock(key, node_id).await?;
                                #[cfg(debug_assertions)]
                                cloned_session.debug_print("ran read_lock").await;
                                Ok("".to_string())
                            }
                            "/read_unlock" => {
                                let key = get_key(&body, "key")?.to_string();
                                let node_id = get_key(&body, "node_id")?.to_string();
                                cloned_session.read_unlock(key, node_id).await?;
                                #[cfg(debug_assertions)]
                                cloned_session.debug_print("ran read_unlock").await;
                                Ok("".to_string())
                            }
                            p => Err(HTTPError::InvalidPath(p.to_string())),
                        }
                    }
                    "GET" => match req.uri().path().to_string().as_str() {
                        "/lookup" => {
                            let key = get_key(&body, "key")?.to_string();
                            let res = cloned_session.lookup(key).await?;
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
    session: BlobStorage,
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
        let fut = async move { Ok(Svc { session }) };
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
        }
    }
}
