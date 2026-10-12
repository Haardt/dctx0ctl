#![cfg(feature = "axum-http")]
use axum::{
    body::{Body, to_bytes},
    http::Request,
};
use cinema::{
    axumapi::{Server, router},
    domain::{Error, failure},
    httpapi::HttpService,
};
use serde_json::{Value, json};
use tower::ServiceExt;
struct ActualBoundary;
impl HttpService for ActualBoundary {
    fn invoke(&mut self, operation: &str, args: &[Value]) -> Result<Value, Error> {
        if args.first() == Some(&json!("00000000-0000-0000-0000-000000000001")) {
            return Err(failure(
                "FilmNotFound",
                json!({"filmId":"00000000-0000-0000-0000-000000000001"}),
            ));
        }
        if args.first() == Some(&json!("00000000-0000-0000-0000-000000000002")) {
            return Err(failure(
                "PersistenceUnavailable",
                json!({"reason":"00000000-0000-0000-0000-000000000002"}),
            ));
        }
        if args.first() == Some(&json!("00000000-0000-0000-0000-000000000003")) {
            return Err(failure(
                "VotingClosed",
                json!({"pollId":"00000000-0000-0000-0000-000000000003"}),
            ));
        }
        Ok(json!({"operation":operation,"arguments":args}))
    }
}
#[test]
fn actual_router_preserves_route_arguments_rejections_and_middleware() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let app = router(ActualBoundary);
        let cases = [
            ("GET", "/films?offset=7&query=Movie", "", 200, "searchFilms"),
            ("GET", "/films/id", "", 200, "getFilm"),
            ("POST", "/films", "{}", 200, "saveFilm"),
            (
                "PATCH",
                "/films/id/availability",
                "{}",
                200,
                "changeFilmAvailability",
            ),
            ("GET", "/films/id/comments", "", 200, "listComments"),
            ("POST", "/films/id/comments", "{}", 200, "addComment"),
            (
                "GET",
                "/films/id/rating?memberId=member",
                "",
                200,
                "getFilmRating",
            ),
            ("PUT", "/films/id/rating", "{}", 200, "rateFilm"),
            ("POST", "/voting", "{}", 200, "openVoting"),
            ("GET", "/voting/current", "", 200, "getCurrentVoting"),
            ("PUT", "/voting/id/votes", "{}", 200, "castVote"),
            ("POST", "/voting/id/close", "", 200, "closeVoting"),
            (
                "GET",
                "/films/00000000-0000-0000-0000-000000000001",
                "",
                404,
                "",
            ),
            (
                "GET",
                "/films/00000000-0000-0000-0000-000000000002",
                "",
                503,
                "",
            ),
            (
                "POST",
                "/voting/00000000-0000-0000-0000-000000000003/close",
                "",
                409,
                "",
            ),
            ("GET", "/films?offset=bad", "", 400, ""),
            ("POST", "/films", "{", 400, ""),
            ("GET", "/unknown", "", 404, ""),
            ("DELETE", "/films/id", "", 405, ""),
        ];
        for (method, path, body, status, operation) in cases {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(path)
                        .header("content-type", "application/json")
                        .body(Body::from(body))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status().as_u16(), status, "{method} {path}");
            assert_eq!(response.headers()["x-cinema-framework"], "axum");
            assert_eq!(response.headers()["content-type"], "application/json");
            let body: Value =
                serde_json::from_slice(&to_bytes(response.into_body(), 1_048_576).await.unwrap())
                    .unwrap();
            if !operation.is_empty() {
                assert_eq!(body["operation"], operation);
            }
            if path == "/films?offset=7&query=Movie" {
                assert_eq!(body["arguments"][0]["offset"], 7);
            }
            if path == "/films/id/rating?memberId=member" {
                assert_eq!(body["arguments"], json!(["id", "member"]));
            }
        }
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/films")
                    .body(Body::from(vec![b'x'; 1_048_577]))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), 413);
    });
}
#[test]
fn loopback_server_sessions_close_and_release_the_actual_port() {
    for _ in 0..3 {
        let mut server = Server::start("127.0.0.1:0", ActualBoundary).unwrap();
        let address = server.address();
        assert!(std::net::TcpStream::connect(address).is_ok());
        server.shutdown().unwrap();
        server.shutdown().unwrap();
        assert!(std::net::TcpStream::connect(address).is_err());
        assert!(std::net::TcpListener::bind(address).is_ok());
    }
    assert!(Server::start("invalid-address", ActualBoundary).is_err());
}

struct FixedSources;
impl cinema::persistence::Sources for FixedSources {
    fn now(&mut self) -> Result<String, Error> {
        Ok("2026-10-12T00:00:00Z".into())
    }
    fn id(&mut self) -> Result<String, Error> {
        Ok("00000000-0000-0000-0000-000000000005".into())
    }
}
struct Publisher;
impl cinema::application::Publisher for Publisher {
    fn publish(&mut self, _: cinema::application::Event) -> Result<(), Error> {
        Ok(())
    }
}
#[test]
fn routers_use_real_sqlite_services_and_do_not_share_state_between_sessions() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let film_id="00000000-0000-0000-0000-000000000010";
        for round in 0..2 {
            let store=cinema::persistence::Store::open(":memory:",FixedSources,cinema::persistence::NoObserver).unwrap();
            let app=router(cinema::application::Service::new(store,Publisher));
            if round==0 {
                let film=json!({"id":film_id,"title":"Axum Movie","releaseYear":2026,"genres":[],"runtimeMinutes":90,"availability":"Available"});
                let response=app.clone().oneshot(Request::builder().method("POST").uri("/films").header("content-type","application/json").body(Body::from(serde_json::to_vec(&film).unwrap())).unwrap()).await.unwrap();
                assert_eq!(response.status().as_u16(),200);
            }
            let response=app.oneshot(Request::builder().uri(format!("/films/{film_id}")).body(Body::empty()).unwrap()).await.unwrap();
            assert_eq!(response.status().as_u16(),if round==0 {200}else{404});
            let body:Value=serde_json::from_slice(&to_bytes(response.into_body(),1_048_576).await.unwrap()).unwrap();
            if round==0 {assert_eq!(body["film"]["title"],"Axum Movie");}else{assert_eq!(body["failure"],"FilmNotFound");}
        }
    });
}
