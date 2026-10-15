// Project testing adapter fixture for Go transport conformance.
package main

import (
	"encoding/json"
	"errors"
	"log"
	"os"
	"strconv"
	"strings"
	"time"
)

func integer(value int) Value {
	raw, err := json.Marshal(strconv.Itoa(value))
	if err != nil {
		panic(err)
	}
	return Value{Kind: "integer", Value: raw}
}

func number(value Value) (int, error) {
	if value.Kind != "integer" {
		return 0, errors.New("Expected an integer value")
	}
	var text string
	if err := json.Unmarshal(value.Value, &text); err != nil {
		return 0, errors.New("Expected integer text")
	}
	return strconv.Atoi(text)
}

func textOf(value Value) string {
	if value.Kind != "string" {
		return ""
	}
	var text string
	if err := json.Unmarshal(value.Value, &text); err != nil {
		return ""
	}
	return text
}

func record(fields map[string]Value) Value {
	raw, err := json.Marshal(fields)
	if err != nil {
		panic(err)
	}
	return Value{Kind: "record", Value: raw}
}

type counterAdapter struct{}

func (counterAdapter) Describe() (AdapterDescription, error) {
	return AdapterDescription{
		ProtocolVersion: 1,
		Subjects: []SubjectDescription{
			{Subject: "operation", Role: "operation", Artifact: "counter.go", EntryPoint: "Counter.Increment"},
			{Subject: "value", Role: "value", Artifact: "counter.go", EntryPoint: "Counter"},
			{Subject: "Entity", Role: "entity", Artifact: "counter.go", EntryPoint: "Counter"},
		},
		Artifacts:    []string{"counter.go"},
		Capabilities: []string{"sessions", "invocation", "state", "events", "mock-injection", "call-observation", "resource-observation"},
		Representations: map[string]string{
			"Int": "text integers",
		},
	}, nil
}

func (counterAdapter) Begin(caseID string, context TestContext) (TestSession, error) {
	if caseID == "init-error" {
		return nil, errors.New("Initialization unavailable")
	}
	counter := &Counter{}
	counter.load = func() (int, error) {
		response, err := context.Collaborator(Invocation{
			Subject:   "port",
			Arguments: []Value{integer(counter.value)},
		})
		if err != nil {
			return 0, err
		}
		switch response.Kind {
		case "returned":
			if response.Value == nil {
				return 0, errors.New("Missing collaborator value")
			}
			return number(*response.Value)
		case "threw":
			if response.Message == nil {
				return 0, errors.New("Collaborator threw")
			}
			return 0, errors.New(*response.Message)
		case "unavailable":
			if response.Reason == nil {
				return 0, errors.New("Collaborator unavailable")
			}
			return 0, errors.New(*response.Reason)
		default:
			return 0, errors.New("Unexpected collaborator response")
		}
	}
	counter.changed = func(value int) error {
		return context.Emit("CounterChanged", integer(value))
	}
	return &counterSession{
		counter: counter,
		context: context,
		cleanup: caseID == "cleanup-error",
		caseID:  caseID,
	}, nil
}

type counterSession struct {
	counter *Counter
	context TestContext
	cleanup bool
	caseID  string
}

func (session *counterSession) Invoke(invocation Invocation) (Observation, error) {
	log.Print("actual application output")
	if len(invocation.Arguments) == 0 {
		return Observation{}, errors.New("Missing invocation argument")
	}
	command := invocation.Arguments[0]
	var observation Observation
	switch {
	case command.Kind == "string" && textOf(command) == "application-panic":
		observation = InvokeApplication(session.counter.Panic, integer)
	case command.Kind == "string" && textOf(command) == "exception":
		observation = InvokeApplication(func() (int, error) {
			return 0, session.counter.Crash()
		}, integer)
	case command.Kind == "string" && textOf(command) == "adapter-error":
		return Observation{}, errors.New("Invalid adapter translation")
	case command.Kind == "string" && textOf(command) == "caught-observation-limit":
		for index := 0; index < 10001; index++ {
			if err := session.context.RecordResource(ResourceObservation{Resource: "Counter", Access: "read"}); err != nil {
				break
			}
		}
		value := integer(session.counter.value)
		return Observation{Kind: "returned", Value: &value}, nil
	case command.Kind == "string" && textOf(command) == "observation-limit":
		for index := 0; index < 10001; index++ {
			if err := session.context.RecordResource(ResourceObservation{Resource: "Counter", Access: "read"}); err != nil {
				return Observation{}, err
			}
		}
		value := integer(session.counter.value)
		return Observation{Kind: "returned", Value: &value}, nil
	case command.Kind == "string" && textOf(command) == "mutable-observation":
		value, err := session.counter.Increment(1)
		if err != nil {
			return Observation{}, err
		}
		resourceRecord := ResourceObservation{Resource: "Counter", Access: "write"}
		if err := session.context.RecordResource(resourceRecord); err != nil {
			return Observation{}, err
		}
		resourceRecord.Resource = "mutated-after-recording"
		payload := integer(value)
		if err := session.context.Emit("Snapshot", payload); err != nil {
			return Observation{}, err
		}
		payload.Value = json.RawMessage(`"999"`)
		result := integer(value)
		return Observation{Kind: "returned", Value: &result}, nil
	case command.Kind == "string" && textOf(command) == "invalid-resource":
		return Observation{}, session.context.RecordResource(ResourceObservation{Resource: "Counter", Access: "delete"})
	case command.Kind == "string" && textOf(command) == "mock":
		observation = InvokeApplication(session.counter.Refresh, integer)
	default:
		amount, err := number(command)
		if err != nil {
			return Observation{}, err
		}
		observation = InvokeApplication(func() (int, error) {
			return session.counter.Increment(amount)
		}, integer)
	}
	if err := session.context.RecordCall(invocation, observation); err != nil {
		return Observation{}, err
	}
	return observation, nil
}

func (session *counterSession) Validate(_ string, value Value) (ValueCheck, error) {
	if value.Kind != "integer" {
		return ValueCheck{Kind: "rejected", Reason: pointerTo("Expected a safe integer")}, nil
	}
	if _, err := number(value); err != nil {
		return ValueCheck{Kind: "rejected", Reason: pointerTo(err.Error())}, nil
	}
	return ValueCheck{Kind: "accepted", Value: &value}, nil
}

func (session *counterSession) Observe(entities []string) (map[string][]Value, error) {
	states := make(map[string][]Value, len(entities))
	for _, entity := range entities {
		states[entity] = []Value{record(map[string]Value{"count": integer(session.counter.value)})}
	}
	return states, nil
}

func (session *counterSession) Close() error {
	if strings.HasPrefix(session.caseID, "cleanup-order-") {
		time.Sleep(100 * time.Millisecond)
		if err := os.WriteFile(session.caseID+".closed", []byte("closed"), 0600); err != nil {
			return err
		}
	}

	if session.caseID == "cleanup-order-panic" {
		panic("Cleanup panicked")
	}
	if session.cleanup {
		return errors.New("Cleanup unavailable")
	}
	return nil
}

var adapter ProjectAdapter = counterAdapter{}
