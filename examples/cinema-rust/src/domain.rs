//! Application-owned runtime contracts and voting rules.
use num_bigint::BigInt;
use serde_json::Value;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    Contract(String),
    Failure { code: String, payload: Value },
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Contract(reason) => write!(f, "{reason}"),
            Self::Failure { code, .. } => write!(f, "{code}"),
        }
    }
}
impl std::error::Error for Error {}
pub fn failure(code: &str, payload: Value) -> Error {
    match validate(code, &payload) {
        Ok(()) => Error::Failure {
            code: code.into(),
            payload,
        },
        Err(err) => err,
    }
}
#[derive(Clone)]
enum Contract {
    Text,
    Visible,
    Uuid,
    Integer,
    Float,
    Timestamp,
    Range(i64, i64),
    Alternatives(&'static [&'static str]),
    Optional(Box<Contract>),
    List(Box<Contract>),
    Record(Vec<(&'static str, Contract)>),
}
use Contract::*;
fn optional(c: Contract) -> Contract {
    Optional(Box::new(c))
}
fn list(c: Contract) -> Contract {
    List(Box::new(c))
}
fn record(fields: Vec<(&'static str, Contract)>) -> Contract {
    Record(fields)
}
fn reject(reason: &str) -> Error {
    Error::Contract(reason.into())
}
pub fn exact_integer(value: &Value) -> Result<BigInt, Error> {
    value
        .as_number()
        .and_then(|n| n.to_string().parse::<BigInt>().ok())
        .ok_or_else(|| reject("Expected exact integer"))
}
fn whitespace(c: char) -> bool {
    matches!(c,'\u{0009}'..='\u{000d}'|'\u{0020}'|'\u{00a0}'|'\u{1680}'|'\u{2000}'..='\u{200a}'|'\u{2028}'|'\u{2029}'|'\u{202f}'|'\u{205f}'|'\u{3000}'|'\u{feff}')
}
fn check(contract: &Contract, value: &Value) -> Result<(), Error> {
    let valid = match contract {
        Text => value.is_string(),
        Visible => value
            .as_str()
            .is_some_and(|s| !s.trim_matches(whitespace).is_empty()),
        Uuid => value.as_str().is_some_and(|s| {
            s.len() == 36
                && s.bytes().enumerate().all(|(i, b)| {
                    if [8, 13, 18, 23].contains(&i) {
                        b == b'-'
                    } else {
                        b.is_ascii_hexdigit()
                    }
                })
        }),
        Integer => exact_integer(value).is_ok(),
        Float => value.as_f64().is_some_and(f64::is_finite),
        Timestamp => value
            .as_str()
            .is_some_and(|s| chrono::DateTime::parse_from_rfc3339(s).is_ok()),
        Range(min, max) => {
            exact_integer(value).is_ok_and(|n| n >= BigInt::from(*min) && n <= BigInt::from(*max))
        }
        Alternatives(names) => value.as_str().is_some_and(|s| names.contains(&s)),
        Optional(inner) => value.is_null() || check(inner, value).is_ok(),
        List(inner) => value
            .as_array()
            .is_some_and(|items| items.iter().all(|v| check(inner, v).is_ok())),
        Record(fields) => value.as_object().is_some_and(|object| {
            object.len() == fields.len()
                && fields
                    .iter()
                    .all(|(name, c)| object.get(*name).is_some_and(|v| check(c, v).is_ok()))
        }),
    };
    if valid {
        Ok(())
    } else {
        Err(reject("Value violates application contract"))
    }
}
pub fn validate(name: &str, value: &Value) -> Result<(), Error> {
    let contract = contract(name).ok_or_else(|| reject("Unknown application contract"))?;
    check(&contract, value)
}
pub fn contract_names() -> Vec<&'static str> {
    NAMES.to_vec()
}
pub fn candidate_order_winner(votes: &[Value], candidates: &[Value]) -> Value {
    let mut best = 0;
    let mut winner = Value::Null;
    for candidate in candidates {
        let count = votes.iter().filter(|vote| *vote == candidate).count();
        if count > best {
            best = count;
            winner = candidate.clone();
        }
    }
    winner
}

fn film() -> Contract {
    record(vec![
        ("id", Uuid),
        ("title", Text),
        ("releaseYear", Range(1888, 2100)),
        (
            "genres",
            list(Alternatives(&[
                "Action",
                "Adventure",
                "Animation",
                "Comedy",
                "Crime",
                "Documentary",
                "Drama",
                "Fantasy",
                "Horror",
                "Mystery",
                "Romance",
                "ScienceFiction",
                "Thriller",
                "Western",
            ])),
        ),
        ("runtimeMinutes", Range(1, 600)),
        (
            "availability",
            Alternatives(&["Available", "Watched", "Wishlist", "Unavailable"]),
        ),
    ])
}
fn summary() -> Contract {
    let Record(mut fields) = film() else {
        unreachable!()
    };
    fields.extend([
        ("averageStars", Float),
        ("ratingCount", Integer),
        ("currentVoteCount", Integer),
    ]);
    record(fields)
}
const NAMES: &[&str] = &[
    "FilmId",
    "MemberId",
    "CommentId",
    "PollId",
    "ReleaseYear",
    "CommentText",
    "RuntimeMinutes",
    "PageOffset",
    "PageLimit",
    "Genre",
    "FilmAvailability",
    "FilmSort",
    "StarRating",
    "PollStatus",
    "Film",
    "SaveFilm",
    "FilmSummary",
    "FilmOverview",
    "FilmDetails",
    "FilmSearch",
    "ChangeAvailability",
    "AddComment",
    "Comment",
    "CommentView",
    "CommentOverview",
    "Rating",
    "RateFilm",
    "RatingSummary",
    "OpenVoting",
    "CastVote",
    "FilmVote",
    "ScreeningPoll",
    "FilmVoteResult",
    "VotingOverview",
    "FilmSaved",
    "FilmAvailabilityChanged",
    "CommentAdded",
    "FilmRated",
    "VotingOpened",
    "VoteCast",
    "VotingCompleted",
    "FilmNotFound",
    "CommentNotFound",
    "PollNotFound",
    "VotingClosed",
    "PersistenceUnavailable",
];
fn contract(name: &str) -> Option<Contract> {
    Some(match name {
        "FilmId" => Uuid,
        "MemberId" => Uuid,
        "CommentId" => Uuid,
        "PollId" => Uuid,
        "ReleaseYear" => Range(1888, 2100),
        "CommentText" => Visible,
        "RuntimeMinutes" => Range(1, 600),
        "PageOffset" => Range(0, 100000),
        "PageLimit" => Range(1, 100),
        "Genre" => Alternatives(&[
            "Action",
            "Adventure",
            "Animation",
            "Comedy",
            "Crime",
            "Documentary",
            "Drama",
            "Fantasy",
            "Horror",
            "Mystery",
            "Romance",
            "ScienceFiction",
            "Thriller",
            "Western",
        ]),
        "FilmAvailability" => Alternatives(&["Available", "Watched", "Wishlist", "Unavailable"]),
        "FilmSort" => Alternatives(&["TitleAscending", "NewestFirst", "HighestRated", "MostVoted"]),
        "StarRating" => Alternatives(&[
            "OneStar",
            "TwoStars",
            "ThreeStars",
            "FourStars",
            "FiveStars",
        ]),
        "PollStatus" => Alternatives(&["Open", "Closed"]),
        "Film" => film(),
        "SaveFilm" => film(),
        "FilmSummary" => summary(),
        "FilmOverview" => record(vec![
            ("items", list(summary())),
            ("total", Integer),
            ("offset", Integer),
            ("limit", Integer),
        ]),
        "FilmDetails" => record(vec![("film", summary())]),
        "FilmSearch" => record(vec![
            ("query", optional(Text)),
            (
                "genre",
                optional(Alternatives(&[
                    "Action",
                    "Adventure",
                    "Animation",
                    "Comedy",
                    "Crime",
                    "Documentary",
                    "Drama",
                    "Fantasy",
                    "Horror",
                    "Mystery",
                    "Romance",
                    "ScienceFiction",
                    "Thriller",
                    "Western",
                ])),
            ),
            (
                "minimumStars",
                optional(Alternatives(&[
                    "OneStar",
                    "TwoStars",
                    "ThreeStars",
                    "FourStars",
                    "FiveStars",
                ])),
            ),
            (
                "availability",
                optional(Alternatives(&[
                    "Available",
                    "Watched",
                    "Wishlist",
                    "Unavailable",
                ])),
            ),
            ("maximumRuntimeMinutes", optional(Range(1, 600))),
            ("releasedFrom", optional(Range(1888, 2100))),
            (
                "sort",
                Alternatives(&["TitleAscending", "NewestFirst", "HighestRated", "MostVoted"]),
            ),
            ("offset", Range(0, 100000)),
            ("limit", Range(1, 100)),
        ]),
        "ChangeAvailability" => record(vec![
            ("filmId", Uuid),
            (
                "availability",
                Alternatives(&["Available", "Watched", "Wishlist", "Unavailable"]),
            ),
        ]),
        "AddComment" => record(vec![
            ("filmId", Uuid),
            ("authorId", Uuid),
            ("text", Visible),
        ]),
        "Comment" => record(vec![
            ("id", Uuid),
            ("filmId", Uuid),
            ("authorId", Uuid),
            ("text", Visible),
            ("createdAt", Timestamp),
        ]),
        "CommentView" => record(vec![
            ("id", Uuid),
            ("filmId", Uuid),
            ("authorId", Uuid),
            ("text", Visible),
            ("createdAt", Timestamp),
        ]),
        "CommentOverview" => record(vec![
            (
                "items",
                list(record(vec![
                    ("id", Uuid),
                    ("filmId", Uuid),
                    ("authorId", Uuid),
                    ("text", Visible),
                    ("createdAt", Timestamp),
                ])),
            ),
            ("total", Integer),
        ]),
        "Rating" => record(vec![
            ("filmId", Uuid),
            ("memberId", Uuid),
            (
                "stars",
                Alternatives(&[
                    "OneStar",
                    "TwoStars",
                    "ThreeStars",
                    "FourStars",
                    "FiveStars",
                ]),
            ),
        ]),
        "RateFilm" => record(vec![
            ("filmId", Uuid),
            ("memberId", Uuid),
            (
                "stars",
                Alternatives(&[
                    "OneStar",
                    "TwoStars",
                    "ThreeStars",
                    "FourStars",
                    "FiveStars",
                ]),
            ),
        ]),
        "RatingSummary" => record(vec![
            ("filmId", Uuid),
            ("averageStars", Float),
            ("ratingCount", Integer),
            (
                "memberRating",
                optional(Alternatives(&[
                    "OneStar",
                    "TwoStars",
                    "ThreeStars",
                    "FourStars",
                    "FiveStars",
                ])),
            ),
        ]),
        "OpenVoting" => record(vec![
            ("pollId", Uuid),
            ("scheduledFor", Timestamp),
            ("candidateFilmIds", list(Uuid)),
        ]),
        "CastVote" => record(vec![("pollId", Uuid), ("memberId", Uuid), ("filmId", Uuid)]),
        "FilmVote" => record(vec![("pollId", Uuid), ("memberId", Uuid), ("filmId", Uuid)]),
        "ScreeningPoll" => record(vec![
            ("id", Uuid),
            ("scheduledFor", Timestamp),
            ("candidateFilmIds", list(Uuid)),
            ("status", Alternatives(&["Open", "Closed"])),
        ]),
        "FilmVoteResult" => record(vec![("filmId", Uuid), ("votes", Integer)]),
        "VotingOverview" => record(vec![
            ("pollId", Uuid),
            ("scheduledFor", Timestamp),
            ("status", Alternatives(&["Open", "Closed"])),
            (
                "results",
                list(record(vec![("filmId", Uuid), ("votes", Integer)])),
            ),
            ("winningFilmId", optional(Uuid)),
        ]),
        "FilmSaved" => record(vec![("filmId", Uuid)]),
        "FilmAvailabilityChanged" => record(vec![
            ("filmId", Uuid),
            (
                "availability",
                Alternatives(&["Available", "Watched", "Wishlist", "Unavailable"]),
            ),
        ]),
        "CommentAdded" => record(vec![("commentId", Uuid), ("filmId", Uuid)]),
        "FilmRated" => record(vec![
            ("filmId", Uuid),
            ("memberId", Uuid),
            (
                "stars",
                Alternatives(&[
                    "OneStar",
                    "TwoStars",
                    "ThreeStars",
                    "FourStars",
                    "FiveStars",
                ]),
            ),
        ]),
        "VotingOpened" => record(vec![("pollId", Uuid), ("scheduledFor", Timestamp)]),
        "VoteCast" => record(vec![("pollId", Uuid), ("memberId", Uuid), ("filmId", Uuid)]),
        "VotingCompleted" => record(vec![("pollId", Uuid), ("winningFilmId", optional(Uuid))]),
        "FilmNotFound" => record(vec![("filmId", Uuid)]),
        "CommentNotFound" => record(vec![("commentId", Uuid)]),
        "PollNotFound" => record(vec![("pollId", Uuid)]),
        "VotingClosed" => record(vec![("pollId", Uuid)]),
        "PersistenceUnavailable" => record(vec![("reason", Text)]),
        _ => return None,
    })
}
