//! Explicit HTTP boundary; production routes are independent of specification tooling.
use crate::{
    application::{Ports, Publisher, Service},
    domain::Error,
};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

pub trait HttpService: Send {
    fn invoke(&mut self, operation: &str, args: &[Value]) -> Result<Value, Error>;
}
impl<P: Ports + Send, E: Publisher + Send> HttpService for Service<P, E> {
    fn invoke(&mut self, operation: &str, args: &[Value]) -> Result<Value, Error> {
        Service::invoke(self, operation, args)
    }
}
pub struct Server {
    address: SocketAddr,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}
impl Server {
    pub fn start<S: HttpService + 'static>(address: &str, service: S) -> std::io::Result<Self> {
        let listener = TcpListener::bind(address)?;
        let address = listener.local_addr()?;
        listener.set_nonblocking(true)?;
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let service = Arc::new(Mutex::new(service));
        let worker = thread::spawn(move || {
            while !flag.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                        let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
                        let result = read_request(&mut stream).and_then(|(method, path, body)| {
                            let (operation, args) = route(&method, &path, &body)?;
                            service
                                .lock()
                                .map_err(|_| Error::Contract("Service unavailable".into()))?
                                .invoke(operation, &args)
                        });
                        let (status, value) = response(result);
                        if let Ok(body) = serde_json::to_vec(&value) {
                            let _ = write!(
                                stream,
                                "HTTP/1.1 {status} Response\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                body.len()
                            );
                            let _ = stream.write_all(&body);
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2))
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            address,
            stop,
            worker: Some(worker),
        })
    }
    pub fn address(&self) -> SocketAddr {
        self.address
    }
    pub fn shutdown(&mut self) -> std::io::Result<()> {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            worker
                .join()
                .map_err(|_| std::io::Error::other("HTTP worker panicked"))?;
        }
        Ok(())
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}
fn invalid(reason: &str) -> Error {
    Error::Contract(reason.into())
}
fn read_request(stream: &mut TcpStream) -> Result<(String, String, Vec<u8>), Error> {
    let mut data = Vec::new();
    let mut buffer = [0; 4096];
    let end = loop {
        if let Some(index) = data.windows(4).position(|w| w == b"\r\n\r\n") {
            break index + 4;
        }
        if data.len() > 65536 {
            return Err(invalid("HTTP headers too large"));
        }
        let count = stream
            .read(&mut buffer)
            .map_err(|_| invalid("Cannot read request"))?;
        if count == 0 {
            return Err(invalid("Incomplete HTTP request"));
        }
        data.extend_from_slice(&buffer[..count]);
    };
    let header = std::str::from_utf8(&data[..end]).map_err(|_| invalid("Invalid HTTP header"))?;
    let mut lines = header.split("\r\n");
    let first = lines
        .next()
        .ok_or_else(|| invalid("Missing request line"))?;
    let mut fields = first.split_whitespace();
    let method = fields
        .next()
        .ok_or_else(|| invalid("Missing method"))?
        .to_owned();
    let path = fields
        .next()
        .ok_or_else(|| invalid("Missing path"))?
        .to_owned();
    if fields.next() != Some("HTTP/1.1") || fields.next().is_some() {
        return Err(invalid("Unsupported HTTP version"));
    }
    let mut length = None;
    for line in lines.filter(|line| !line.is_empty()) {
        let (key, value) = line
            .split_once(':')
            .ok_or_else(|| invalid("Invalid header"))?;
        if key.eq_ignore_ascii_case("transfer-encoding") {
            return Err(invalid("Transfer encoding unsupported"));
        }
        if key.eq_ignore_ascii_case("content-length") {
            if length.is_some() {
                return Err(invalid("Duplicate body length"));
            }
            length = Some(
                value
                    .trim()
                    .parse::<usize>()
                    .map_err(|_| invalid("Invalid body length"))?,
            );
        }
    }
    let length = length.unwrap_or(0);
    if length > 1048576 {
        return Err(invalid("Body too large"));
    }
    while data.len() - end < length {
        let count = stream
            .read(&mut buffer)
            .map_err(|_| invalid("Cannot read body"))?;
        if count == 0 {
            return Err(invalid("Incomplete body"));
        }
        data.extend_from_slice(&buffer[..count]);
    }
    Ok((method, path, data[end..end + length].to_vec()))
}
pub(crate) fn response(result: Result<Value, Error>) -> (u16, Value) {
    match result {
        Ok(value) => (200, value),
        Err(Error::Contract(reason)) => (400, json!({"error":reason})),
        Err(Error::Failure { code, payload }) => {
            let status = match code.as_str() {
                "PersistenceUnavailable" => 503,
                "VotingClosed" => 409,
                _ => 404,
            };
            (status, json!({"failure":code,"payload":payload}))
        }
    }
}
pub(crate) fn route(
    method: &str,
    path: &str,
    body: &[u8],
) -> Result<(&'static str, Vec<Value>), Error> {
    let url = url::Url::parse(&format!("http://localhost{path}"))
        .map_err(|_| invalid("Invalid request URL"))?;
    let segments = url.path().trim_matches('/').split('/').collect::<Vec<_>>();
    let request = || -> Result<Value, Error> {
        let value: Value = serde_json::from_slice(body).map_err(|_| invalid("Invalid JSON"))?;
        if !value.is_object() {
            return Err(invalid("Expected object"));
        }
        Ok(value)
    };
    let with_id = |field: &str, id: &str| -> Result<Vec<Value>, Error> {
        let mut value = request()?;
        value[field] = json!(id);
        Ok(vec![value])
    };
    match (method, segments.as_slice()) {
        ("GET", ["films"]) => {
            let mut filter = json!({"query":null,"genre":null,"minimumStars":null,"availability":null,"maximumRuntimeMinutes":null,"releasedFrom":null,"sort":"TitleAscending","offset":0,"limit":20});
            for (key, value) in url.query_pairs() {
                if filter.get(key.as_ref()).is_some() {
                    filter[key.as_ref()] = if matches!(
                        key.as_ref(),
                        "maximumRuntimeMinutes" | "releasedFrom" | "offset" | "limit"
                    ) {
                        let number: Value = serde_json::from_str(&value)
                            .map_err(|_| invalid("Invalid integer query"))?;
                        crate::domain::exact_integer(&number)?;
                        number
                    } else {
                        json!(value)
                    };
                }
            }
            Ok(("searchFilms", vec![filter]))
        }
        ("GET", ["films", id]) => Ok(("getFilm", vec![json!(id)])),
        ("POST", ["films"]) => Ok(("saveFilm", vec![request()?])),
        ("PATCH", ["films", id, "availability"]) => {
            Ok(("changeFilmAvailability", with_id("filmId", id)?))
        }
        ("GET", ["films", id, "comments"]) => Ok(("listComments", vec![json!(id)])),
        ("POST", ["films", id, "comments"]) => Ok(("addComment", with_id("filmId", id)?)),
        ("GET", ["films", id, "rating"]) => {
            let member = url
                .query_pairs()
                .find(|(key, _)| key == "memberId")
                .map(|(_, value)| value.into_owned())
                .unwrap_or_default();
            Ok(("getFilmRating", vec![json!(id), json!(member)]))
        }
        ("PUT", ["films", id, "rating"]) => Ok(("rateFilm", with_id("filmId", id)?)),
        ("POST", ["voting"]) => Ok(("openVoting", vec![request()?])),
        ("GET", ["voting", "current"]) => Ok(("getCurrentVoting", vec![])),
        ("PUT", ["voting", id, "votes"]) => Ok(("castVote", with_id("pollId", id)?)),
        ("POST", ["voting", id, "close"]) => Ok(("closeVoting", vec![json!(id)])),
        _ => Err(invalid("Unknown HTTP route")),
    }
}
