//! Real SQLite storage, actual query observations and transactional voting changes.
use crate::{
    application::Ports,
    domain::{self, Error},
};
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use std::{cmp::Ordering, collections::BTreeMap, path::Path};

pub trait Sources {
    fn now(&mut self) -> Result<String, Error>;
    fn id(&mut self) -> Result<String, Error>;
}
pub trait QueryObserver {
    fn access(&mut self, operation: &str, entity: &str, access: &str) -> Result<(), Error>;
}
pub struct NoObserver;
impl QueryObserver for NoObserver {
    fn access(&mut self, _: &str, _: &str, _: &str) -> Result<(), Error> {
        Ok(())
    }
}
pub struct Store<S, O> {
    db: Connection,
    sources: S,
    observer: O,
    operation: String,
}
fn database_error(e: impl std::fmt::Display) -> Error {
    domain::failure("PersistenceUnavailable", json!({"reason":e.to_string()}))
}
fn entity(table: &str) -> &str {
    match table {
        "films" => "Film",
        "comments" => "Comment",
        "ratings" => "Rating",
        "polls" => "ScreeningPoll",
        "votes" => "FilmVote",
        _ => "Unknown",
    }
}
fn star(v: &Value) -> usize {
    match v.as_str() {
        Some("OneStar") => 1,
        Some("TwoStars") => 2,
        Some("ThreeStars") => 3,
        Some("FourStars") => 4,
        Some("FiveStars") => 5,
        _ => 0,
    }
}
fn number(v: &Value) -> i64 {
    v.as_i64().unwrap_or(0)
}
impl<S: Sources, O: QueryObserver> Store<S, O> {
    pub fn open(path: impl AsRef<Path>, sources: S, observer: O) -> Result<Self, Error> {
        let db = Connection::open(path).map_err(database_error)?;
        db.execute_batch("PRAGMA foreign_keys=ON;
CREATE TABLE IF NOT EXISTS films(id TEXT PRIMARY KEY,document TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS comments(id TEXT PRIMARY KEY,film_id TEXT NOT NULL REFERENCES films(id),document TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS ratings(film_id TEXT NOT NULL REFERENCES films(id),member_id TEXT NOT NULL,document TEXT NOT NULL,PRIMARY KEY(film_id,member_id));
CREATE TABLE IF NOT EXISTS polls(id TEXT PRIMARY KEY,document TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS votes(poll_id TEXT NOT NULL REFERENCES polls(id),member_id TEXT NOT NULL,film_id TEXT NOT NULL REFERENCES films(id),document TEXT NOT NULL,PRIMARY KEY(poll_id,member_id));").map_err(database_error)?;
        Ok(Self {
            db,
            sources,
            observer,
            operation: String::new(),
        })
    }
    pub fn close(self) -> Result<(), Error> {
        self.db.close().map_err(|(_, err)| database_error(err))
    }
    fn access(&mut self, table: &str, kind: &str) -> Result<(), Error> {
        if self.operation.is_empty() {
            Ok(())
        } else {
            self.observer.access(&self.operation, entity(table), kind)
        }
    }
    fn rows(&mut self, table: &str) -> Result<Vec<Value>, Error> {
        let result: Result<Vec<Value>, Error> = (|| {
            let mut statement = self
                .db
                .prepare(&format!("SELECT document FROM {table} ORDER BY rowid"))
                .map_err(database_error)?;
            let values = statement
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(database_error)?;
            values
                .map(|row| {
                    serde_json::from_str(&row.map_err(database_error)?).map_err(database_error)
                })
                .collect()
        })();
        let observed = self.access(table, "read");
        let rows = result?;
        observed?;
        Ok(rows)
    }
    fn put(&mut self, table: &str, value: &Value) -> Result<(), Error> {
        let document = serde_json::to_string(value).map_err(database_error)?;
        let s = |field: &str| value[field].as_str().unwrap_or("").to_owned();
        let result=match table {
            "films"|"polls"=>self.db.execute(&format!("INSERT INTO {table} VALUES(?,?) ON CONFLICT(id) DO UPDATE SET document=excluded.document"),params![s("id"),document]),
            "comments"=>self.db.execute("INSERT INTO comments VALUES(?,?,?)",params![s("id"),s("filmId"),document]),
            "ratings"=>self.db.execute("INSERT INTO ratings VALUES(?,?,?) ON CONFLICT(film_id,member_id) DO UPDATE SET document=excluded.document",params![s("filmId"),s("memberId"),document]),
            "votes"=>self.db.execute("INSERT INTO votes VALUES(?,?,?,?) ON CONFLICT(poll_id,member_id) DO UPDATE SET film_id=excluded.film_id,document=excluded.document",params![s("pollId"),s("memberId"),s("filmId"),document]),
            _=>return Err(Error::Contract("Unknown storage table".into())),
        };
        let observed = self.access(table, "write");
        result.map_err(database_error)?;
        observed
    }
    fn find(&mut self, table: &str, id: &Value) -> Result<Value, Error> {
        Ok(self
            .rows(table)?
            .into_iter()
            .find(|r| r["id"] == *id)
            .unwrap_or(Value::Null))
    }
    fn require_film(&mut self, id: &Value) -> Result<Value, Error> {
        let film = self.find("films", id)?;
        if film.is_null() {
            Err(domain::failure("FilmNotFound", json!({"filmId":id})))
        } else {
            Ok(film)
        }
    }
    fn require_poll(&mut self, id: &Value) -> Result<Value, Error> {
        let poll = self.find("polls", id)?;
        if poll.is_null() {
            Err(domain::failure("PollNotFound", json!({"pollId":id})))
        } else if poll["status"] != "Open" {
            Err(domain::failure("VotingClosed", json!({"pollId":id})))
        } else {
            Ok(poll)
        }
    }
    fn rating_summary(&mut self, film: &Value, member: &Value) -> Result<Value, Error> {
        let mut count = 0;
        let mut total = 0;
        let mut current = Value::Null;
        for row in self.rows("ratings")? {
            if row["filmId"] == *film {
                count += 1;
                total += star(&row["stars"]);
                if row["memberId"] == *member {
                    current = row["stars"].clone();
                }
            }
        }
        let average = if count == 0 {
            0.0
        } else {
            total as f64 / count as f64
        };
        Ok(json!({"filmId":film,"averageStars":average,"ratingCount":count,"memberRating":current}))
    }
    fn summary(&mut self, mut film: Value) -> Result<Value, Error> {
        let rating = self.rating_summary(&film["id"], &Value::Null)?;
        film["averageStars"] = rating["averageStars"].clone();
        film["ratingCount"] = rating["ratingCount"].clone();
        let current = self
            .rows("polls")?
            .into_iter()
            .rfind(|r| r["status"] == "Open");
        let count = if let Some(poll) = current {
            self.rows("votes")?
                .iter()
                .filter(|v| v["pollId"] == poll["id"] && v["filmId"] == film["id"])
                .count()
        } else {
            0
        };
        film["currentVoteCount"] = json!(count);
        Ok(film)
    }
    fn details(&mut self, film: Value) -> Result<Value, Error> {
        Ok(json!({"film":self.summary(film)?}))
    }
    fn overview(&mut self, poll: Value) -> Result<Value, Error> {
        let votes = self
            .rows("votes")?
            .into_iter()
            .filter(|v| v["pollId"] == poll["id"])
            .map(|v| v["filmId"].clone())
            .collect::<Vec<_>>();
        let candidates = poll["candidateFilmIds"]
            .as_array()
            .ok_or_else(|| Error::Contract("Invalid stored poll".into()))?;
        let results = candidates
            .iter()
            .map(|id| json!({"filmId":id,"votes":votes.iter().filter(|v|*v==id).count()}))
            .collect::<Vec<_>>();
        Ok(
            json!({"pollId":poll["id"],"scheduledFor":poll["scheduledFor"],"status":poll["status"],"results":results,"winningFilmId":domain::candidate_order_winner(&votes,candidates)}),
        )
    }
    fn search(&mut self, filter: &Value) -> Result<Value, Error> {
        let mut items = Vec::new();
        for film in self.rows("films")? {
            let r = self.summary(film)?;
            if filter["query"].as_str().is_some_and(|q| {
                !r["title"]
                    .as_str()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&q.to_lowercase())
            }) {
                continue;
            }
            if !filter["genre"].is_null()
                && !r["genres"]
                    .as_array()
                    .is_some_and(|g| g.contains(&filter["genre"]))
            {
                continue;
            }
            if !filter["availability"].is_null() && filter["availability"] != r["availability"] {
                continue;
            }
            if !filter["maximumRuntimeMinutes"].is_null()
                && number(&r["runtimeMinutes"]) > number(&filter["maximumRuntimeMinutes"])
            {
                continue;
            }
            if !filter["releasedFrom"].is_null()
                && number(&r["releaseYear"]) < number(&filter["releasedFrom"])
            {
                continue;
            }
            if !filter["minimumStars"].is_null()
                && r["averageStars"].as_f64().unwrap_or(0.0) < star(&filter["minimumStars"]) as f64
            {
                continue;
            }
            items.push(r);
        }
        items.sort_by(|a, b| {
            let primary = match filter["sort"].as_str() {
                Some("NewestFirst") => number(&b["releaseYear"]).cmp(&number(&a["releaseYear"])),
                Some("HighestRated") => b["averageStars"]
                    .as_f64()
                    .unwrap_or(0.0)
                    .total_cmp(&a["averageStars"].as_f64().unwrap_or(0.0)),
                Some("MostVoted") => {
                    number(&b["currentVoteCount"]).cmp(&number(&a["currentVoteCount"]))
                }
                _ => a["title"].as_str().cmp(&b["title"].as_str()),
            };
            if primary == Ordering::Equal {
                a["id"].as_str().cmp(&b["id"].as_str())
            } else {
                primary
            }
        });
        let total = items.len();
        let start = (number(&filter["offset"]) as usize).min(total);
        let end = (start + number(&filter["limit"]) as usize).min(total);
        Ok(
            json!({"items":items[start..end],"total":total,"offset":filter["offset"],"limit":filter["limit"]}),
        )
    }
    pub fn observe(&mut self) -> Result<BTreeMap<String, Vec<Value>>, Error> {
        let previous = std::mem::take(&mut self.operation);
        let result = ["films", "comments", "ratings", "polls", "votes"]
            .into_iter()
            .map(|t| Ok((entity(t).into(), self.rows(t)?)))
            .collect();
        self.operation = previous;
        result
    }
    fn execute(&mut self, operation: &str, args: &[Value]) -> Result<Value, Error> {
        let request = args.first().cloned().unwrap_or(Value::Null);
        let contracts: &[&str] = match operation {
            "Catalog.FilmCatalogStore.search" => &["FilmSearch"],
            "Catalog.FilmCatalogStore.find"
            | "Community.FilmPresence.exists"
            | "Community.CommentStore.listByFilm" => &["FilmId"],
            "Catalog.FilmCatalogStore.save" => &["SaveFilm"],
            "Catalog.FilmCatalogStore.changeAvailability" => &["ChangeAvailability"],
            "Community.CommentStore.append" => &["AddComment"],
            "Community.RatingStore.summary" => &["FilmId", "MemberId"],
            "Community.RatingStore.upsert" => &["RateFilm"],
            "ScreeningPlanning.VotingStore.open" => &["OpenVoting"],
            "ScreeningPlanning.VotingStore.current" => &[],
            "ScreeningPlanning.VotingStore.cast" => &["CastVote"],
            "ScreeningPlanning.VotingStore.close" => &["PollId"],
            _ => return Err(Error::Contract("Unknown storage operation".into())),
        };
        if args.len() != contracts.len() {
            return Err(Error::Contract("Incorrect port arity".into()));
        }
        for (name, value) in contracts.iter().zip(args) {
            domain::validate(name, value)?;
        }
        match operation {
            "Catalog.FilmCatalogStore.search" => self.search(&request),
            "Catalog.FilmCatalogStore.find" => {
                let film = self.find("films", &request)?;
                if film.is_null() {
                    Ok(Value::Null)
                } else {
                    self.details(film)
                }
            }
            "Catalog.FilmCatalogStore.save" => {
                self.put("films", &request)?;
                self.details(request)
            }
            "Catalog.FilmCatalogStore.changeAvailability" => {
                let mut film = self.require_film(&request["filmId"])?;
                film["availability"] = request["availability"].clone();
                self.put("films", &film)?;
                self.details(film)
            }
            "Community.FilmPresence.exists" => {
                let film = self.find("films", &request)?;
                Ok(film["id"].clone())
            }
            "Community.CommentStore.listByFilm" => {
                let items = self
                    .rows("comments")?
                    .into_iter()
                    .filter(|r| r["filmId"] == request)
                    .collect::<Vec<_>>();
                Ok(json!({"total":items.len(),"items":items}))
            }
            "Community.CommentStore.append" => {
                let mut comment = request;
                comment["id"] = json!(self.sources.id()?);
                comment["createdAt"] = json!(self.sources.now()?);
                domain::validate("Comment", &comment)?;
                self.put("comments", &comment)?;
                Ok(comment)
            }
            "Community.RatingStore.summary" => self.rating_summary(&args[0], &args[1]),
            "Community.RatingStore.upsert" => {
                self.put("ratings", &request)?;
                self.rating_summary(&request["filmId"], &request["memberId"])
            }
            "ScreeningPlanning.VotingStore.open" => {
                let poll = json!({"id":request["pollId"],"scheduledFor":request["scheduledFor"],"candidateFilmIds":request["candidateFilmIds"],"status":"Open"});
                let deleted = self.db.execute(
                    "DELETE FROM votes WHERE poll_id=?",
                    params![poll["id"].as_str()],
                );
                let observed = self.access("votes", "write");
                deleted.map_err(database_error)?;
                observed?;
                self.put("polls", &poll)?;
                self.overview(poll)
            }
            "ScreeningPlanning.VotingStore.current" => {
                let polls = self.rows("polls")?;
                let poll = polls
                    .iter()
                    .rfind(|p| p["status"] == "Open")
                    .or_else(|| polls.last())
                    .cloned();
                if let Some(poll) = poll {
                    self.overview(poll)
                } else {
                    Ok(Value::Null)
                }
            }
            "ScreeningPlanning.VotingStore.cast" => {
                let poll = self.require_poll(&request["pollId"])?;
                self.require_film(&request["filmId"])?;
                if !poll["candidateFilmIds"]
                    .as_array()
                    .is_some_and(|ids| ids.contains(&request["filmId"]))
                {
                    return Err(domain::failure(
                        "FilmNotFound",
                        json!({"filmId":request["filmId"]}),
                    ));
                }
                self.put("votes", &request)?;
                self.overview(poll)
            }
            "ScreeningPlanning.VotingStore.close" => {
                let mut poll = self.require_poll(&request)?;
                poll["status"] = json!("Closed");
                self.put("polls", &poll)?;
                self.overview(poll)
            }
            _ => Err(Error::Contract("Unknown storage operation".into())),
        }
    }
}
impl<S: Sources, O: QueryObserver> Ports for Store<S, O> {
    fn invoke(&mut self, operation: &str, args: &[Value]) -> Result<Value, Error> {
        let normalized = operation
            .strip_prefix("HomeCinema.MovieNight.")
            .unwrap_or(operation);
        let previous = std::mem::replace(&mut self.operation, operation.into());
        let transactional = matches!(
            normalized,
            "ScreeningPlanning.VotingStore.open"
                | "ScreeningPlanning.VotingStore.cast"
                | "ScreeningPlanning.VotingStore.close"
        );
        let result = (|| {
            if transactional {
                self.db
                    .execute_batch("BEGIN IMMEDIATE")
                    .map_err(database_error)?;
            }
            let result = self.execute(normalized, args);
            if transactional {
                match &result {
                    Ok(_) => {
                        if let Err(commit_error) = self.db.execute_batch("COMMIT") {
                            if let Err(rollback_error) = self.db.execute_batch("ROLLBACK") {
                                return Err(database_error(format!(
                                    "Commit failed: {commit_error}; rollback failed: {rollback_error}"
                                )));
                            }
                            return Err(database_error(commit_error));
                        }
                    }
                    Err(_) => {
                        self.db.execute_batch("ROLLBACK").map_err(database_error)?;
                    }
                }
            }
            result
        })();
        self.operation = previous;
        result
    }
}
