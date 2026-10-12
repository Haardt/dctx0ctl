// External adapter: translations and observation access only; all Cinema behavior is production code.
package main

import (
	"cinema/application"
	"cinema/domain"
	"cinema/ginapi"
	"cinema/httpapi"
	"cinema/persistence"
	"encoding/json"
	"errors"
	"fmt"
	"net/http"
	"net/http/httptest"
	"os"
	"sort"
	"strings"
	"time"
)

var ports = map[string]bool{
	"Catalog.FilmCatalogStore.search": true, "Catalog.FilmCatalogStore.find": true, "Catalog.FilmCatalogStore.save": true, "Catalog.FilmCatalogStore.changeAvailability": true,
	"Community.FilmPresence.exists": true, "Community.CommentStore.listByFilm": true, "Community.CommentStore.append": true, "Community.RatingStore.summary": true, "Community.RatingStore.upsert": true,
	"ScreeningPlanning.VotingStore.open": true, "ScreeningPlanning.VotingStore.current": true, "ScreeningPlanning.VotingStore.cast": true, "ScreeningPlanning.VotingStore.close": true,
}
var surfaces = map[string]string{
	"Catalog.FilmCatalogApi.searchFilms": "searchFilms", "Catalog.FilmCatalogApi.getFilm": "getFilm", "Catalog.FilmCatalogApi.saveFilm": "saveFilm", "Catalog.FilmCatalogApi.changeFilmAvailability": "changeFilmAvailability",
	"Community.FilmCommunityApi.listComments": "listComments", "Community.FilmCommunityApi.addComment": "addComment", "Community.FilmCommunityApi.getFilmRating": "getFilmRating", "Community.FilmCommunityApi.rateFilm": "rateFilm",
	"ScreeningPlanning.ScreeningPlanningApi.openVoting": "openVoting", "ScreeningPlanning.ScreeningPlanningApi.getCurrentVoting": "getCurrentVoting", "ScreeningPlanning.ScreeningPlanningApi.castVote": "castVote", "ScreeningPlanning.ScreeningPlanningApi.closeVoting": "closeVoting",
}

type configuration struct {
	HTTPFramework string   `json:"httpFramework"`
	Mode          string   `json:"mode"`
	Database      string   `json:"database"`
	Time          string   `json:"time"`
	IDs           []string `json:"ids"`
	RealCases     []string `json:"realCases"`
	Generated     bool     `json:"realGeneratedOperations"`
	HTTPCases     []string `json:"httpCases"`
}

func includes(items []string, item string) bool {
	for _, v := range items {
		if v == item {
			return true
		}
	}
	return false
}

type cinemaAdapter struct{}

var adapter ProjectAdapter = cinemaAdapter{}

func (cinemaAdapter) Describe() (AdapterDescription, error) {
	if err := loadModel(); err != nil {
		return AdapterDescription{}, err
	}
	subjects := []SubjectDescription{}
	for _, name := range domain.ContractNames() {
		subjects = append(subjects, SubjectDescription{Subject: root + name, Role: "contract", Artifact: "domain/contracts.go", EntryPoint: "domain.Validate"})
	}
	for subject, name := range surfaces {
		subjects = append(subjects, SubjectDescription{Subject: root + subject, Role: "operation", Artifact: "application/service.go", EntryPoint: "Service.Invoke(" + name + ")"})
	}
	for subject := range ports {
		subjects = append(subjects, SubjectDescription{Subject: root + subject, Role: "port", Artifact: "persistence/store.go", EntryPoint: "Store.Invoke"})
	}
	subjects = append(subjects, SubjectDescription{Subject: root + "ScreeningPlanning.VotingRules.candidateOrderWinner", Role: "operation", Artifact: "domain/contracts.go", EntryPoint: "domain.CandidateOrderWinner"})
	sort.Slice(subjects, func(i, j int) bool { return subjects[i].Subject < subjects[j].Subject })
	return AdapterDescription{ProtocolVersion: 1, Subjects: subjects, Artifacts: []string{"domain/contracts.go", "domain/json.go", "application/service.go", "persistence/store.go", "httpapi/handler.go", "ginapi/handler.go", "cmd/cinema/main.go"}, Capabilities: []string{"sessions", "invocation", "events", "values", "call-observation", "mock-injection", "state", "real-ports", "resource-observation", "controlled-time", "controlled-ids"}, Representations: map[string]string{"Int": "math/big.Int; explicit checked bounded conversions", "Float": "finite float64", "UUID": "validated text", "Timestamp": "RFC3339 text", "Option": "nil or value", "variants": "closed alternative names"}}, nil
}

