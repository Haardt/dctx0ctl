// Package persistence implements Cinema ports with real SQLite statements and transactions.
package persistence

import (
	"cinema/domain"
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"math/big"
	_ "modernc.org/sqlite"
	"sort"
	"strings"
	"sync"
	"time"
)

type Sources struct {
	Now func() string
	ID  func() string
}
type QueryObserver func(operation, entity, access string) error
type Store struct {
	mutex     sync.Mutex
	db        *sql.DB
	sources   Sources
	observer  QueryObserver
	operation string
	tx        *sql.Tx
}

var entities = map[string]string{"films": "Film", "comments": "Comment", "ratings": "Rating", "polls": "ScreeningPoll", "votes": "FilmVote"}

func New(database string, sources Sources, observer QueryObserver) (*Store, error) {
	db, err := sql.Open("sqlite", database)
	if err != nil {
		return nil, err
	}
	db.SetMaxOpenConns(1)
	s := &Store{db: db, sources: sources, observer: observer}
	_, err = db.Exec(`PRAGMA foreign_keys=ON;
 CREATE TABLE IF NOT EXISTS films(id TEXT PRIMARY KEY,document TEXT NOT NULL);
 CREATE TABLE IF NOT EXISTS comments(id TEXT PRIMARY KEY,film_id TEXT NOT NULL REFERENCES films(id),document TEXT NOT NULL);
 CREATE TABLE IF NOT EXISTS ratings(film_id TEXT NOT NULL REFERENCES films(id),member_id TEXT NOT NULL,document TEXT NOT NULL,PRIMARY KEY(film_id,member_id));
 CREATE TABLE IF NOT EXISTS polls(id TEXT PRIMARY KEY,document TEXT NOT NULL);
 CREATE TABLE IF NOT EXISTS votes(poll_id TEXT NOT NULL REFERENCES polls(id),member_id TEXT NOT NULL,film_id TEXT NOT NULL REFERENCES films(id),document TEXT NOT NULL,PRIMARY KEY(poll_id,member_id));`)
	if err != nil {
		db.Close()
		return nil, err
	}
	return s, nil
}
func (s *Store) Close() error { s.mutex.Lock(); defer s.mutex.Unlock(); return s.db.Close() }
func (s *Store) access(table, access string) error {
	if s.observer != nil && s.operation != "" {
		return s.observer(s.operation, entities[table], access)
	}
	return nil
}
func (s *Store) exec(table, statement string, args ...any) error {
	var err error
	if s.tx != nil {
		_, err = s.tx.Exec(statement, args...)
	} else {
		_, err = s.db.Exec(statement, args...)
	}
	observed := s.access(table, "write")
	if err != nil {
		return domain.NewFailure("PersistenceUnavailable", domain.Record{"reason": err.Error()})
	}
	return observed
}
func (s *Store) rows(table string) ([]any, error) {
	var rows *sql.Rows
	var err error
	query := "SELECT document FROM " + table + " ORDER BY rowid"
	if s.tx != nil {
		rows, err = s.tx.Query(query)
	} else {
		rows, err = s.db.Query(query)
	}
	observed := s.access(table, "read")
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	if observed != nil {
		return nil, observed
	}
	out := []any{}
	for rows.Next() {
		var document []byte
		if err := rows.Scan(&document); err != nil {
			return nil, err
		}
		value, err := domain.DecodeJSON(document)
		if err != nil {
			return nil, err
		}
		out = append(out, value)
	}
	return out, rows.Err()
}
func (s *Store) put(table string, record domain.Record) error {
	data, err := json.Marshal(record)
	if err != nil {
		return err
	}
	switch table {
	case "films", "polls":
		return s.exec(table, "INSERT INTO "+table+" VALUES(?,?) ON CONFLICT(id) DO UPDATE SET document=excluded.document", record["id"], string(data))
	case "comments":
		return s.exec(table, "INSERT INTO comments VALUES(?,?,?)", record["id"], record["filmId"], string(data))
	case "ratings":
		return s.exec(table, "INSERT INTO ratings VALUES(?,?,?) ON CONFLICT(film_id,member_id) DO UPDATE SET document=excluded.document", record["filmId"], record["memberId"], string(data))
	case "votes":
		return s.exec(table, "INSERT INTO votes VALUES(?,?,?,?) ON CONFLICT(poll_id,member_id) DO UPDATE SET film_id=excluded.film_id,document=excluded.document", record["pollId"], record["memberId"], record["filmId"], string(data))
	}
	return fmt.Errorf("Unknown table")
}
func (s *Store) find(table string, id any) (domain.Record, error) {
	rows, err := s.rows(table)
	if err != nil {
		return nil, err
	}
	for _, row := range rows {
		record := row.(domain.Record)
		if record["id"] == id {
			return record, nil
		}
	}
	return nil, nil
}
func (s *Store) requireFilm(id any) (domain.Record, error) {
	r, err := s.find("films", id)
	if err != nil {
		return nil, err
	}
	if r == nil {
		return nil, domain.NewFailure("FilmNotFound", domain.Record{"filmId": id})
	}
	return r, nil
}
func (s *Store) requirePoll(id any) (domain.Record, error) {
	r, err := s.find("polls", id)
	if err != nil {
		return nil, err
	}
	if r == nil {
		return nil, domain.NewFailure("PollNotFound", domain.Record{"pollId": id})
	}
	if r["status"] != "Open" {
		return nil, domain.NewFailure("VotingClosed", domain.Record{"pollId": id})
	}
	return r, nil
}

