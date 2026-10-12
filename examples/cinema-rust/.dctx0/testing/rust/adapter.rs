//! External test adapter; domain behavior lives in the independent Cinema crate.
pub mod contract;
mod http;
mod translation;
use cinema::{
    application::{Event, Ports, Publisher, Service},
    axumapi,
    domain::{self, Error},
    httpapi::Server,
    persistence::{QueryObserver, Sources, Store},
};
use contract::*;
use serde_json::json;
use std::sync::{Arc, Mutex};
use translation::*;

struct TestSources {
    time: String,
    ids: Vec<String>,
    next: usize,
}
impl Sources for TestSources {
    fn now(&mut self) -> Result<String, Error> {
        Ok(self.time.clone())
    }
    fn id(&mut self) -> Result<String, Error> {
        let id = self
            .ids
            .get(self.next)
            .cloned()
            .ok_or_else(|| Error::Contract("Controlled IDs exhausted".into()))?;
        self.next += 1;
        Ok(id)
    }
}
struct SqlObserver(Context);
impl QueryObserver for SqlObserver {
    fn access(&mut self, operation: &str, entity: &str, access: &str) -> Result<(), Error> {
        self.0
            .resource(operation, &format!("{ROOT}{entity}"), access)
            .map_err(Error::Contract)
    }
}
type ActualStore = Store<TestSources, SqlObserver>;
#[derive(Clone)]
struct ObservedPorts {
    context: Context,
    store: Arc<Mutex<Option<ActualStore>>>,
    schema: Arc<Value>,
    real: bool,
}
fn actual<T>(f: impl FnOnce() -> Result<T, Error>) -> Result<T, Error> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))
        .unwrap_or_else(|_| Err(Error::Contract("Application panic".into())))
}
impl Ports for ObservedPorts {
    fn invoke(&mut self, subject: &str, args: &[Value]) -> Result<Value, Error> {
        let operation = format!("{ROOT}{subject}");
        let descriptor = self.schema["operations"]
            .get(&operation)
            .ok_or_else(|| Error::Contract("Missing port descriptor".into()))?;
        let wire = args
            .iter()
            .enumerate()
            .map(|(i, arg)| encode(&self.schema, arg, &descriptor["parameters"][i]["ty"]))
            .collect::<Result<Vec<_>, _>>()
            .map_err(Error::Contract)?;
        let invocation = json!({"subject":operation,"arguments":wire});
        if self.real {
            let result = actual(|| {
                self.store
                    .lock()
                    .map_err(|_| Error::Contract("Store lock poisoned".into()))?
                    .as_mut()
                    .ok_or_else(|| Error::Contract("Missing actual store".into()))?
                    .invoke(&operation, args)
            });
            let observation = outcome(&self.schema, result.clone(), &descriptor["result"])
                .map_err(Error::Contract)?;
            self.context
                .record_call(invocation, observation)
                .map_err(Error::Contract)?;
            result
        } else {
            let observation = self
                .context
                .collaborator(invocation.clone())
                .map_err(Error::Contract)?;
            self.context
                .record_call(invocation, observation.clone())
                .map_err(Error::Contract)?;
            match observation["kind"].as_str() {
                Some("returned") if observation["value"]["kind"] == "failure" => {
                    let failure = &observation["value"]["value"];
                    let name = failure["declaration"]
                        .as_str()
                        .and_then(|s| s.strip_prefix(ROOT))
                        .ok_or_else(|| Error::Contract("Invalid failure declaration".into()))?;
                    let payload = if failure["payload"].is_null() {
                        json!({})
                    } else {
                        decode(&failure["payload"]).map_err(Error::Contract)?
                    };
                    Err(domain::failure(name, payload))
                }
                Some("returned") => decode(&observation["value"]).map_err(Error::Contract),
                _ => Err(Error::Contract(
                    observation["message"]
                        .as_str()
                        .or_else(|| observation["reason"].as_str())
                        .unwrap_or("Unsupported collaborator observation")
                        .into(),
                )),
            }
        }
    }
}
#[derive(Clone)]
struct EventPublisher {
    context: Context,
    schema: Arc<Value>,
}
impl Publisher for EventPublisher {
    fn publish(&mut self, event: Event) -> Result<(), Error> {
        let wire =
            encode(&self.schema, &event.payload, &named(&event.name)).map_err(Error::Contract)?;
        self.context
            .emit(&format!("{ROOT}{}", event.name), wire)
            .map_err(Error::Contract)
    }
}
enum HttpServer {
    Builtin(Server),
    Axum(axumapi::Server),
}
impl HttpServer {
    fn address(&self) -> std::net::SocketAddr {
        match self {
            Self::Builtin(s) => s.address(),
            Self::Axum(s) => s.address(),
        }
    }
    fn framework(&self) -> &'static str {
        match self {
            Self::Builtin(_) => "builtin",
            Self::Axum(_) => "axum",
        }
    }
    fn shutdown(&mut self) -> std::io::Result<()> {
        match self {
            Self::Builtin(s) => s.shutdown(),
            Self::Axum(s) => s.shutdown(),
        }
    }
}

