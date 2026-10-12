use cinema::{
    application::{Event, Publisher, Service},
    domain::Error,
    httpapi::Server,
    persistence::{NoObserver, Sources, Store},
};
use std::{io::Read, time::Duration};
struct ProductionSources;
impl Sources for ProductionSources {
    fn now(&mut self) -> Result<String, Error> {
        Ok(chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true))
    }
    fn id(&mut self) -> Result<String, Error> {
        let mut bytes = [0u8; 16];
        std::fs::File::open("/dev/urandom")
            .and_then(|mut file| file.read_exact(&mut bytes))
            .map_err(|err| {
                cinema::domain::failure(
                    "PersistenceUnavailable",
                    serde_json::json!({"reason":err.to_string()}),
                )
            })?;
        bytes[6] = (bytes[6] & 15) | 64;
        bytes[8] = (bytes[8] & 63) | 128;
        Ok(format!(
            "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            bytes[0],
            bytes[1],
            bytes[2],
            bytes[3],
            bytes[4],
            bytes[5],
            bytes[6],
            bytes[7],
            bytes[8],
            bytes[9],
            bytes[10],
            bytes[11],
            bytes[12],
            bytes[13],
            bytes[14],
            bytes[15]
        ))
    }
}
struct LogPublisher;
impl Publisher for LogPublisher {
    fn publish(&mut self, event: Event) -> Result<(), Error> {
        eprintln!("{} {}", event.name, event.payload);
        Ok(())
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database = std::env::var("CINEMA_DATABASE").unwrap_or_else(|_| "cinema.sqlite3".into());
    let address = std::env::var("CINEMA_ADDRESS").unwrap_or_else(|_| "127.0.0.1:4182".into());
    let store = Store::open(database, ProductionSources, NoObserver)?;
    let framework = std::env::var("CINEMA_HTTP_FRAMEWORK").unwrap_or_else(|_| "builtin".into());
    let service = Service::new(store, LogPublisher);
    if framework == "axum" {
        #[cfg(feature = "axum-http")]
        {
            let server = cinema::axumapi::Server::start(&address, service)?;
            eprintln!("Cinema (Axum) listening on http://{}", server.address());
            loop {
                std::thread::park_timeout(Duration::from_secs(3600));
            }
        }
        #[cfg(not(feature = "axum-http"))]
        return Err("Axum requires the axum-http Cargo feature".into());
    }
    if framework != "builtin" {
        return Err("CINEMA_HTTP_FRAMEWORK must be builtin or axum".into());
    }
    let server = Server::start(&address, service)?;
    eprintln!("Cinema listening on http://{}", server.address());
    loop {
        std::thread::park_timeout(Duration::from_secs(3600));
    }
}