var starNames = []string{"OneStar", "TwoStars", "ThreeStars", "FourStars", "FiveStars"}

func star(value any) int {
	for i, name := range starNames {
		if value == name {
			return i + 1
		}
	}
	return 0
}
func number(value any) int64 {
	n, _ := domain.ExactInteger(value)
	if n == nil {
		return 0
	}
	return n.Int64()
}
func copyRecord(r domain.Record) domain.Record {
	out := domain.Record{}
	for k, v := range r {
		out[k] = v
	}
	return out
}
func (s *Store) ratingSummary(film, member any) (domain.Record, error) {
	ratings, err := s.rows("ratings")
	if err != nil {
		return nil, err
	}
	count, total := 0, 0
	var current any
	for _, row := range ratings {
		r := row.(domain.Record)
		if r["filmId"] == film {
			count++
			total += star(r["stars"])
			if r["memberId"] == member {
				current = r["stars"]
			}
		}
	}
	average := 0.0
	if count > 0 {
		average = float64(total) / float64(count)
	}
	return domain.Record{"filmId": film, "averageStars": average, "ratingCount": big.NewInt(int64(count)), "memberRating": current}, nil
}
func (s *Store) summary(film domain.Record) (domain.Record, error) {
	r, err := s.ratingSummary(film["id"], nil)
	if err != nil {
		return nil, err
	}
	out := copyRecord(film)
	out["averageStars"] = r["averageStars"]
	out["ratingCount"] = r["ratingCount"]
	polls, err := s.rows("polls")
	if err != nil {
		return nil, err
	}
	var current domain.Record
	for _, row := range polls {
		poll := row.(domain.Record)
		if poll["status"] == "Open" {
			current = poll
		}
	}
	count := int64(0)
	if current != nil {
		votes, err := s.rows("votes")
		if err != nil {
			return nil, err
		}
		for _, row := range votes {
			v := row.(domain.Record)
			if v["pollId"] == current["id"] && v["filmId"] == film["id"] {
				count++
			}
		}
	}
	out["currentVoteCount"] = big.NewInt(count)
	return out, nil
}
func (s *Store) details(film domain.Record) (any, error) {
	summary, err := s.summary(film)
	if err != nil {
		return nil, err
	}
	return domain.Record{"film": summary}, nil
}
func (s *Store) overview(poll domain.Record) (any, error) {
	rows, err := s.rows("votes")
	if err != nil {
		return nil, err
	}
	votes := []any{}
	for _, row := range rows {
		v := row.(domain.Record)
		if v["pollId"] == poll["id"] {
			votes = append(votes, v["filmId"])
		}
	}
	results := []any{}
	candidates := poll["candidateFilmIds"].([]any)
	for _, id := range candidates {
		count := int64(0)
		for _, vote := range votes {
			if vote == id {
				count++
			}
		}
		results = append(results, domain.Record{"filmId": id, "votes": big.NewInt(count)})
	}
	return domain.Record{"pollId": poll["id"], "scheduledFor": poll["scheduledFor"], "status": poll["status"], "results": results, "winningFilmId": domain.CandidateOrderWinner(votes, candidates)}, nil
}
func (s *Store) search(filter domain.Record) (any, error) {
	rows, err := s.rows("films")
	if err != nil {
		return nil, err
	}
	items := []any{}
	for _, row := range rows {
		r, err := s.summary(row.(domain.Record))
		if err != nil {
			return nil, err
		}
		if q := filter["query"]; q != nil && !strings.Contains(strings.ToLower(r["title"].(string)), strings.ToLower(q.(string))) {
			continue
		}
		if g := filter["genre"]; g != nil {
			found := false
			for _, genre := range r["genres"].([]any) {
				if genre == g {
					found = true
				}
			}
			if !found {
				continue
			}
		}
		if a := filter["availability"]; a != nil && a != r["availability"] {
			continue
		}
		if m := filter["maximumRuntimeMinutes"]; m != nil && number(r["runtimeMinutes"]) > number(m) {
			continue
		}
		if y := filter["releasedFrom"]; y != nil && number(r["releaseYear"]) < number(y) {
			continue
		}
		if stars := filter["minimumStars"]; stars != nil && r["averageStars"].(float64) < float64(star(stars)) {
			continue
		}
		items = append(items, r)
	}
	sort.SliceStable(items, func(i, j int) bool {
		a, b := items[i].(domain.Record), items[j].(domain.Record)
		var comparison int
		switch filter["sort"] {
		case "NewestFirst":
			comparison = compare(number(b["releaseYear"]), number(a["releaseYear"]))
		case "HighestRated":
			if a["averageStars"] != b["averageStars"] {
				return a["averageStars"].(float64) > b["averageStars"].(float64)
			}
		case "MostVoted":
			comparison = compare(number(b["currentVoteCount"]), number(a["currentVoteCount"]))
		default:
			comparison = strings.Compare(a["title"].(string), b["title"].(string))
		}
		if comparison == 0 {
			return a["id"].(string) < b["id"].(string)
		}
		return comparison < 0
	})
	total := len(items)
	start := int(number(filter["offset"]))
	if start > total {
		start = total
	}
	end := start + int(number(filter["limit"]))
	if end > total {
		end = total
	}
	return domain.Record{"items": items[start:end], "total": big.NewInt(int64(total)), "offset": filter["offset"], "limit": filter["limit"]}, nil
}
func compare(a, b int64) int {
	if a < b {
		return -1
	}
	if a > b {
		return 1
	}
	return 0
}
func (s *Store) Invoke(operation string, args []any) (result any, err error) {
	s.mutex.Lock()
	defer s.mutex.Unlock()
	defer func() {
		if err != nil {
			var failure *domain.Failure
			var invalid *domain.ContractError
			if !errors.As(err, &failure) && !errors.As(err, &invalid) {
				err = domain.NewFailure("PersistenceUnavailable", domain.Record{"reason": err.Error()})
			}
		}
	}()
	previous := s.operation
	s.operation = operation
	defer func() { s.operation = previous }()
	if strings.HasPrefix(operation, "HomeCinema.MovieNight.") {
		operation = strings.TrimPrefix(operation, "HomeCinema.MovieNight.")
	}
	var request domain.Record
	if len(args) > 0 {
		request, _ = args[0].(domain.Record)
	}
	input := map[string]string{"Catalog.FilmCatalogStore.search": "FilmSearch", "Catalog.FilmCatalogStore.save": "SaveFilm", "Catalog.FilmCatalogStore.changeAvailability": "ChangeAvailability", "Community.CommentStore.append": "AddComment", "Community.RatingStore.upsert": "RateFilm", "ScreeningPlanning.VotingStore.open": "OpenVoting", "ScreeningPlanning.VotingStore.cast": "CastVote"}
	if contract := input[operation]; contract != "" {
		if err = domain.Validate(contract, request); err != nil {
			return nil, err
		}
	}
	scalarInputs := map[string][]string{
		"Catalog.FilmCatalogStore.find": {"FilmId"}, "Community.FilmPresence.exists": {"FilmId"},
		"Community.CommentStore.listByFilm": {"FilmId"}, "Community.RatingStore.summary": {"FilmId", "MemberId"},
		"ScreeningPlanning.VotingStore.close": {"PollId"},
	}
	if names, ok := scalarInputs[operation]; ok {
		if len(args) != len(names) {
			return nil, &domain.ContractError{Message: "Incorrect port arity"}
		}
		for i, name := range names {
			if err = domain.Validate(name, args[i]); err != nil {
				return nil, err
			}
		}
	}
	transactional := strings.HasPrefix(operation, "ScreeningPlanning.VotingStore.") && (strings.HasSuffix(operation, ".open") || strings.HasSuffix(operation, ".cast") || strings.HasSuffix(operation, ".close"))
	if transactional {
		s.tx, err = s.db.Begin()
		if err != nil {
			return nil, err
		}
		defer func() {
			if err != nil {
				s.tx.Rollback()
			} else {
				err = s.tx.Commit()
			}
			s.tx = nil
		}()
	}
	switch operation {
	case "Catalog.FilmCatalogStore.search":
		return s.search(request)
	case "Catalog.FilmCatalogStore.find":
		film, err := s.find("films", args[0])
		if err != nil || film == nil {
			return nil, err
		}
		return s.details(film)
	case "Catalog.FilmCatalogStore.save":
		if err = s.put("films", request); err != nil {
			return nil, err
		}
		return s.details(request)
	case "Catalog.FilmCatalogStore.changeAvailability":
		film, err := s.requireFilm(request["filmId"])
		if err != nil {
			return nil, err
		}
		film["availability"] = request["availability"]
		if err = s.put("films", film); err != nil {
			return nil, err
		}
		return s.details(film)
	case "Community.FilmPresence.exists":
		film, err := s.find("films", args[0])
		if err != nil || film == nil {
			return nil, err
		}
		return film["id"], nil
	case "Community.CommentStore.listByFilm":
		rows, err := s.rows("comments")
		if err != nil {
			return nil, err
		}
		items := []any{}
		for _, row := range rows {
			if row.(domain.Record)["filmId"] == args[0] {
				items = append(items, row)
			}
		}
		return domain.Record{"items": items, "total": big.NewInt(int64(len(items)))}, nil
	case "Community.CommentStore.append":
		comment := copyRecord(request)
		if s.sources.ID == nil || s.sources.Now == nil {
			return nil, fmt.Errorf("Missing ID/time source")
		}
		comment["id"] = s.sources.ID()
		comment["createdAt"] = s.sources.Now()
		if err = domain.Validate("Comment", comment); err != nil {
			return nil, err
		}
		if err = s.put("comments", comment); err != nil {
			return nil, err
		}
		return comment, nil
	case "Community.RatingStore.summary":
		return s.ratingSummary(args[0], args[1])
	case "Community.RatingStore.upsert":
		if err = s.put("ratings", request); err != nil {
			return nil, err
		}
		return s.ratingSummary(request["filmId"], request["memberId"])
	case "ScreeningPlanning.VotingStore.open":
		poll := domain.Record{"id": request["pollId"], "scheduledFor": request["scheduledFor"], "candidateFilmIds": request["candidateFilmIds"], "status": "Open"}
		if err = s.exec("votes", "DELETE FROM votes WHERE poll_id=?", poll["id"]); err != nil {
			return nil, err
		}
		if err = s.put("polls", poll); err != nil {
			return nil, err
		}
		return s.overview(poll)
	case "ScreeningPlanning.VotingStore.current":
		rows, err := s.rows("polls")
		if err != nil || len(rows) == 0 {
			return nil, err
		}
		poll := rows[len(rows)-1].(domain.Record)
		for _, row := range rows {
			if row.(domain.Record)["status"] == "Open" {
				poll = row.(domain.Record)
			}
		}
		return s.overview(poll)
	case "ScreeningPlanning.VotingStore.cast":
		poll, err := s.requirePoll(request["pollId"])
		if err != nil {
			return nil, err
		}
		if _, err = s.requireFilm(request["filmId"]); err != nil {
			return nil, err
		}
		found := false
		for _, candidate := range poll["candidateFilmIds"].([]any) {
			if candidate == request["filmId"] {
				found = true
			}
		}
		if !found {
			return nil, domain.NewFailure("FilmNotFound", domain.Record{"filmId": request["filmId"]})
		}
		if err = s.put("votes", request); err != nil {
			return nil, err
		}
		return s.overview(poll)
	case "ScreeningPlanning.VotingStore.close":
		poll, err := s.requirePoll(args[0])
		if err != nil {
			return nil, err
		}
		poll["status"] = "Closed"
		if err = s.put("polls", poll); err != nil {
			return nil, err
		}
		return s.overview(poll)
	}
	return nil, fmt.Errorf("Unknown port %s", operation)
}
func (s *Store) Observe() (map[string][]any, error) {
	s.mutex.Lock()
	defer s.mutex.Unlock()
	out := map[string][]any{}
	for table, entity := range entities {
		rows, err := s.rows(table)
		if err != nil {
			return nil, err
		}
		out[entity] = rows
	}
	return out, nil
}

// Defaults supplies production time; callers own the ID factory.
func Defaults(id func() string) Sources {
	return Sources{Now: func() string { return time.Now().UTC().Format(time.RFC3339Nano) }, ID: id}
}