struct CinemaSession {
    schema: Arc<Value>,
    store: Arc<Mutex<Option<ActualStore>>>,
    service: Service<ObservedPorts, EventPublisher>,
    server: Option<HttpServer>,
}
pub fn describe() -> Result<Value, String> {
    let mut subjects=domain::contract_names().iter().map(|name|json!({"subject":format!("{ROOT}{name}"),"role":"contract","artifact":"src/domain.rs","entry_point":"domain::validate"})).collect::<Vec<_>>();
    for (subject, name) in SURFACES {
        subjects.push(json!({"subject":format!("{ROOT}{subject}"),"role":"operation","artifact":"src/application.rs","entry_point":format!("Service::invoke({name})")}));
    }
    for subject in PORTS {
        subjects.push(json!({"subject":format!("{ROOT}{subject}"),"role":"port","artifact":"src/persistence.rs","entry_point":"Store::invoke"}));
    }
    subjects.push(json!({"subject":format!("{ROOT}ScreeningPlanning.VotingRules.candidateOrderWinner"),"role":"operation","artifact":"src/domain.rs","entry_point":"candidate_order_winner"}));
    subjects.sort_by(|a, b| a["subject"].as_str().cmp(&b["subject"].as_str()));
    Ok(
        json!({"protocol_version":1,"subjects":subjects,"artifacts":["src/domain.rs","src/application.rs","src/persistence.rs","src/httpapi.rs","src/axumapi.rs","src/main.rs"],"capabilities":["sessions","invocation","events","values","call-observation","mock-injection","state","real-ports","resource-observation","controlled-time","controlled-ids"],"representations":{"Int":"serde_json arbitrary precision and num_bigint checked conversion","Float":"finite f64","UUID":"validated text","Timestamp":"RFC3339 text","Option":"null or value","variants":"closed alternative names"}}),
    )
}
fn includes(config: &Value, key: &str, value: &str) -> bool {
    config[key]
        .as_array()
        .is_some_and(|items| items.iter().any(|v| v.as_str() == Some(value)))
}
pub fn begin(case: &str, context: Context) -> Result<Box<dyn Session>, String> {
    let schema = Arc::new(model()?);
    let config: Value =
        serde_json::from_slice(&std::fs::read("config.json").map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let framework = config
        .get("httpFramework")
        .map(|value| value.as_str().ok_or("httpFramework must be a string"))
        .transpose()?
        .unwrap_or("builtin");
    if !["builtin", "axum"].contains(&framework) {
        return Err("httpFramework must be builtin or axum".into());
    }
    let mode = config["mode"].as_str().ok_or("Missing mode")?;
    if !["real", "adaptive", "mock"].contains(&mode) {
        return Err("Invalid session mode".into());
    }
    let real = mode == "real"
        || (mode == "adaptive"
            && (includes(&config, "realCases", case)
                || (config["realGeneratedOperations"] == true
                    && case.starts_with(ROOT)
                    && case.contains(":operation:"))));
    let store = if real {
        let database = config["database"]
            .as_str()
            .ok_or("Explicit test database required")?;
        let time = config["time"]
            .as_str()
            .ok_or("Missing controlled time")?
            .into();
        let ids = serde_json::from_value(config["ids"].clone()).map_err(|e| e.to_string())?;
        Some(
            Store::open(
                database,
                TestSources { time, ids, next: 0 },
                SqlObserver(context.clone()),
            )
            .map_err(|e| e.to_string())?,
        )
    } else {
        None
    };
    let store = Arc::new(Mutex::new(store));
    let ports = ObservedPorts {
        context: context.clone(),
        store: store.clone(),
        schema: schema.clone(),
        real,
    };
    let publisher = EventPublisher {
        context,
        schema: schema.clone(),
    };
    let http_case = includes(&config, "httpCases", case)
        || (framework == "axum"
            && real
            && SURFACES
                .iter()
                .any(|(subject, _)| case.starts_with(&format!("{ROOT}{subject}:operation:"))));
    let server = if http_case {
        if !real {
            return Err("HTTP requires a real session".into());
        }
        let service = Service::new(ports.clone(), publisher.clone());
        Some(match framework {
            "axum" => HttpServer::Axum(
                axumapi::Server::start("127.0.0.1:0", service).map_err(|e| e.to_string())?,
            ),
            _ => HttpServer::Builtin(
                Server::start("127.0.0.1:0", service).map_err(|e| e.to_string())?,
            ),
        })
    } else {
        None
    };
    Ok(Box::new(CinemaSession {
        schema,
        store,
        service: Service::new(ports, publisher),
        server,
    }))
}
impl Session for CinemaSession {
    fn invoke(&mut self, invocation: Value) -> Result<Value, String> {
        let full = invocation["subject"].as_str().ok_or("Missing subject")?;
        let subject = full.strip_prefix(ROOT).ok_or("Subject outside Cinema")?;
        let descriptor = self.schema["operations"]
            .get(full)
            .ok_or("Missing operation descriptor")?;
        let args = invocation["arguments"]
            .as_array()
            .ok_or("Missing arguments")?
            .iter()
            .map(decode)
            .collect::<Result<Vec<_>, _>>()?;
        let result = if subject == "ScreeningPlanning.VotingRules.candidateOrderWinner" {
            if args.len() != 2 {
                return Err("Incorrect pure operation arity".into());
            }
            actual(|| {
                Ok(domain::candidate_order_winner(
                    args[0]
                        .as_array()
                        .ok_or_else(|| Error::Contract("Expected votes".into()))?,
                    args[1]
                        .as_array()
                        .ok_or_else(|| Error::Contract("Expected candidates".into()))?,
                ))
            })
        } else if PORTS.contains(&subject) {
            actual(|| {
                self.store
                    .lock()
                    .map_err(|_| Error::Contract("Store lock poisoned".into()))?
                    .as_mut()
                    .ok_or_else(|| Error::Contract("No actual port entry".into()))?
                    .invoke(full, &args)
            })
        } else if let Some((_, name)) = SURFACES.iter().find(|(s, _)| *s == subject) {
            if let Some(server) = &self.server {
                http::invoke(server.address(), name, &args, server.framework())
            } else {
                actual(|| self.service.invoke(name, &args))
            }
        } else {
            return Ok(json!({"kind":"unavailable","reason":"No actual entry point"}));
        };
        outcome(&self.schema, result, &descriptor["result"])
    }
    fn validate(&mut self, subject: &str, wire: Value) -> Result<Value, String> {
        let name = subject
            .strip_prefix(ROOT)
            .ok_or("Unknown contract namespace")?;
        if !domain::contract_names().contains(&name) {
            return Ok(json!({"kind":"unavailable","reason":"No actual validator"}));
        }
        let value = decode(&wire)?;
        Ok(match domain::validate(name, &value) {
            Ok(()) => json!({"kind":"accepted","value":wire}),
            Err(error) => json!({"kind":"rejected","reason":error.to_string()}),
        })
    }
    fn observe(&mut self, entities: &[String]) -> Result<Value, String> {
        let actual = self
            .store
            .lock()
            .map_err(|_| "Store lock poisoned")?
            .as_mut()
            .ok_or("No actual state observer")?
            .observe()
            .map_err(|e| e.to_string())?;
        let mut out = serde_json::Map::new();
        for entity in entities {
            let name = entity
                .strip_prefix(ROOT)
                .ok_or("Unknown entity namespace")?;
            let values = actual
                .get(name)
                .ok_or("Unknown entity")?
                .iter()
                .map(|row| encode(&self.schema, row, &named(name)))
                .collect::<Result<Vec<_>, _>>()?;
            out.insert(entity.clone(), json!(values));
        }
        Ok(Value::Object(out))
    }
    fn close(&mut self) -> Result<(), String> {
        if let Some(mut server) = self.server.take() {
            server.shutdown().map_err(|e| e.to_string())?;
        }
        if let Some(store) = self.store.lock().map_err(|_| "Store lock poisoned")?.take() {
            store.close().map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

const PORTS: &[&str] = &[
    "Catalog.FilmCatalogStore.search",
    "Catalog.FilmCatalogStore.find",
    "Catalog.FilmCatalogStore.save",
    "Catalog.FilmCatalogStore.changeAvailability",
    "Community.FilmPresence.exists",
    "Community.CommentStore.listByFilm",
    "Community.CommentStore.append",
    "Community.RatingStore.summary",
    "Community.RatingStore.upsert",
    "ScreeningPlanning.VotingStore.open",
    "ScreeningPlanning.VotingStore.current",
    "ScreeningPlanning.VotingStore.cast",
    "ScreeningPlanning.VotingStore.close",
];
const SURFACES: &[(&str, &str)] = &[
    ("Catalog.FilmCatalogApi.searchFilms", "searchFilms"),
    ("Catalog.FilmCatalogApi.getFilm", "getFilm"),
    ("Catalog.FilmCatalogApi.saveFilm", "saveFilm"),
    (
        "Catalog.FilmCatalogApi.changeFilmAvailability",
        "changeFilmAvailability",
    ),
    ("Community.FilmCommunityApi.listComments", "listComments"),
    ("Community.FilmCommunityApi.addComment", "addComment"),
    ("Community.FilmCommunityApi.getFilmRating", "getFilmRating"),
    ("Community.FilmCommunityApi.rateFilm", "rateFilm"),
    (
        "ScreeningPlanning.ScreeningPlanningApi.openVoting",
        "openVoting",
    ),
    (
        "ScreeningPlanning.ScreeningPlanningApi.getCurrentVoting",
        "getCurrentVoting",
    ),
    (
        "ScreeningPlanning.ScreeningPlanningApi.castVote",
        "castVote",
    ),
    (
        "ScreeningPlanning.ScreeningPlanningApi.closeVoting",
        "closeVoting",
    ),
];
