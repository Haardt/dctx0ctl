package ginapi

import (
	"cinema/application"
	"cinema/persistence"
	"encoding/json"
	"fmt"
	"net/http"
	"net/http/httptest"
	"strings"
	"sync"
	"testing"
)

func TestActualGinRoutesValidateInputMapFailuresAndRunMiddleware(t *testing.T) {
	store, err := persistence.New(":memory:", persistence.Sources{}, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer store.Close()
	router := New(application.New(store, func(application.Event) error { return nil }))
	server := httptest.NewServer(router)
	defer server.Close()
	id := "00000000-0000-4000-8000-000000000001"
	call := func(method, path, body string, status int) map[string]any {
		t.Helper()
		request, err := http.NewRequest(method, server.URL+path, strings.NewReader(body))
		if err != nil {
			t.Fatal(err)
		}
		request.Header.Set("Content-Type", "application/json")
		response, err := server.Client().Do(request)
		if err != nil {
			t.Fatal(err)
		}
		defer response.Body.Close()
		if response.StatusCode != status {
			t.Fatalf("%s %s: got %d want %d", method, path, response.StatusCode, status)
		}
		if response.Header.Get("X-Cinema-Framework") != "gin" || !strings.HasPrefix(response.Header.Get("Content-Type"), "application/json") {
			t.Fatal("Actual middleware/content-type missing")
		}
		var value map[string]any
		if err = json.NewDecoder(response.Body).Decode(&value); err != nil {
			t.Fatal(err)
		}
		return value
	}
	call("GET", "/films/bad-id", "", 400)
	missing := call("GET", "/films/"+id, "", 404)
	if missing["failure"] != "FilmNotFound" {
		t.Fatal(missing)
	}
	call("POST", "/films", `{"id":"`+id+`","title":"Gin Movie","releaseYear":2026,"genres":[],"runtimeMinutes":90,"availability":"Available"}`, 200)
	found := call("GET", "/films/"+id, "", 200)
	if found["film"].(map[string]any)["title"] != "Gin Movie" {
		t.Fatal(found)
	}
	call("PATCH", "/films/"+id+"/availability", `{"availability":"Watched"}`, 200)
	call("GET", "/films?offset=bad", "", 400)
	call("POST", "/films", "{", 400)
	call("POST", "/films", strings.Repeat("x", 1048577), 400)
	call("GET", "/unknown", "", 404)
	call("DELETE", "/films/"+id, "", 405)
	if err = store.Close(); err != nil {
		t.Fatal(err)
	}
	call("GET", "/films/"+id, "", 503)
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
