//! Ordinary constructor-injected application service.
use crate::domain::{self, Error};
use serde_json::{Value, json};

pub trait Ports {
    fn invoke(&mut self, operation: &str, arguments: &[Value]) -> Result<Value, Error>;
}
#[derive(Debug, Clone)]
pub struct Event {
    pub name: String,
    pub payload: Value,
}
pub trait Publisher {
    fn publish(&mut self, event: Event) -> Result<(), Error>;
}
pub struct Service<P, E> {
    ports: P,
    publisher: E,
}
impl<P: Ports, E: Publisher> Service<P, E> {
    pub fn new(ports: P, publisher: E) -> Self {
        Self { ports, publisher }
    }
    pub fn parts_mut(&mut self) -> (&mut P, &mut E) {
        (&mut self.ports, &mut self.publisher)
    }
    pub fn invoke(&mut self, operation: &str, args: &[Value]) -> Result<Value, Error> {
        let (port, contracts): (&str, &[&str]) = match operation {
            "searchFilms" => ("Catalog.FilmCatalogStore.search", &["FilmSearch"]),
            "getFilm" => ("Catalog.FilmCatalogStore.find", &["FilmId"]),
            "saveFilm" => ("Catalog.FilmCatalogStore.save", &["SaveFilm"]),
            "changeFilmAvailability" => (
                "Catalog.FilmCatalogStore.changeAvailability",
                &["ChangeAvailability"],
            ),
            "listComments" => ("Community.CommentStore.listByFilm", &["FilmId"]),
            "addComment" => ("Community.CommentStore.append", &["AddComment"]),
            "getFilmRating" => ("Community.RatingStore.summary", &["FilmId", "MemberId"]),
            "rateFilm" => ("Community.RatingStore.upsert", &["RateFilm"]),
            "openVoting" => ("ScreeningPlanning.VotingStore.open", &["OpenVoting"]),
            "getCurrentVoting" => ("ScreeningPlanning.VotingStore.current", &[]),
            "castVote" => ("ScreeningPlanning.VotingStore.cast", &["CastVote"]),
            "closeVoting" => ("ScreeningPlanning.VotingStore.close", &["PollId"]),
            _ => return Err(Error::Contract("Unknown application operation".into())),
        };
        if args.len() != contracts.len() {
            return Err(Error::Contract("Incorrect operation arity".into()));
        }
        for (name, value) in contracts.iter().zip(args) {
            domain::validate(name, value)?;
        }
        let request = args.first().cloned().unwrap_or(Value::Null);
        if matches!(
            operation,
            "listComments" | "addComment" | "getFilmRating" | "rateFilm"
        ) {
            let film_id = if request.is_object() {
                request["filmId"].clone()
            } else {
                request.clone()
            };
            let present = self.ports.invoke(
                "Community.FilmPresence.exists",
                std::slice::from_ref(&film_id),
            )?;
            if present.is_null() {
                return Err(domain::failure("FilmNotFound", json!({"filmId":film_id})));
            }
        }
        let result = self.ports.invoke(port, args)?;
        if operation == "getFilm" && result.is_null() {
            return Err(domain::failure("FilmNotFound", json!({"filmId":request})));
        }
        let event = match operation {
            "saveFilm" => Some(("FilmSaved", json!({"filmId":request["id"]}))),
            "changeFilmAvailability" => Some(("FilmAvailabilityChanged", request.clone())),
            "addComment" => Some((
                "CommentAdded",
                json!({"commentId":result["id"],"filmId":request["filmId"]}),
            )),
            "rateFilm" => Some(("FilmRated", request.clone())),
            "openVoting" => Some((
                "VotingOpened",
                json!({"pollId":request["pollId"],"scheduledFor":request["scheduledFor"]}),
            )),
            "castVote" => Some(("VoteCast", request.clone())),
            "closeVoting" => Some((
                "VotingCompleted",
                json!({"pollId":request,"winningFilmId":result["winningFilmId"]}),
            )),
            _ => None,
        };
        if let Some((name, payload)) = event {
            domain::validate(name, &payload)?;
            self.publisher.publish(Event {
                name: name.into(),
                payload,
            })?;
        }
        Ok(result)
    }
}
