package contracttests

import (
	"cinema/application"
	"cinema/domain"
	"cinema/persistence"
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	app "github.com/Haardt/dctx0ctl/applicationtests/go"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

type operationDescriptor struct {
	Parameters []field `json:"parameters"`
	Result     typeRef `json:"result"`
}

type applicationClient struct {
	*app.Client
	codec *valueCodec
}

type portsFunction func(string, []any) (any, error)

func (p portsFunction) Invoke(subject string, args []any) (any, error) { return p(subject, args) }

func clientFor(t *testing.T) *applicationClient {
	t.Helper()
	endpoint := os.Getenv("DCTX0_APPLICATION_SOCKET")
	if endpoint == "" {
		t.Skip("application assessment service not selected")
	}
	bytes, err := os.ReadFile(os.Getenv("DCTX0_APPLICATION_PLAN"))
	if err != nil {
		t.Fatal(err)
	}
	var bundle struct {
		Model struct {
			Data       map[string]declaration         `json:"data"`
			Operations map[string]operationDescriptor `json:"operations"`
		} `json:"model"`
	}
	if err := json.Unmarshal(bytes, &bundle); err != nil {
		t.Fatal(err)
	}
	applicationRoot := os.Getenv("DCTX0_APPLICATION_ROOT")
	if applicationRoot == "" {
		applicationRoot = filepath.Clean(filepath.Join(filepath.Dir(os.Getenv("DCTX0_APPLICATION_PLAN")), "..", ".."))
	}
	codec := &valueCodec{Data: bundle.Model.Data, Operations: bundle.Model.Operations, ApplicationRoot: applicationRoot}
	client, err := app.Dial(endpoint, app.DefaultFrameBytes, 10*time.Second)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		defer client.Close()
		if _, err := client.Finish(); err != nil {
			t.Error(err)
		}
	})
	return &applicationClient{Client: client, codec: codec}
}
func (codec *valueCodec) decodeArguments(inv app.Invocation) ([]any, error) {
	operation, ok := codec.Operations[inv.Subject]
	if !ok || len(operation.Parameters) != len(inv.Arguments) {
		return nil, fmt.Errorf("input witness needs the exact operation signature")
	}
	args := make([]any, len(inv.Arguments))
	for i, v := range inv.Arguments {
		item, err := decode(v)
		if err != nil {
			return nil, err
		}
		actual, err := codec.encode(item, operation.Parameters[i].Type)
		if err != nil {
			return nil, err
		}
		actual, err = witnessInput(actual, v)
		if err != nil {
			return nil, err
		}
		inv.Arguments[i] = actual
		args[i] = item
	}
	return args, nil
}
func (codec *valueCodec) resultValue(actual any, err error, ty typeRef) (app.OperationResult, error) {
	outcome := app.OperationResult{}
	if err != nil {
		var failure *domain.Failure
		if errors.As(err, &failure) {
			s := root + failure.Code
			payload, bindingError := codec.encode(failure.Payload, typeRef{Name: failure.Code, Symbol: &s})
			if bindingError != nil {
				return outcome, bindingError
			}
			v := value("failure", map[string]any{"declaration": s, "payload": payload})
			outcome.Observation = app.Observation{Kind: "returned", Value: &v}
			return outcome, nil
		}
		message := err.Error()
		outcome.Observation = app.Observation{Kind: "threw", Message: &message}
		return outcome, nil
	}
	v, bindingError := codec.encode(actual, ty)
	outcome.Observation = app.Observation{Kind: "returned", Value: &v}
	return outcome, bindingError
}

func (codec *valueCodec) winnerFixture(_ string, _ uint64) (app.OperationFixture, error) {
	const subject = root + "ScreeningPlanning.VotingRules.candidateOrderWinner"
	resultType := typeRef{Name: "Option", Arguments: []typeRef{{Name: "FilmId", Symbol: func() *string { s := root + "FilmId"; return &s }()}}}
	call := func(inv app.Invocation) (app.OperationResult, error) {
		if inv.Subject != subject {
			return app.OperationResult{}, fmt.Errorf("unsupported typed winner continuation: %s", inv.Subject)
		}
		args, err := codec.decodeArguments(inv)
		if err != nil {
			return app.OperationResult{}, err
		}
		if len(args) != 2 {
			return app.OperationResult{}, fmt.Errorf("winner needs two arguments")
		}
		votes, ok := args[0].([]any)
		if !ok {
			return app.OperationResult{}, fmt.Errorf("expected vote list")
		}
		candidates, ok := args[1].([]any)
		if !ok {
			return app.OperationResult{}, fmt.Errorf("expected candidate list")
		}
		actual := domain.CandidateOrderWinner(votes, candidates)
		return codec.resultValue(actual, nil, resultType)
	}
	return app.OperationFixture{Subject: subject, Production: call, Oracle: call}, nil
}

func TestCinemaWinner(t *testing.T) {
	client := clientFor(t)
	client.OperationTest(t, client.codec.winnerFixture)
}
func TestCinemaWinnerAssertions(t *testing.T) {
	client := clientFor(t)
	client.AssertionTest(t, client.codec.winnerFixture)
}

