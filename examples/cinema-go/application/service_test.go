package application

import (
	"cinema/domain"
	"errors"
	"math/big"
	"testing"
)

type fakePorts struct {
	calls  []string
	result any
	err    error
}

func (p *fakePorts) Invoke(operation string, args []any) (any, error) {
	p.calls = append(p.calls, operation)
	return p.result, p.err
}
func TestConsumerInvokesRealBoundaryAndEmitsOnlyAfterSuccess(t *testing.T) {
	p := &fakePorts{result: domain.Record{"film": domain.Record{}}}
	events := []Event{}
	s := New(p, func(e Event) error { events = append(events, e); return nil })
	id := "00000000-0000-4000-8000-000000000001"
	if _, err := s.Invoke("saveFilm", []any{domain.Record{"id": id, "title": "Cinema", "releaseYear": big.NewInt(2026), "genres": []any{}, "runtimeMinutes": big.NewInt(90), "availability": "Available"}}); err != nil {
		t.Fatal(err)
	}
	if len(events) != 1 || events[0].Name != "FilmSaved" || p.calls[0] != "Catalog.FilmCatalogStore.save" {
		t.Fatal("missing actual call/event")
	}
	p.err = domain.NewFailure("PersistenceUnavailable", domain.Record{"reason": "offline"})
	if _, err := s.Invoke("saveFilm", []any{domain.Record{"id": id, "title": "Cinema", "releaseYear": big.NewInt(2026), "genres": []any{}, "runtimeMinutes": big.NewInt(90), "availability": "Available"}}); err == nil || len(events) != 1 {
		t.Fatal("failed call emitted success")
	}
	p.err = nil
	p.result = nil
	if _, err := s.Invoke("getFilm", []any{id}); err == nil {
		t.Fatal("missing-film check")
	}
	_, err := s.Invoke("listComments", []any{id})
	var failure *domain.Failure
	if !errors.As(err, &failure) || failure.Code != "FilmNotFound" {
		t.Fatal(err)
	}
}
