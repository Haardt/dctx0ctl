use cinema::{
    application::{Event, Ports, Publisher, Service},
    domain::Error,
    httpapi::Server,
    persistence::{NoObserver, Sources, Store},
};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::TcpStream,
};
const FILM: &str = "00000000-0000-0000-0000-000000000001";
const OTHER: &str = "00000000-0000-0000-0000-000000000002";
const POLL: &str = "00000000-0000-0000-0000-000000000003";
const MEMBER: &str = "00000000-0000-0000-0000-000000000004";
struct Fixed;
impl Sources for Fixed {
    fn now(&mut self) -> Result<String, Error> {
        Ok("2026-10-12T00:00:00Z".into())
    }
    fn id(&mut self) -> Result<String, Error> {
        Ok("00000000-0000-0000-0000-000000000005".into())
    }
}
struct Events(Vec<Event>);
impl Publisher for Events {
    fn publish(&mut self, event: Event) -> Result<(), Error> {
        self.0.push(event);
        Ok(())
    }
}
fn film(id: &str, title: &str) -> Value {
    json!({"id":id,"title":title,"releaseYear":2020,"genres":["Drama"],"runtimeMinutes":90,"availability":"Available"})
}
#[test]
fn sqlite_replaces_ratings_votes_and_retains_schedule_after_closing() {
    let mut store = Store::open(":memory:", Fixed, NoObserver).unwrap();
    for (id, title) in [(FILM, "First"), (OTHER, "Second")] {
        store
            .invoke("Catalog.FilmCatalogStore.save", &[film(id, title)])
            .unwrap();
    }
    for stars in ["OneStar", "FiveStars"] {
        store
            .invoke(
                "Community.RatingStore.upsert",
                &[json!({"filmId":FILM,"memberId":MEMBER,"stars":stars})],
            )
            .unwrap();
    }
    let rating = store
        .invoke(
            "Community.RatingStore.summary",
            &[json!(FILM), json!(MEMBER)],
        )
        .unwrap();
    assert_eq!(rating["ratingCount"], 1);
    assert_eq!(rating["averageStars"], 5.0);
    store.invoke("ScreeningPlanning.VotingStore.open",&[json!({"pollId":POLL,"scheduledFor":"2026-10-12T00:00:00Z","candidateFilmIds":[FILM,OTHER]})]).unwrap();
    for id in [FILM, OTHER] {
        store
            .invoke(
                "ScreeningPlanning.VotingStore.cast",
                &[json!({"pollId":POLL,"memberId":MEMBER,"filmId":id})],
            )
            .unwrap();
    }
    let state = store.observe().unwrap();
    assert_eq!(state["FilmVote"].len(), 1);
    assert_eq!(state["FilmVote"][0]["filmId"], OTHER);
    let closed = store
        .invoke("ScreeningPlanning.VotingStore.close", &[json!(POLL)])
        .unwrap();
    assert_eq!(closed["winningFilmId"], OTHER);
    assert_eq!(closed["status"], "Closed");
    assert_eq!(closed["scheduledFor"], "2026-10-12T00:00:00Z");
    assert!(
        matches!(store.invoke("ScreeningPlanning.VotingStore.cast",&[json!({"pollId":POLL,"memberId":MEMBER,"filmId":FILM})]),Err(Error::Failure{code,..}) if code=="VotingClosed")
    );
}
#[test]
fn durable_film_survives_a_real_connection_reopen() {
    let directory =
        std::env::temp_dir().join(format!("cinema-rust-durable-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("store.sqlite3");
    let mut store = Store::open(&path, Fixed, NoObserver).unwrap();
    store
        .invoke("Catalog.FilmCatalogStore.save", &[film(FILM, "Stored")])
        .unwrap();
    store.close().unwrap();
    let mut reopened = Store::open(&path, Fixed, NoObserver).unwrap();
    assert_eq!(
        reopened
            .invoke("Catalog.FilmCatalogStore.find", &[json!(FILM)])
            .unwrap()["film"]["title"],
        "Stored"
    );
    reopened.close().unwrap();
    std::fs::remove_dir_all(directory).unwrap();
}
fn request(address: std::net::SocketAddr, method: &str, path: &str, body: Value) -> (u16, Value) {
    let data = if body.is_null() {
        vec![]
    } else {
        serde_json::to_vec(&body).unwrap()
    };
    let mut socket = TcpStream::connect(address).unwrap();
    write!(socket,"{method} {path} HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",data.len()).unwrap();
    socket.write_all(&data).unwrap();
    let mut response = Vec::new();
    socket.read_to_end(&mut response).unwrap();
    let end = response.windows(4).position(|v| v == b"\r\n\r\n").unwrap();
    let header = std::str::from_utf8(&response[..end]).unwrap();
    let status = header.split_whitespace().nth(1).unwrap().parse().unwrap();
    (
        status,
        serde_json::from_slice(&response[end + 4..]).unwrap(),
    )
}
#[test]
fn real_http_routes_validate_input_and_handle_parallel_requests() {
    let service = Service::new(
        Store::open(":memory:", Fixed, NoObserver).unwrap(),
        Events(vec![]),
    );
    let mut server = Server::start("127.0.0.1:0", service).unwrap();
    let address = server.address();
    assert_eq!(
        request(address, "GET", "/films/not-a-uuid", Value::Null).0,
        400
    );
    assert_eq!(
        request(address, "GET", &format!("/films/{FILM}"), Value::Null).0,
        404
    );
    let threads = (0..20)
        .map(|index| {
            std::thread::spawn(move || {
                let id = format!("00000000-0000-0000-0000-{index:012}");
                assert_eq!(
                    request(address, "POST", "/films", film(&id, "Concurrent")).0,
                    200
                );
            })
        })
        .collect::<Vec<_>>();
    for thread in threads {
        thread.join().unwrap();
    }
    let (status, overview) = request(address, "GET", "/films?limit=100", Value::Null);
    assert_eq!(status, 200);
    assert_eq!(overview["total"], 20);
    server.shutdown().unwrap();
}