func TestCinemaFilmSave(t *testing.T) {
	client := clientFor(t)
	codec := client.codec
	const subject = root + "Catalog.FilmCatalogStore.save"
	filmSymbol := root + "FilmDetails"
	filmType := typeRef{Name: "FilmDetails", Symbol: &filmSymbol}
	client.StateChangeTest(t, func(_ string, context uint64) (app.OperationFixture, error) {
		resources := []app.ResourceObservation{}
		databasePath := filepath.Join(t.TempDir(), fmt.Sprintf("case-%d.sqlite3", context))
		store, err := persistence.New(databasePath, persistence.Sources{Now: func() string { return "2026-10-12T12:00:00Z" }, ID: func() string { return "00000000-0000-4000-8000-000000000001" }}, func(operation, entity, access string) error {
			operationID := operation
			if !strings.HasPrefix(operationID, root) {
				operationID = root + operationID
			}
			resources = append(resources, app.ResourceObservation{Operation: &operationID, Resource: root + entity, Access: access})
			return nil
		})
		if err != nil {
			return app.OperationFixture{}, err
		}
		call := func(inv app.Invocation) (app.OperationResult, error) {
			if inv.Subject != subject {
				return app.OperationResult{}, fmt.Errorf("unsupported typed save continuation: %s", inv.Subject)
			}
			args, err := codec.decodeArguments(inv)
			if err != nil {
				return app.OperationResult{}, err
			}
			resources = []app.ResourceObservation{}
			actual, productionError := store.Invoke(subject, args)
			result, bindingError := codec.resultValue(actual, productionError, filmType)
			result.Resources = resources
			return result, bindingError
		}
		setup := func(inv app.Invocation) (app.OperationResult, error) {
			if inv.Subject == subject {
				return call(inv)
			}
			if inv.Subject != root+"Catalog.FilmCatalogApi.saveFilm" {
				return app.OperationResult{}, fmt.Errorf("unsupported local save setup: %s", inv.Subject)
			}
			args, err := codec.decodeArguments(inv)
			if err != nil {
				return app.OperationResult{}, err
			}
			resources = []app.ResourceObservation{}
			events := []app.Event{}
			calls := []app.InvocationObservation{}
			ports := portsFunction(func(operation string, args []any) (any, error) {
				if root+strings.TrimPrefix(operation, root) != subject {
					return nil, fmt.Errorf("unsupported save setup port: %s", operation)
				}
				actual, productionError := store.Invoke(operation, args)
				wire, bindingError := codec.resultValue(actual, productionError, filmType)
				if bindingError != nil {
					return nil, bindingError
				}
				symbol := root + "SaveFilm"
				argument, bindingError := codec.encode(args[0], typeRef{Name: "SaveFilm", Symbol: &symbol})
				if bindingError != nil {
					return nil, bindingError
				}
				calls = append(calls, app.InvocationObservation{Invocation: app.Invocation{Subject: subject, Arguments: []Value{argument}}, Observation: wire.Observation})
				return actual, productionError
			})
			service := application.New(ports, func(event application.Event) error {
				symbol := root + event.Name
				payload, err := codec.encode(event.Payload, typeRef{Name: event.Name, Symbol: &symbol})
				if err != nil {
					return err
				}
				events = append(events, app.Event{Subject: symbol, Payload: payload})
				return nil
			})
			actual, productionError := service.Invoke("saveFilm", args)
			result, bindingError := codec.resultValue(actual, productionError, filmType)
			result.Resources = resources
			result.Events = events
			result.Calls = calls
			return result, bindingError
		}
		read := func(entities []string, _ string, _ app.Invocation) (map[string][]Value, bool, error) {
			// Entity observations must reflect the actual SQL key binding too.
			// This is a projection-completeness guard, not a modeled new rule.
			connection, err := sql.Open("sqlite", "file:"+databasePath+"?mode=ro")
			if err != nil {
				return nil, false, err
			}
			defer connection.Close()
			persisted, err := connection.Query("SELECT id,document FROM films")
			if err != nil {
				return nil, false, err
			}
			for persisted.Next() {
				var key string
				var document []byte
				if err := persisted.Scan(&key, &document); err != nil {
					persisted.Close()
					return nil, false, err
				}
				decoded, err := domain.DecodeJSON(document)
				if err != nil {
					persisted.Close()
					return nil, false, err
				}
				record, ok := decoded.(domain.Record)
				if !ok || record["id"] != key {
					persisted.Close()
					return nil, false, fmt.Errorf("physical film key differs from observed entity identity")
				}
			}
			if err := persisted.Err(); err != nil {
				persisted.Close()
				return nil, false, err
			}
			if err := persisted.Close(); err != nil {
				return nil, false, err
			}
			actual, err := store.Observe()
			if err != nil {
				return nil, false, err
			}
			rows := map[string][]Value{}
			for _, entity := range entities {
				name := strings.TrimPrefix(entity, root)
				observed, ok := actual[name]
				if !ok {
					return nil, false, fmt.Errorf("missing observed entity: %s", entity)
				}
				rows[entity] = []Value{}
				symbol := entity
				ty := typeRef{Name: name, Symbol: &symbol}
				for _, row := range observed {
					v, err := codec.encode(row, ty)
					if err != nil {
						return nil, false, err
					}
					rows[entity] = append(rows[entity], v)
				}
			}
			return rows, true, nil
		}
		return app.OperationFixture{Subject: subject, Production: call, Setup: setup, ReadState: read, Cleanup: store.Close}, nil
	})
}
