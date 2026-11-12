package contracttests

import (
	"cinema/application"
	"cinema/domain"
	"cinema/httpapi"
	"cinema/persistence"
	"database/sql"
	"encoding/json"
	"fmt"
	app "github.com/Haardt/dctx0ctl/applicationtests/go"
	"net/http/httptest"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"
	"time"
)

type appInvocation = app.Invocation
type applicationConfiguration struct {
	Time      string   `json:"time"`
	IDs       []string `json:"ids"`
	HTTPCases []string `json:"httpCases"`
}
type cinemaFixture struct {
	context   *app.CaseContext
	store     *persistence.Store
	service   *application.Service
	server    *httptest.Server
	framework string
	calls     []app.InvocationObservation
	events    []app.Event
	resources []app.ResourceObservation
}

func hasSubject(subjects []string, subject string) bool {
	for _, candidate := range subjects {
		if candidate == subject {
			return true
		}
	}
	return false
}
func namedType(subject string) typeRef {
	return typeRef{Name: strings.TrimPrefix(subject, root), Symbol: &subject}
}
func (codec *valueCodec) actualObservation(observation app.Observation) (any, error) {
	if observation.Kind == "returned" && observation.Value != nil {
		return decode(*observation.Value)
	}
	if observation.Message != nil {
		return nil, fmt.Errorf("Production exception: %s", *observation.Message)
	}
	if observation.Reason != nil {
		return nil, fmt.Errorf("Collaborator unavailable: %s", *observation.Reason)
	}
	return nil, fmt.Errorf("Unsupported collaborator outcome")
}
func newCinemaFixture(t *testing.T, context *app.CaseContext, codec *valueCodec) (app.OperationFixture, error) {
	bytes, err := os.ReadFile(filepath.Join(codec.ApplicationRoot, ".dctx0", "testing", "go", "config.json"))
	if err != nil {
		return app.OperationFixture{}, err
	}
	var config applicationConfiguration
	if err = json.Unmarshal(bytes, &config); err != nil {
		return app.OperationFixture{}, err
	}
	fixture := &cinemaFixture{context: context}
	next := 0
	databasePath := filepath.Join(t.TempDir(), fmt.Sprintf("case-%d.sqlite3", context.Context))
	fixture.store, err = persistence.New(databasePath, persistence.Sources{Now: func() string { return config.Time }, ID: func() string {
		if next >= len(config.IDs) {
			return ""
		}
		id := config.IDs[next]
		next++
		return id
	}}, func(operation, entity, access string) error {
		operation = root + strings.TrimPrefix(operation, root)
		fixture.resources = append(fixture.resources, app.ResourceObservation{Operation: &operation, Resource: root + entity, Access: access})
		return nil
	})
	if err != nil {
		return app.OperationFixture{}, err
	}
	fixture.service = application.New(portsFunction(func(subject string, arguments []any) (any, error) {
		subject = root + strings.TrimPrefix(subject, root)
		operation, ok := codec.Operations[subject]
		if !ok || len(arguments) != len(operation.Parameters) {
			return nil, fmt.Errorf("Missing actual collaborator signature")
		}
		invocation := app.Invocation{Subject: subject, Arguments: make([]Value, len(arguments))}
		for index, argument := range arguments {
			wire, err := codec.encode(argument, operation.Parameters[index].Type)
			if err != nil {
				return nil, err
			}
			invocation.Arguments[index] = wire
		}
		var actual any
		var productionError error
		var evidence app.OperationResult
		if len(context.Mocks) > 0 {
			observation, err := context.Collaborate(invocation)
			if err != nil {
				return nil, err
			}
			evidence.Observation = observation
			actual, productionError = codec.actualObservation(observation)
		} else {
			actual, productionError = fixture.store.Invoke(subject, arguments)
			var err error
			evidence, err = codec.resultValue(actual, productionError, operation.Result)
			if err != nil {
				return nil, err
			}
		}
		fixture.calls = append(fixture.calls, app.InvocationObservation{Invocation: invocation, Observation: evidence.Observation})
		return actual, productionError
	}), func(event application.Event) error {
		subject := root + event.Name
		payload, err := codec.encode(event.Payload, namedType(subject))
		if err != nil {
			return err
		}
		fixture.events = append(fixture.events, app.Event{Subject: subject, Payload: payload})
		return nil
	})
	if hasSubject(config.HTTPCases, context.ID) {
		fixture.server = httptest.NewUnstartedServer(httpapi.New(fixture.service))
		fixture.server.Config.ReadHeaderTimeout = 5 * time.Second
		fixture.server.Config.ReadTimeout = 5 * time.Second
		fixture.server.Config.WriteTimeout = 5 * time.Second
		fixture.server.Start()
		fixture.server.Client().Timeout = 5 * time.Second
	}
	invoke := func(invocation app.Invocation) (app.OperationResult, error) {
		arguments, err := codec.decodeArguments(invocation)
		if err != nil {
			return app.OperationResult{}, err
		}
		fixture.calls = []app.InvocationObservation{}
		fixture.events = []app.Event{}
		fixture.resources = []app.ResourceObservation{}
		actual, productionError := fixture.production(invocation, arguments)
		evidence, err := codec.resultValue(actual, productionError, codec.Operations[invocation.Subject].Result)
		evidence.Calls = fixture.calls
		evidence.Events = fixture.events
		evidence.Resources = fixture.resources
		return evidence, err
	}
	subjects := make([]string, 0, len(codec.Operations))
	for subject := range codec.Operations {
		subjects = append(subjects, subject)
	}
	return app.OperationFixture{Subjects: subjects, Production: invoke, Setup: invoke, Oracle: invoke,
		ReadState: func(entities []string, _ string, _ app.Invocation) (map[string][]Value, bool, error) {
			connection, err := sql.Open("sqlite", "file:"+databasePath+"?mode=ro")
			if err != nil {
				return nil, false, err
			}
			defer connection.Close()
			physical := map[string]struct {
				query  string
				fields []string
			}{
				"Film":          {"SELECT id,document FROM films", []string{"id"}},
				"Comment":       {"SELECT id,film_id,document FROM comments", []string{"id", "filmId"}},
				"Rating":        {"SELECT film_id,member_id,document FROM ratings", []string{"filmId", "memberId"}},
				"ScreeningPoll": {"SELECT id,document FROM polls", []string{"id"}},
				"FilmVote":      {"SELECT poll_id,member_id,film_id,document FROM votes", []string{"pollId", "memberId", "filmId"}},
			}
			actual, err := fixture.store.Observe()
			if err != nil {
				return nil, false, err
			}
			rows := map[string][]Value{}
			for _, entity := range entities {
				records, ok := actual[strings.TrimPrefix(entity, root)]
				if !ok {
					return nil, false, fmt.Errorf("Missing actual entity %s", entity)
				}
				binding, ok := physical[strings.TrimPrefix(entity, root)]
				if !ok {
					return nil, false, fmt.Errorf("Missing physical entity binding")
				}
				physicalRows, err := connection.Query(binding.query)
				if err != nil {
					return nil, false, err
				}
				documents := map[string]int{}
				for physicalRows.Next() {
					keys := make([]string, len(binding.fields))
					var document []byte
					destinations := make([]any, len(keys)+1)
					for index := range keys {
						destinations[index] = &keys[index]
					}
					destinations[len(keys)] = &document
					if err := physicalRows.Scan(destinations...); err != nil {
						physicalRows.Close()
						return nil, false, err
					}
					decoded, err := domain.DecodeJSON(document)
					if err != nil {
						physicalRows.Close()
						return nil, false, err
					}
					record, ok := decoded.(domain.Record)
					if !ok {
						physicalRows.Close()
						return nil, false, fmt.Errorf("Persisted document is not a record")
					}
					for index, field := range binding.fields {
						if record[field] != keys[index] {
							physicalRows.Close()
							return nil, false, fmt.Errorf("Physical SQLite key differs from actual entity field %s", field)
						}
					}
					wire, err := codec.encode(record, namedType(entity))
					if err != nil {
						physicalRows.Close()
						return nil, false, err
					}
					bytes, _ := json.Marshal(wire)
					documents[string(bytes)]++
				}
				if err := physicalRows.Err(); err != nil {
					physicalRows.Close()
					return nil, false, err
				}
				physicalRows.Close()
				rows[entity] = []Value{}
				for _, record := range records {
					wire, err := codec.encode(record, namedType(entity))
					if err != nil {
						return nil, false, err
					}
					bytes, _ := json.Marshal(wire)
					if documents[string(bytes)] == 0 {
						return nil, false, fmt.Errorf("Entity observer differs from persisted rows")
					}
					documents[string(bytes)]--
					rows[entity] = append(rows[entity], wire)
				}
				for _, count := range documents {
					if count != 0 {
						return nil, false, fmt.Errorf("Entity observer omitted persisted rows")
					}
				}
			}
			return rows, true, nil
		}, Cleanup: func() error {
			if fixture.server != nil {
				fixture.server.Close()
			}
			before, err := fixture.store.Observe()
			if err != nil {
				fixture.store.Close()
				return err
			}
			if err := fixture.store.Close(); err != nil {
				return err
			}
			reopened, err := persistence.New(databasePath, persistence.Defaults(func() string { return "" }), nil)
			if err != nil {
				return err
			}
			defer reopened.Close()
			after, err := reopened.Observe()
			if err != nil {
				return err
			}
			if !reflect.DeepEqual(before, after) {
				return fmt.Errorf("Persisted state changed after close/reopen")
			}
			return nil
		}}, nil
}
func TestCinemaOperations(t *testing.T) {
	client := clientFor(t)
	client.OperationContextTest(t, func(context *app.CaseContext) (app.OperationFixture, error) {
		return newCinemaFixture(t, context, client.codec)
	})
}
func TestCinemaStateChanges(t *testing.T) {
	client := clientFor(t)
	client.StateChangeContextTest(t, func(context *app.CaseContext) (app.OperationFixture, error) {
		return newCinemaFixture(t, context, client.codec)
	})
}
func TestCinemaScenarios(t *testing.T) {
	client := clientFor(t)
	client.ScenarioTest(t, func(context *app.CaseContext) (app.OperationFixture, error) {
		return newCinemaFixture(t, context, client.codec)
	})
}
func TestCinemaLifecycles(t *testing.T) {
	client := clientFor(t)
	client.LifecycleTest(t, func(context *app.CaseContext) (app.OperationFixture, error) {
		return newCinemaFixture(t, context, client.codec)
	})
}
