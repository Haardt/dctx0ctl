// Package domain owns Cinema's actual runtime contracts, independently of its specification tooling.
package domain

import (
	"fmt"
	"math"
	"math/big"
	"regexp"
	"strings"
	"time"
)

type Record map[string]any

type Failure struct {
	Code    string
	Payload Record
}

func (f *Failure) Error() string { return f.Code }
func NewFailure(code string, payload Record) error {
	if err := Validate(code, payload); err != nil {
		return err
	}
	return &Failure{code, payload}
}

type validator func(any) error

type ContractError struct{ Message string }

func (e *ContractError) Error() string { return e.Message }
func reject(message string) error      { return &ContractError{Message: message} }
func text(v any) error {
	if _, ok := v.(string); !ok {
		return reject("Expected text")
	}
	return nil
}
func visible(v any) error {
	if err := text(v); err != nil {
		return err
	}
	if strings.TrimFunc(v.(string), contractWhitespace) == "" {
		return reject("Comment text must be visible")
	}
	return nil
}

// Contract whitespace follows the language's explicit trim semantics, including BOM
// and excluding NEXT LINE; Go's Unicode White_Space table differs at these boundaries.
func contractWhitespace(r rune) bool {
	return r >= '\u0009' && r <= '\u000d' || r == '\u0020' || r == '\u00a0' || r == '\u1680' || r >= '\u2000' && r <= '\u200a' || r == '\u2028' || r == '\u2029' || r == '\u202f' || r == '\u205f' || r == '\u3000' || r == '\ufeff'
}

var uuidPattern = regexp.MustCompile(`^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$`)

func uuid(v any) error {
	s, ok := v.(string)
	if !ok || !uuidPattern.MatchString(s) {
		return reject("Expected UUID")
	}
	return nil
}
func ExactInteger(v any) (*big.Int, error) {
	switch n := v.(type) {
	case *big.Int:
		if n != nil {
			return new(big.Int).Set(n), nil
		}
	case int:
		return big.NewInt(int64(n)), nil
	case int64:
		return big.NewInt(n), nil
	}
	return nil, reject("Expected exact integer")
}
func integer(v any) error { _, err := ExactInteger(v); return err }
func finite(v any) error {
	f, ok := v.(float64)
	if !ok || math.IsNaN(f) || math.IsInf(f, 0) {
		return reject("Expected finite float")
	}
	return nil
}
func bounded(min, max int64) validator {
	return func(v any) error {
		n, err := ExactInteger(v)
		if err != nil {
			return err
		}
		if n.Cmp(big.NewInt(min)) < 0 || n.Cmp(big.NewInt(max)) > 0 {
			return reject("Number outside range")
		}
		return nil
	}
}
func alternatives(names ...string) validator {
	return func(v any) error {
		s, ok := v.(string)
		if ok {
			for _, name := range names {
				if s == name {
					return nil
				}
			}
		}
		return reject("Unknown alternative")
	}
}
func optional(check validator) validator {
	return func(v any) error {
		if v == nil {
			return nil
		}
		return check(v)
	}
}
func list(check validator) validator {
	return func(v any) error {
		items, ok := v.([]any)
		if !ok {
			return reject("Expected list")
		}
		for _, item := range items {
			if err := check(item); err != nil {
				return err
			}
		}
		return nil
	}
}
func record(fields map[string]validator) validator {
	return func(v any) error {
		r, ok := v.(Record)
		if !ok || len(r) != len(fields) {
			return reject("Unexpected record fields")
		}
		for key, check := range fields {
			value, ok := r[key]
			if !ok {
				return reject("Missing record field")
			}
			if err := check(value); err != nil {
				return fmt.Errorf("%s: %w", key, err)
			}
		}
		return nil
	}
}
func timestamp(v any) error {
	s, ok := v.(string)
	if !ok {
		return reject("Expected timestamp")
	}
	if _, err := time.Parse(time.RFC3339Nano, s); err != nil {
		return reject(err.Error())
	}
	return nil
}

var genre = alternatives("Action", "Adventure", "Animation", "Comedy", "Crime", "Documentary", "Drama", "Fantasy", "Horror", "Mystery", "Romance", "ScienceFiction", "Thriller", "Western")
var availability = alternatives("Available", "Watched", "Wishlist", "Unavailable")
var stars = alternatives("OneStar", "TwoStars", "ThreeStars", "FourStars", "FiveStars")
var status = alternatives("Open", "Closed")
var filmSort = alternatives("TitleAscending", "NewestFirst", "HighestRated", "MostVoted")
var year = bounded(1888, 2100)
var runtime = bounded(1, 600)
var offset = bounded(0, 100000)
var limit = bounded(1, 100)
var filmFields = map[string]validator{"id": uuid, "title": text, "releaseYear": year, "genres": list(genre), "runtimeMinutes": runtime, "availability": availability}