type cinemaSession struct {
	context   TestContext
	store     *persistence.Store
	service   *application.Service
	server    *httptest.Server
	real      bool
	framework string
}
type observedPorts struct{ session *cinemaSession }

func outcome(result any, err error, ty typeRef) (Observation, error) {
	if err != nil {
		var failure *domain.Failure
		if errors.As(err, &failure) {
			payload, e := encode(failure.Payload, named(failure.Code))
			if e != nil {
				return Observation{}, e
			}
			v := value("failure", map[string]any{"declaration": root + failure.Code, "payload": payload})
			return Observation{Kind: "returned", Value: &v}, nil
		}
		return Observation{Kind: "threw", Message: pointerTo(err.Error())}, nil
	}
	v, e := encode(result, ty)
	return Observation{Kind: "returned", Value: &v}, e
}
func (p observedPorts) Invoke(subject string, args []any) (any, error) {
	s := p.session
	op, ok := schema.Operations[root+subject]
	if !ok {
		return nil, fmt.Errorf("Missing port descriptor")
	}
	wire := make([]Value, len(args))
	for i, arg := range args {
		v, err := encode(arg, op.Parameters[i].Type)
		if err != nil {
			return nil, err
		}
		wire[i] = v
	}
	invocation := Invocation{Subject: root + subject, Arguments: wire}
	var result any
	var err error
	var observed Observation
	if s.real {
		result, err = s.store.Invoke(root+subject, args)
		observed, e := outcome(result, err, op.Result)
		if e != nil {
			return nil, e
		}
		if e = s.context.RecordCall(invocation, observed); e != nil {
			return nil, e
		}
		return result, err
	}
	observed, err = s.context.Collaborator(invocation)
	if err != nil {
		return nil, err
	}
	if err = s.context.RecordCall(invocation, observed); err != nil {
		return nil, err
	}
	if observed.Kind == "returned" && observed.Value != nil {
		return decode(*observed.Value)
	}
	if observed.Kind == "unavailable" && observed.Reason != nil {
		return nil, fmt.Errorf("%s", *observed.Reason)
	}
	if observed.Message != nil {
		return nil, fmt.Errorf("%s", *observed.Message)
	}
	return nil, fmt.Errorf("Unsupported collaborator observation")
}
func (cinemaAdapter) Begin(caseID string, context TestContext) (TestSession, error) {
	bytes, err := os.ReadFile("config.json")
	if err != nil {
		return nil, err
	}
	var config configuration
	if err = json.Unmarshal(bytes, &config); err != nil {
		return nil, err
	}
	if config.HTTPFramework != "" && config.HTTPFramework != "net-http" && config.HTTPFramework != "gin" {
		return nil, fmt.Errorf("httpFramework must be net-http or gin")
	}
	if config.Mode != "adaptive" && config.Mode != "real" && config.Mode != "mock" {
		return nil, fmt.Errorf("Unsupported mode")
	}
	s := &cinemaSession{context: context, framework: config.HTTPFramework}
	s.real = config.Mode == "real" || (config.Mode == "adaptive" && (includes(config.RealCases, caseID) || (config.Generated && strings.HasPrefix(caseID, root) && strings.Contains(caseID, ":operation:"))))
	if s.real {
		if config.Database == "" {
			return nil, fmt.Errorf("Explicit database required")
		}
		next := 0
		s.store, err = persistence.New(config.Database, persistence.Sources{Now: func() string { return config.Time }, ID: func() string {
			if next >= len(config.IDs) {
				return ""
			}
			id := config.IDs[next]
			next++
			return id
		}}, func(operation, entity, access string) error {
			return context.RecordResource(ResourceObservation{Operation: &operation, Resource: root + entity, Access: access})
		})
		if err != nil {
			return nil, err
		}
	}
	s.service = application.New(observedPorts{s}, func(event application.Event) error {
		v, err := encode(event.Payload, named(event.Name))
		if err != nil {
			return err
		}
		return context.Emit(root+event.Name, v)
	})
	httpCase := includes(config.HTTPCases, caseID)
	if config.HTTPFramework == "gin" && s.real {
		for subject := range surfaces {
			if strings.HasPrefix(caseID, root+subject+":operation:") {
				httpCase = true
			}
		}
	}
	if httpCase {
		if !s.real {
			return nil, fmt.Errorf("HTTP requires real session")
		}
		var handler http.Handler
		switch config.HTTPFramework {
		case "", "net-http":
			handler = httpapi.New(s.service)
		case "gin":
			handler = ginapi.New(s.service)
		default:
			return nil, fmt.Errorf("httpFramework must be net-http or gin")
		}
		s.server = httptest.NewUnstartedServer(handler)
		s.server.Config.ReadHeaderTimeout = 5 * time.Second
		s.server.Config.ReadTimeout = 5 * time.Second
		s.server.Config.WriteTimeout = 5 * time.Second
		s.server.Start()
		s.server.Client().Timeout = 5 * time.Second
	}
	return s, nil
}
func (s *cinemaSession) Validate(subject string, v Value) (ValueCheck, error) {
	name := strings.TrimPrefix(subject, root)
	if !includes(domain.ContractNames(), name) {
		return ValueCheck{Kind: "unavailable", Reason: pointerTo("No production validator")}, nil
	}
	input, err := decode(v)
	if err != nil {
		return ValueCheck{Kind: "unavailable", Reason: pointerTo(err.Error())}, nil
	}
	if err = domain.Validate(name, input); err != nil {
		return ValueCheck{Kind: "rejected", Reason: pointerTo(err.Error())}, nil
	}
	return ValueCheck{Kind: "accepted", Value: &v}, nil
}
func (s *cinemaSession) Invoke(invocation Invocation) (Observation, error) {
	op, ok := schema.Operations[invocation.Subject]
	if !ok {
		return Observation{}, fmt.Errorf("No operation descriptor")
	}
	args := make([]any, len(invocation.Arguments))
	for i, v := range invocation.Arguments {
		x, err := decode(v)
		if err != nil {
			return Observation{}, err
		}
		args[i] = x
	}
	subject := strings.TrimPrefix(invocation.Subject, root)
	var result any
	var err error
	if subject == "ScreeningPlanning.VotingRules.candidateOrderWinner" {
		result, err = CallApplication(func() (any, error) { return domain.CandidateOrderWinner(args[0].([]any), args[1].([]any)), nil })
	} else if ports[subject] && s.real {
		result, err = CallApplication(func() (any, error) { return s.store.Invoke(invocation.Subject, args) })
	} else if name, ok := surfaces[subject]; ok {
		if s.server != nil {
			result, err = s.invokeHTTP(name, args)
		} else {
			result, err = CallApplication(func() (any, error) { return s.service.Invoke(name, args) })
		}
	} else {
		return Observation{Kind: "unavailable", Reason: pointerTo("No actual entry point")}, nil
	}
	return outcome(result, err, op.Result)
}
func (s *cinemaSession) Observe(entities []string) (map[string][]Value, error) {
	if s.store == nil {
		return nil, fmt.Errorf("No actual state observer")
	}
	rows, err := s.store.Observe()
	if err != nil {
		return nil, err
	}
	out := map[string][]Value{}
	for _, entity := range entities {
		name := strings.TrimPrefix(entity, root)
		actual, ok := rows[name]
		if !ok {
			return nil, fmt.Errorf("Unknown entity")
		}
		values := make([]Value, len(actual))
		for i, row := range actual {
			v, err := encode(row, named(name))
			if err != nil {
				return nil, err
			}
			values[i] = v
		}
		out[entity] = values
	}
	return out, nil
}
func (s *cinemaSession) Close() error {
	if s.server != nil {
		s.server.Close()
	}
	if s.store != nil {
		return s.store.Close()
	}
	return nil
}
