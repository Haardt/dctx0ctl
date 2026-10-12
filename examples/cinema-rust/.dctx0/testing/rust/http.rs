//! Actual TCP HTTP requests to production routes.
use crate::contract::Value;
use cinema::domain::{self, Error};
use serde_json::json;
use std::{io::Read, net::SocketAddr, time::Duration};
pub fn invoke(
    address: SocketAddr,
    name: &str,
    args: &[Value],
    framework: &str,
) -> Result<Value, Error> {
    let request = args.first().cloned().unwrap_or(Value::Null);
    let id = |field: &str| request[field].as_str().unwrap_or("").to_owned();
    let (method, mut path, body) = match name {
        "searchFilms" => {
            let mut url = url::Url::parse("http://localhost/films")
                .map_err(|e| Error::Contract(e.to_string()))?;
            for (key, value) in request
                .as_object()
                .ok_or_else(|| Error::Contract("Expected filter".into()))?
            {
                if !value.is_null() {
                    url.query_pairs_mut().append_pair(
                        key,
                        &value
                            .as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| value.to_string()),
                    );
                }
            }
            (
                "GET",
                format!("{}?{}", url.path(), url.query().unwrap_or("")),
                Value::Null,
            )
        }
        "getFilm" => (
            "GET",
            format!("/films/{}", request.as_str().unwrap_or("")),
            Value::Null,
        ),
        "saveFilm" => ("POST", "/films".into(), request.clone()),
        "changeFilmAvailability" => (
            "PATCH",
            format!("/films/{}/availability", id("filmId")),
            request.clone(),
        ),
        "listComments" => (
            "GET",
            format!("/films/{}/comments", request.as_str().unwrap_or("")),
            Value::Null,
        ),
        "addComment" => (
            "POST",
            format!("/films/{}/comments", id("filmId")),
            request.clone(),
        ),
        "getFilmRating" => (
            "GET",
            format!(
                "/films/{}/rating?memberId={}",
                request.as_str().unwrap_or(""),
                args.get(1).and_then(Value::as_str).unwrap_or("")
            ),
            Value::Null,
        ),
        "rateFilm" => (
            "PUT",
            format!("/films/{}/rating", id("filmId")),
            request.clone(),
        ),
        "openVoting" => ("POST", "/voting".into(), request.clone()),
        "getCurrentVoting" => ("GET", "/voting/current".into(), Value::Null),
        "castVote" => (
            "PUT",
            format!("/voting/{}/votes", id("pollId")),
            request.clone(),
        ),
        "closeVoting" => (
            "POST",
            format!("/voting/{}/close", request.as_str().unwrap_or("")),
            Value::Null,
        ),
        _ => return Err(Error::Contract("No explicit HTTP route".into())),
    };
    if path.is_empty() {
        path = "/".into();
    }
    let bytes = if body.is_null() {
        vec![]
    } else {
        serde_json::to_vec(&body).map_err(|e| Error::Contract(e.to_string()))?
    };
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| Error::Contract(e.to_string()))?;
    let response = client
        .request(
            reqwest::Method::from_bytes(method.as_bytes())
                .map_err(|e| Error::Contract(e.to_string()))?,
            format!("http://{address}{path}"),
        )
        .header("content-type", "application/json")
        .body(bytes)
        .send()
        .map_err(|e| Error::Contract(e.to_string()))?;
    if framework == "axum"
        && response
            .headers()
            .get("x-cinema-framework")
            .and_then(|v| v.to_str().ok())
            != Some("axum")
    {
        return Err(Error::Contract("Axum middleware missing".into()));
    }
    let status = response.status().as_u16();
    let mut bytes = Vec::new();
    response
        .take(1_048_577)
        .read_to_end(&mut bytes)
        .map_err(|e| Error::Contract(e.to_string()))?;
    if bytes.len() > 1_048_576 {
        return Err(Error::Contract("HTTP response exceeds budget".into()));
    }
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|e| Error::Contract(e.to_string()))?;
    if status == 200 {
        Ok(value)
    } else if let Some(code) = value["failure"].as_str() {
        let expected = match code {
            "PersistenceUnavailable" => 503,
            "VotingClosed" => 409,
            _ => 404,
        };
        if status != expected {
            return Err(Error::Contract("Wrong failure HTTP status".into()));
        }
        Err(domain::failure(code, value["payload"].clone()))
    } else {
        Err(Error::Contract(format!(
            "Unexpected HTTP status {status}: {}",
            json!(value)
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };
    fn with_response(response: &str, f: impl FnOnce(SocketAddr)) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let response = response.to_owned();
        let worker = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = [0; 4096];
            socket.read(&mut bytes).unwrap();
            socket.write_all(response.as_bytes()).unwrap();
        });
        f(address);
        worker.join().unwrap();
    }
    #[test]
    fn standard_client_decodes_chunked_http_and_checks_actual_status_and_middleware() {
        let body = r#"{"film":null}"#;
        with_response(
            &format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nX-Cinema-Framework: axum\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n{:x}\r\n{body}\r\n0\r\n\r\n",
                body.len()
            ),
            |address| {
                assert_eq!(
                    invoke(address, "getFilm", &[json!("id")], "axum").unwrap(),
                    json!({"film":null})
                );
            },
        );
        let body = r#"{"failure":"FilmNotFound","payload":{"filmId":"00000000-0000-0000-0000-000000000001"}}"#;
        with_response(
            &format!(
                "HTTP/1.1 503 Wrong\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            ),
            |address| {
                assert!(
                    matches!(invoke(address,"getFilm",&[json!("id")],"builtin"),Err(Error::Contract(message)) if message=="Wrong failure HTTP status")
                );
            },
        );
        with_response(
            "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
            |address| {
                assert!(
                    matches!(invoke(address,"getFilm",&[json!("id")],"axum"),Err(Error::Contract(message)) if message=="Axum middleware missing")
                );
            },
        );
    }
}