func summaryFields() map[string]validator {
	r := map[string]validator{}
	for k, v := range filmFields {
		r[k] = v
	}
	r["averageStars"] = finite
	r["ratingCount"] = integer
	r["currentVoteCount"] = integer
	return r
}

var summary = record(summaryFields())
var comment = record(map[string]validator{"id": uuid, "filmId": uuid, "authorId": uuid, "text": visible, "createdAt": timestamp})
var rating = record(map[string]validator{"filmId": uuid, "memberId": uuid, "stars": stars})
var vote = record(map[string]validator{"pollId": uuid, "memberId": uuid, "filmId": uuid})
var contracts = map[string]validator{
	"FilmId": uuid, "MemberId": uuid, "CommentId": uuid, "PollId": uuid, "ReleaseYear": year, "CommentText": visible,
	"RuntimeMinutes": runtime, "PageOffset": offset, "PageLimit": limit, "Genre": genre, "FilmAvailability": availability, "FilmSort": filmSort, "StarRating": stars, "PollStatus": status,
	"Film": record(filmFields), "SaveFilm": record(filmFields), "FilmSummary": summary,
	"FilmOverview":       record(map[string]validator{"items": list(summary), "total": integer, "offset": integer, "limit": integer}),
	"FilmDetails":        record(map[string]validator{"film": summary}),
	"FilmSearch":         record(map[string]validator{"query": optional(text), "genre": optional(genre), "minimumStars": optional(stars), "availability": optional(availability), "maximumRuntimeMinutes": optional(runtime), "releasedFrom": optional(year), "sort": filmSort, "offset": offset, "limit": limit}),
	"ChangeAvailability": record(map[string]validator{"filmId": uuid, "availability": availability}),
	"AddComment":         record(map[string]validator{"filmId": uuid, "authorId": uuid, "text": visible}), "Comment": comment, "CommentView": comment,
	"CommentOverview": record(map[string]validator{"items": list(comment), "total": integer}), "Rating": rating, "RateFilm": rating,
	"RatingSummary": record(map[string]validator{"filmId": uuid, "averageStars": finite, "ratingCount": integer, "memberRating": optional(stars)}),
	"OpenVoting":    record(map[string]validator{"pollId": uuid, "scheduledFor": timestamp, "candidateFilmIds": list(uuid)}), "CastVote": vote, "FilmVote": vote,
	"ScreeningPoll":  record(map[string]validator{"id": uuid, "scheduledFor": timestamp, "candidateFilmIds": list(uuid), "status": status}),
	"FilmVoteResult": record(map[string]validator{"filmId": uuid, "votes": integer}),
	"VotingOverview": record(map[string]validator{"pollId": uuid, "scheduledFor": timestamp, "status": status, "results": list(record(map[string]validator{"filmId": uuid, "votes": integer})), "winningFilmId": optional(uuid)}),
	"FilmSaved":      record(map[string]validator{"filmId": uuid}), "FilmAvailabilityChanged": record(map[string]validator{"filmId": uuid, "availability": availability}),
	"CommentAdded": record(map[string]validator{"commentId": uuid, "filmId": uuid}), "FilmRated": rating,
	"VotingOpened": record(map[string]validator{"pollId": uuid, "scheduledFor": timestamp}), "VoteCast": vote,
	"VotingCompleted": record(map[string]validator{"pollId": uuid, "winningFilmId": optional(uuid)}),
	"FilmNotFound":    record(map[string]validator{"filmId": uuid}), "CommentNotFound": record(map[string]validator{"commentId": uuid}),
	"PollNotFound": record(map[string]validator{"pollId": uuid}), "VotingClosed": record(map[string]validator{"pollId": uuid}), "PersistenceUnavailable": record(map[string]validator{"reason": text}),
}

func ContractNames() []string {
	out := make([]string, 0, len(contracts))
	for name := range contracts {
		out = append(out, name)
	}
	return out
}
func Validate(name string, value any) error {
	check, ok := contracts[name]
	if !ok {
		return reject("Unknown contract: " + name)
	}
	return check(value)
}
func CandidateOrderWinner(votes, candidates []any) any {
	if len(candidates) == 0 {
		return nil
	}
	var winner any
	best := 0
	for _, candidate := range candidates {
		count := 0
		for _, vote := range votes {
			if vote == candidate {
				count++
			}
		}
		if count > best {
			winner = candidate
			best = count
		}
	}
	return winner
}
