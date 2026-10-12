// Package ginapi owns additional Gin routes; no internal operations are published implicitly.
package ginapi

import (
	"cinema/application"
	"cinema/domain"
	"encoding/json"
	"errors"
	"fmt"
	"github.com/gin-gonic/gin"
	"io"
	"math/big"
	"net/http"
	"os"
	"strings"
)

// Gin debug output must never share the testing protocol stdout. Configure once before sessions.
func init() {
	gin.SetMode(gin.ReleaseMode)
	gin.DefaultWriter = os.Stderr
	gin.DefaultErrorWriter = os.Stderr
}

func New(service *application.Service) http.Handler {
	router := gin.New()
	router.HandleMethodNotAllowed = true
	router.Use(func(c *gin.Context) { c.Header("X-Cinema-Framework", "gin"); c.Next() })
	router.NoRoute(func(c *gin.Context) { c.JSON(http.StatusNotFound, gin.H{"error": "Unknown HTTP route"}) })
	router.NoMethod(func(c *gin.Context) { c.JSON(http.StatusMethodNotAllowed, gin.H{"error": "Unsupported HTTP method"}) })
	type invocation func(*http.Request) ([]any, error)
	body := func(r *http.Request) (domain.Record, error) {
		data, err := io.ReadAll(io.LimitReader(r.Body, 1048577))
		if err != nil {
			return nil, err
		}
		if len(data) > 1048576 {
			return nil, fmt.Errorf("Body too large")
		}
		v, err := domain.DecodeJSON(data)
		if err != nil {
			return nil, err
		}
		record, ok := v.(domain.Record)
		if !ok {
			return nil, fmt.Errorf("Expected object")
		}
		return record, nil
	}
	register := func(pattern, operation string, arguments invocation) {
		parts := strings.SplitN(pattern, " ", 2)
		path := strings.ReplaceAll(parts[1], "{id}", ":id")
		router.Handle(parts[0], path, func(c *gin.Context) {
			w, r := c.Writer, c.Request
			r.SetPathValue("id", c.Param("id"))
			w.Header().Set("Content-Type", "application/json")
			args, err := arguments(r)
			var result any
			if err == nil {
				result, err = service.Invoke(operation, args)
			}
			if err != nil {
				var failure *domain.Failure
				if errors.As(err, &failure) {
					status := 404
					if failure.Code == "PersistenceUnavailable" {
						status = 503
					} else if failure.Code == "VotingClosed" {
						status = 409
					}
					w.WriteHeader(status)
					json.NewEncoder(w).Encode(domain.Record{"failure": failure.Code, "payload": failure.Payload})
				} else {
					w.WriteHeader(400)
					json.NewEncoder(w).Encode(domain.Record{"error": err.Error()})
				}
				return
			}
			json.NewEncoder(w).Encode(result)
		})
	}
	id := func(r *http.Request) ([]any, error) { return []any{r.PathValue("id")}, nil }
	request := func(r *http.Request) ([]any, error) { record, err := body(r); return []any{record}, err }
	withID := func(field string) invocation {
		return func(r *http.Request) ([]any, error) {
			record, err := body(r)
			if err != nil {
				return nil, err
			}
			record[field] = r.PathValue("id")
			return []any{record}, nil
		}
	}
	register("GET /films", "searchFilms", func(r *http.Request) ([]any, error) {
		q := r.URL.Query()
		filter := domain.Record{"query": nil, "genre": nil, "minimumStars": nil, "availability": nil, "maximumRuntimeMinutes": nil, "releasedFrom": nil, "sort": "TitleAscending", "offset": big.NewInt(0), "limit": big.NewInt(20)}
		for key := range filter {
			if q.Has(key) {
				switch key {
				case "maximumRuntimeMinutes", "releasedFrom", "offset", "limit":
					n, ok := new(big.Int).SetString(q.Get(key), 10)
					if !ok {
						return nil, fmt.Errorf("Invalid integer query")
					}
					filter[key] = n
				default:
					filter[key] = q.Get(key)
				}
			}
		}
		return []any{filter}, nil
	})
	register("GET /films/{id}", "getFilm", id)
	register("POST /films", "saveFilm", request)
	register("PATCH /films/{id}/availability", "changeFilmAvailability", withID("filmId"))
	register("GET /films/{id}/comments", "listComments", id)
	register("POST /films/{id}/comments", "addComment", withID("filmId"))
	register("GET /films/{id}/rating", "getFilmRating", func(r *http.Request) ([]any, error) {
		return []any{r.PathValue("id"), r.URL.Query().Get("memberId")}, nil
	})
	register("PUT /films/{id}/rating", "rateFilm", withID("filmId"))
	register("POST /voting", "openVoting", request)
	register("GET /voting/current", "getCurrentVoting", func(*http.Request) ([]any, error) { return []any{}, nil })
	register("PUT /voting/{id}/votes", "castVote", withID("pollId"))
	register("POST /voting/{id}/close", "closeVoting", id)
	return router
}
