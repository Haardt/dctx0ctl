// Explicit testing access to application-owned HTTP routes.
package main

import (
	"bytes"
	"cinema/domain"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
)

func (s *cinemaSession) invokeHTTP(name string, args []any) (any, error) {
	path := ""
	method := "GET"
	var body any
	var r domain.Record
	if len(args) > 0 {
		r, _ = args[0].(domain.Record)
	}
	id := func(v any) string { return url.PathEscape(v.(string)) }
	switch name {
	case "searchFilms":
		query := url.Values{}
		for k, v := range r {
			if v != nil {
				query.Set(k, fmt.Sprint(v))
			}
		}
		path = "/films?" + query.Encode()
	case "getFilm":
		path = "/films/" + id(args[0])
	case "saveFilm":
		path = "/films"
		method = "POST"
		body = r
	case "changeFilmAvailability":
		path = "/films/" + id(r["filmId"]) + "/availability"
		method = "PATCH"
		body = domain.Record{"availability": r["availability"]}
	case "listComments":
		path = "/films/" + id(args[0]) + "/comments"
	case "addComment":
		path = "/films/" + id(r["filmId"]) + "/comments"
		method = "POST"
		body = domain.Record{"authorId": r["authorId"], "text": r["text"]}
	case "getFilmRating":
		path = "/films/" + id(args[0]) + "/rating?memberId=" + url.QueryEscape(args[1].(string))
	case "rateFilm":
		path = "/films/" + id(r["filmId"]) + "/rating"
		method = "PUT"
		body = domain.Record{"memberId": r["memberId"], "stars": r["stars"]}
	case "openVoting":
		path = "/voting"
		method = "POST"
		body = r
	case "getCurrentVoting":
		path = "/voting/current"
	case "castVote":
		path = "/voting/" + id(r["pollId"]) + "/votes"
		method = "PUT"
		body = domain.Record{"memberId": r["memberId"], "filmId": r["filmId"]}
	case "closeVoting":
		path = "/voting/" + id(args[0]) + "/close"
		method = "POST"
	default:
		return nil, fmt.Errorf("No HTTP mapping")
	}
	data, err := json.Marshal(body)
	if err != nil {
		return nil, err
	}
	request, err := http.NewRequest(method, s.server.URL+path, bytes.NewReader(data))
	if err != nil {
		return nil, err
	}
	request.Header.Set("Content-Type", "application/json")
	response, err := s.server.Client().Do(request)
	if err != nil {
		return nil, err
	}
	defer response.Body.Close()
	data, err = io.ReadAll(io.LimitReader(response.Body, 1048577))
	if err != nil {
		return nil, err
	}
	if len(data) > 1048576 {
		return nil, fmt.Errorf("Response too large")
	}
	result, err := domain.DecodeJSON(data)
	if err != nil {
		return nil, err
	}
	if s.framework == "gin" && response.Header.Get("X-Cinema-Framework") != "gin" {
		return nil, fmt.Errorf("Gin middleware missing")
	}
	if response.StatusCode == 200 {
		return result, nil
	}
	record, ok := result.(domain.Record)
	if ok {
		code, ok := record["failure"].(string)
		if ok {
			status := 404
			if code == "PersistenceUnavailable" {
				status = 503
			} else if code == "VotingClosed" {
				status = 409
			}
			if response.StatusCode != status {
				return nil, fmt.Errorf("Wrong failure HTTP status")
			}
			return nil, domain.NewFailure(code, record["payload"].(domain.Record))
		}
	}
	return nil, fmt.Errorf("Unexpected HTTP status %d", response.StatusCode)
}
