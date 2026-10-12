//! Additional Axum HTTP boundary. Domain rules remain in the actual Cinema service.
use crate::{
    domain::Error,
    httpapi::{HttpService, response, route},
};
use axum::{
    Router,
    body::{Body, to_bytes},
    extract::{Request, State},
    http::HeaderValue,
    middleware::{self, Next},
    response::Response,
    routing::{get, patch, post, put},
};
use std::{
    future::IntoFuture,
    net::SocketAddr,
    sync::{Arc, Mutex},
    thread::JoinHandle,
    time::Duration,
};

type SharedService = Arc<Mutex<Box<dyn HttpService>>>;
fn reply(status: u16, value: serde_json::Value) -> Response {
    match serde_json::to_vec(&value) {
        Ok(bytes) => Response::builder()
            .status(status)
            .header("content-type", "application/json")
            .body(Body::from(bytes))
            .expect("valid static response"),
        Err(_) => Response::builder()
            .status(500)
            .body(Body::empty())
            .expect("valid static response"),
    }
}
async fn framework_header(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert("x-cinema-framework", HeaderValue::from_static("axum"));
    response
}
async fn invoke(State(service): State<SharedService>, request: Request) -> Response {
    let method = request.method().as_str().to_owned();
    let path = request.uri().to_string();
    let bytes = match to_bytes(request.into_body(), 1_048_576).await {
        Ok(bytes) => bytes,
        Err(_) => return reply(413, serde_json::json!({"error":"Body too large"})),
    };
    let (operation, args) = match route(&method, &path, &bytes) {
        Ok(invocation) => invocation,
        Err(error) => {
            let (status, body) = response(Err(error));
            return reply(status, body);
        }
    };
    let result = tokio::task::spawn_blocking(move || {
        service
            .lock()
            .map_err(|_| Error::Contract("Service unavailable".into()))?
            .invoke(operation, &args)
    })
    .await
    .unwrap_or_else(|_| Err(Error::Contract("Service task failed".into())));
    let (status, body) = response(result);
    reply(status, body)
}
/// Compose production routes once; tests exercise this exact router and middleware.
pub fn router<S: HttpService + 'static>(service: S) -> Router {
    Router::new()
        .route("/films", get(invoke).post(invoke))
        .route("/films/{id}", get(invoke))
        .route("/films/{id}/availability", patch(invoke))
        .route("/films/{id}/comments", get(invoke).post(invoke))
        .route("/films/{id}/rating", get(invoke).put(invoke))
        .route("/voting", post(invoke))
        .route("/voting/current", get(invoke))
        .route("/voting/{id}/votes", put(invoke))
        .route("/voting/{id}/close", post(invoke))
        .fallback(|| async { reply(404, serde_json::json!({"error":"Unknown HTTP route"})) })
        .method_not_allowed_fallback(|| async {
            reply(405, serde_json::json!({"error":"Unsupported HTTP method"}))
        })
        .with_state(Arc::new(Mutex::new(
            Box::new(service) as Box<dyn HttpService>
        )))
        .layer(middleware::from_fn(framework_header))
}
/// One bounded, owned runtime per HTTP session, with an explicit shutdown signal.
pub struct Server {
    address: SocketAddr,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    worker: Option<JoinHandle<std::io::Result<()>>>,
}
impl Server {
    pub fn start<S: HttpService + 'static>(address: &str, service: S) -> std::io::Result<Self> {
        let listener = std::net::TcpListener::bind(address)?;
        let address = listener.local_addr()?;
        listener.set_nonblocking(true)?;
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let (ready, started) = std::sync::mpsc::sync_channel(1);
        let worker = std::thread::Builder::new().name("cinema-axum".into()).spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().max_blocking_threads(2).build() {
                Ok(runtime) => runtime,
                Err(error) => { let message = error.to_string(); let _ = ready.send(Err(error)); return Err(std::io::Error::other(message)); }
            };
            let result = runtime.block_on(async move {
                let listener = match tokio::net::TcpListener::from_std(listener) {
                    Ok(listener) => listener,
                    Err(error) => { let message = error.to_string(); let _ = ready.send(Err(error)); return Err(std::io::Error::other(message)); }
                };
                let (shutdown, shut_down) = tokio::sync::oneshot::channel();
                let server = axum::serve(listener, router(service)).with_graceful_shutdown(async { let _ = shut_down.await; }).into_future();
                tokio::pin!(server);
                // Construct before announcing readiness. Deadline caps slow connections.
                let _ = ready.send(Ok(()));
                tokio::select! {
                    result = &mut server => result,
                    _ = stopped => {
                        let _ = shutdown.send(());
                        tokio::time::timeout(Duration::from_secs(5), &mut server).await
                            .map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, "Axum shutdown timed out"))?
                    }
                }
            });
            runtime.shutdown_timeout(Duration::from_secs(1));
            result
        })?;
        match started.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(())) => Ok(Self {
                address,
                stop: Some(stop),
                worker: Some(worker),
            }),
            error => {
                let _ = stop.send(());
                // Failed initialization has no active server or requests.
                let _ = worker.join();
                Err(match error {
                    Ok(Err(error)) => error,
                    _ => std::io::Error::other("Axum initialization failed"),
                })
            }
        }
    }
    pub fn address(&self) -> SocketAddr {
        self.address
    }
    pub fn shutdown(&mut self) -> std::io::Result<()> {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(worker) = self.worker.take() {
            worker
                .join()
                .map_err(|_| std::io::Error::other("Axum worker panicked"))??;
        }
        Ok(())
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}
