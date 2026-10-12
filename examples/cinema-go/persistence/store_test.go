package persistence

import (
	"cinema/domain"
	"errors"
	"math/big"
	"path/filepath"
	"testing"
)

func TestActualPersistenceVotingReplacementAndTransactions(t *testing.T) {
	path := filepath.Join(t.TempDir(), "cinema.sqlite3")
	s, err := New(path, Sources{Now: func() string { return "2026-10-10T18:00:00Z" }, ID: func() string { return "00000000-0000-4000-8000-000000000030" }}, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer s.Close()
	film := "00000000-0000-4000-8000-000000000001"
	member := "00000000-0000-4000-8000-000000000002"
	poll := "00000000-0000-4000-8000-000000000003"
	request := domain.Record{"id": film, "title": "Cinema", "releaseYear": big.NewInt(2026), "genres": []any{}, "runtimeMinutes": big.NewInt(90), "availability": "Available"}
	call := func(op string, args ...any) any {
		t.Helper()
		v, err := s.Invoke(op, args)
		if err != nil {
			t.Fatal(err)
		}
		return v
	}
	call("Catalog.FilmCatalogStore.save", request)
	request["title"] = "Updated"
	call("Catalog.FilmCatalogStore.save", request)
	state, err := s.Observe()
	if err != nil || len(state["Film"]) != 1 {
		t.Fatal("upsert did not preserve key", err)
	}
	otherFilm := "00000000-0000-4000-8000-000000000004"
	second := copyRecord(request)
	second["id"] = otherFilm
	second["title"] = "Second"
	call("Catalog.FilmCatalogStore.save", second)
	call("ScreeningPlanning.VotingStore.open", domain.Record{"pollId": poll, "scheduledFor": "2026-10-10T18:00:00Z", "candidateFilmIds": []any{film, otherFilm}})
	vote := domain.Record{"pollId": poll, "memberId": member, "filmId": film}
	call("ScreeningPlanning.VotingStore.cast", vote)
	vote["filmId"] = otherFilm
	call("ScreeningPlanning.VotingStore.cast", vote)
	state, err = s.Observe()
	if err != nil || len(state["FilmVote"]) != 1 {
		t.Fatal("replaceable vote", err)
	}
	closed := call("ScreeningPlanning.VotingStore.close", poll).(domain.Record)
	if closed["winningFilmId"] != otherFilm {
		t.Fatal("winner")
	}
	_, err = s.Invoke("ScreeningPlanning.VotingStore.cast", []any{vote})
	var failure *domain.Failure
	if !errors.As(err, &failure) || failure.Code != "VotingClosed" {
		t.Fatal(err)
	}
	other, err := New(path, Sources{}, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer other.Close()
	state, err = other.Observe()
	if err != nil || len(state["Film"]) != 2 || state["Film"][0].(domain.Record)["title"] != "Updated" {
		t.Fatal("reopen persistence", err)
	}
}
