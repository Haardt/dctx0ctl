package httpapi

import (
	"cinema/application"
	"cinema/domain"
	"cinema/persistence"
	"encoding/json"
	"fmt"
	"net/http"
	"net/http/httptest"
	"strings"
	"sync"
	"testing"
)

func TestRealHTTPRoutingAndErrorStatuses(t *testing.T) {
	store, err := persistence.New(":memory:", persistence.Sources{}, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer store.Close()
	server := httptest.NewServer(New(application.New(store, func(application.Event) error { return nil })))
	defer server.Close()
	id := "00000000-0000-4000-8000-000000000001"
	request := func(method, path, body string, status int) domain.Record {
		t.Helper()
		r, err := http.NewRequest(method, server.URL+path, strings.NewReader(body))
		if err != nil {
			t.Fatal(err)
		}
		response, err := server.Client().Do(r)
		if err != nil {
			t.Fatal(err)
		}
		defer response.Body.Close()
		if response.StatusCode != status {
			t.Fatalf("%s %s: %d", method, path, response.StatusCode)
		}
		var v map[string]any
		if err = json.NewDecoder(response.Body).Decode(&v); err != nil {
			t.Fatal(err)
		}
		return domain.Record(v)
	}
	request("GET", "/films/bad-id", "", 400)
	missing := request("GET", "/films/"+id, "", 404)
	if missing["failure"] != "FilmNotFound" {
		t.Fatal(missing)
	}
	film := `{"id":"` + id + `","title":"Movie","releaseYear":2026,"genres":[],"runtimeMinutes":90,"availability":"Available"}`
	request("POST", "/films", film, 200)
	request("GET", "/films/"+id, "", 200)
	request("PATCH", "/films/"+id+"/availability", `{"availability":"Watched"}`, 200)
	request("GET", "/films?offset=bad", "", 400)
	request("POST", "/films", `{`, 400)
	if err := store.Close(); err != nil {
		t.Fatal(err)
	}
	request("GET", "/films/"+id, "", 503)
}

func TestConcurrentRequestsRetainDistinctFilmsWithoutTransactionRaces(t *testing.T) {
	store, err := persistence.New(":memory:", persistence.Sources{}, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer store.Close()
	server := httptest.NewServer(New(application.New(store, func(application.Event) error { return nil })))
	defer server.Close()
	var group sync.WaitGroup
	for i := 1; i <= 20; i++ {
		group.Add(1)
		go func(index int) {
			defer group.Done()
			body := fmt.Sprintf(`{"id":"00000000-0000-4000-8000-%012d","title":"Concurrent","releaseYear":2026,"genres":[],"runtimeMinutes":90,"availability":"Available"}`, index)
			response, err := server.Client().Post(server.URL+"/films", "application/json", strings.NewReader(body))
			if err != nil {
				t.Error(err)
				return
			}
			defer response.Body.Close()
			if response.StatusCode != 200 {
				t.Errorf("save status %d", response.StatusCode)
			}
		}(i)
	}
	group.Wait()
	state, err := store.Observe()
	if err != nil || len(state["Film"]) != 20 {
		t.Fatalf("films lost: %v %d", err, len(state["Film"]))
	}
}
