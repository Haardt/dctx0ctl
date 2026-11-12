package applicationtests

import (
	"crypto/sha256"
	"fmt"
	"testing"
)

type Invocation struct {
	Subject   string  `json:"subject"`
	Arguments []Value `json:"arguments"`
}
type Observation struct {
	Kind    string  `json:"kind"`
	Value   *Value  `json:"value,omitempty"`
	Message *string `json:"message,omitempty"`
	Reason  *string `json:"reason,omitempty"`
}
type InvocationObservation struct {
	Invocation  Invocation  `json:"invocation"`
	Observation Observation `json:"observation"`
}
type Event struct {
	Subject string `json:"subject"`
	Payload Value  `json:"payload"`
}
type ResourceObservation struct {
	Operation *string `json:"operation,omitempty"`
	Resource  string  `json:"resource"`
	Access    string  `json:"access"`
}
type OperationResult struct {
	Observation Observation
	Calls       []InvocationObservation
	Events      []Event
	Resources   []ResourceObservation
}

// Callbacks return conversion/fixture errors separately from actual production
// outcomes. Nil trace slices mean absent observers; empty slices mean observed zero.
// Continuations are local typed closures, never an application-wide dispatcher.
type OperationFixture struct {
	Subject    string
	Subjects   []string
	Production func(Invocation) (OperationResult, error)
	Setup      func(Invocation) (OperationResult, error)
	Oracle     func(Invocation) (OperationResult, error)
	ReadState  func(entities []string, phase string, invocation Invocation) (map[string][]Value, bool, error)
	Cleanup    func() error
}
type FixtureFactory func(caseID string, context uint64) (OperationFixture, error)

// CaseContext owns evaluator capabilities for this exact case, never globals.
type CaseContext struct {
	ID             string
	Context        uint64
	InitialSubject string
	Mocks          []string
	Collaborate    func(Invocation) (Observation, error)
}
type ContextFixtureFactory func(*CaseContext) (OperationFixture, error)

func (client *Client) OperationContextTest(t *testing.T, factory ContextFixtureFactory) {
	client.runOperationTest(t, "operation", factory)
}
func (client *Client) StateChangeContextTest(t *testing.T, factory ContextFixtureFactory) {
	client.runOperationTest(t, "state-change", factory)
}
func (client *Client) ScenarioTest(t *testing.T, factory ContextFixtureFactory) {
	client.runOperationTest(t, "scenario", factory)
}
func (client *Client) LifecycleTest(t *testing.T, factory ContextFixtureFactory) {
	client.runOperationTest(t, "lifecycle", factory)
}
func (client *Client) IdentityTest(t *testing.T) { client.runOperationTest(t, "entity-identity", nil) }

func callOperation(call func(Invocation) (OperationResult, error), invocation Invocation) (result OperationResult, err error) {
	defer func() {
		if recovered := recover(); recovered != nil {
			result = OperationResult{Observation: Observation{Kind: "threw", Message: messagePointer(fmt.Sprint(recovered))}}
			err = nil
		}
	}()
	return call(invocation)
}
func cleanupFixture(fixture OperationFixture) (err error) {
	defer func() {
		if recovered := recover(); recovered != nil {
			err = fmt.Errorf("cleanup panic: %v", recovered)
		}
	}()
	if fixture.Cleanup != nil {
		return fixture.Cleanup()
	}
	return nil
}
func makeFixture(factory FixtureFactory, id string, context uint64) (fixture OperationFixture, err error) {
	defer func() {
		if recovered := recover(); recovered != nil {
			err = fmt.Errorf("fixture panic: %v", recovered)
		}
	}()
	return factory(id, context)
}

func (client *Client) AssertionTest(t *testing.T, factory FixtureFactory) {
	client.operationTest(t, "assertion", factory)
}

func (client *Client) OperationTest(t *testing.T, factory FixtureFactory) {
	client.operationTest(t, "operation", factory)
}
func (client *Client) StateChangeTest(t *testing.T, factory FixtureFactory) {
	client.operationTest(t, "state-change", factory)
}
func (client *Client) operationTest(t *testing.T, form string, factory FixtureFactory) {
	if factory == nil {
		t.Fatal("application fixture factory required")
	}
	client.runOperationTest(t, form, func(context *CaseContext) (OperationFixture, error) { return factory(context.ID, context.Context) })
}
func (client *Client) runOperationTest(t *testing.T, form string, factory ContextFixtureFactory) {
	t.Helper()
	if factory == nil && form != "entity-identity" {
		t.Fatal("application fixture factory required")
	}
	test := t.Name()
	registered, err := client.exchange(map[string]any{"kind": "register", "test": test})
	if err != nil || registered.Kind != "registered" {
		t.Fatalf("register native test: %v", err)
	}
	defer func() {
		reply, err := client.exchange(map[string]any{"kind": "close-test", "test": test})
		if err != nil || reply.Kind != "test-closed" {
			t.Errorf("finalize native test: %v", err)
		}
	}()
	for {
		next, err := client.exchange(map[string]any{"kind": "next", "test": test})
		if err != nil {
			t.Fatal(err)
		}
		if next.Kind == "no-more-cases" {
			return
		}
		if next.Kind != "case" || next.Form != form || next.Context == 0 || next.Directive == nil {
			t.Fatal("unexpected native operation case")
		}
		t.Run(fmt.Sprintf("case_%x", sha256.Sum256([]byte(next.ID))), func(t *testing.T) {
			context := &CaseContext{ID: next.ID, Context: next.Context, Mocks: next.Mocks}
			if next.Directive.Invocation != nil {
				context.InitialSubject = next.Directive.Invocation.Subject
			}
			context.Collaborate = func(invocation Invocation) (Observation, error) {
				response, err := client.exchange(map[string]any{"kind": "collaborator", "context": next.Context, "invocation": invocation})
				if err != nil {
					return Observation{}, err
				}
				if response.Kind != "collaborator-result" || response.Context != next.Context || response.Observation == nil {
					return Observation{}, fmt.Errorf("invalid collaborator reply")
				}
				return *response.Observation, nil
			}
			var fixture OperationFixture
			var bindingError error
			if next.Directive.Kind != "verdict" {
				fixture, bindingError = makeFixture(func(_ string, _ uint64) (OperationFixture, error) { return factory(context) }, next.ID, next.Context)
			}
			defer func() {
				var cleanup *string
				if err := cleanupFixture(fixture); err != nil {
					cleanup = messagePointer(err.Error())
					t.Error(err)
				}
				closed, err := client.exchange(map[string]any{"kind": "close-case", "context": next.Context, "cleanup_error": cleanup})
				if err != nil || closed.Kind != "case-closed" {
					t.Errorf("close operation case: %v", err)
					return
				}
				verdictError(t, closed.Result, next.ID)
			}()
			action := next.Directive
			for {
				if action.Kind == "verdict" {
					verdictError(t, action.Result, next.ID)
					return
				}
				observation := map[string]any{}
				if bindingError == nil {
					switch action.Kind {
					case "invoke":
						if action.Invocation == nil {
							bindingError = fmt.Errorf("missing native invocation")
							break
						}
						call := fixture.Production
						switch action.Phase {
						case "production":
							allowed := fixture.Subject == action.Invocation.Subject
							for _, subject := range fixture.Subjects {
								allowed = allowed || subject == action.Invocation.Subject
							}
							if !allowed {
								bindingError = fmt.Errorf("production subject differs from local fixture")
								continue
							}
						case "setup":
							call = fixture.Setup
						case "oracle":
							call = fixture.Oracle
						default:
							bindingError = fmt.Errorf("unsupported invocation phase")
							continue
						}
						if call == nil {
							bindingError = fmt.Errorf("unsupported local continuation: %s", action.Phase)
							break
						}
						result, err := callOperation(call, *action.Invocation)
						if err != nil {
							bindingError = err
							break
						}
						observation = map[string]any{"kind": "invoked", "invocation": action.Invocation, "observation": result.Observation, "calls": result.Calls, "events": result.Events, "resources": result.Resources}
					case "read-state":
						if fixture.ReadState == nil || action.Invocation == nil {
							bindingError = fmt.Errorf("state observer unavailable")
							break
						}
						state, complete, err := fixture.ReadState(action.Entities, action.Phase, *action.Invocation)
						if err != nil {
							bindingError = err
							break
						}
						observation = map[string]any{"kind": "state", "entities": state, "phase": action.Phase, "invocation": action.Invocation, "complete": complete}
					default:
						bindingError = fmt.Errorf("unsupported operation directive: %s", action.Kind)
					}
				}
				if bindingError != nil {
					observation = map[string]any{"kind": "binding-error", "message": bindingError.Error()}
				}
				reply, err := client.exchange(map[string]any{"kind": "observe", "context": next.Context, "observation": observation})
				if err != nil {
					t.Fatal(err)
				}
				if reply.Kind != "action" || reply.Context != next.Context || reply.Directive == nil {
					t.Fatal("missing operation continuation")
				}
				context.Mocks = reply.Mocks
				action = reply.Directive
			}
		})
	}
}
