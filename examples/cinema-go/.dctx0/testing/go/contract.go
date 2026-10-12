// Testing-only wire contract. Production code imports none of these declarations.
package main

import (
	"encoding/json"
	"fmt"
)

// Value is one language-neutral testing value carried as exact JSON wire text.
// Kind selects the variant; Value holds the adjacent wire payload, if any.
type Value struct {
	Kind  string          `json:"kind"`
	Value json.RawMessage `json:"value,omitempty"`
}

// Invocation names one subject call with language-neutral arguments.
type Invocation struct {
	Subject   string  `json:"subject"`
	Arguments []Value `json:"arguments"`
}

// Observation is one returned, threw or unavailable subject outcome.
// Message and Reason are pointers so an empty message still crosses the wire.
type Observation struct {
	Kind    string  `json:"kind"`
	Value   *Value  `json:"value,omitempty"`
	Message *string `json:"message,omitempty"`
	Reason  *string `json:"reason,omitempty"`
}

// VerificationEvent is one modeled event emitted during a case.
type VerificationEvent struct {
	Subject string `json:"subject"`
	Payload Value  `json:"payload"`
}

// InvocationObservation records one actual application call observation.
type InvocationObservation struct {
	Invocation  Invocation  `json:"invocation"`
	Observation Observation `json:"observation"`
}

// ResourceObservation records actual access to a declared resource.
type ResourceObservation struct {
	Operation *string `json:"operation,omitempty"`
	Resource  string  `json:"resource"`
	Access    string  `json:"access"`
}

// ValueCheck is one accepted, rejected or unavailable validation outcome.
type ValueCheck struct {
	Kind   string  `json:"kind"`
	Value  *Value  `json:"value,omitempty"`
	Reason *string `json:"reason,omitempty"`
}

// SubjectDescription names one selected testing subject.
type SubjectDescription struct {
	Subject    string `json:"subject"`
	Role       string `json:"role"`
	Artifact   string `json:"artifact"`
	EntryPoint string `json:"entry_point"`
}

// AdapterDescription is the adapter's static testing description.
// Initialize empty slices and maps to non-nil values when describing.
type AdapterDescription struct {
	ProtocolVersion uint32               `json:"protocol_version"`
	Subjects        []SubjectDescription `json:"subjects"`
	Artifacts       []string             `json:"artifacts"`
	Capabilities    []string             `json:"capabilities"`
	Representations map[string]string    `json:"representations"`
}

// TestContext is supplied by the generated worker for one active case.
// Collaborator waits for a centrally injected mock response; Emit, RecordCall
// and RecordResource fail after session end or without their capability.
type TestContext struct {
	Collaborator   func(invocation Invocation) (Observation, error)
	Emit           func(subject string, payload Value) error
	RecordCall     func(invocation Invocation, observation Observation) error
	RecordResource func(observation ResourceObservation) error
}

// TestSession is one active case session. Validate and Observe are optional
// capabilities discovered by the worker through type assertion.
type TestSession interface {
	Invoke(invocation Invocation) (Observation, error)
	Close() error
}

// ValueValidator is implemented by sessions that can validate wire values.
type ValueValidator interface {
	Validate(subject string, value Value) (ValueCheck, error)
}

// StateObserver is implemented by sessions that can observe entity states.
type StateObserver interface {
	Observe(entities []string) (map[string][]Value, error)
}

// ProjectAdapter is implemented once per project in the adapter file.
// The generated worker consumes the package-level variable `adapter`.
type ProjectAdapter interface {
	Describe() (AdapterDescription, error)
	Begin(caseID string, context TestContext) (TestSession, error)
}

// InvokeApplication captures only the actual application call; translation
// failures stay technical and application errors become threw observations.
func InvokeApplication[T any](operation func() (T, error), encode func(T) Value) Observation {
	result, err := CallApplication(operation)
	if err != nil {
		return Observation{Kind: "threw", Message: pointerTo(err.Error())}
	}
	value := encode(result)
	return Observation{Kind: "returned", Value: &value}
}

func pointerTo[T any](value T) *T {
	return &value
}

// CallApplication recovers panics only around actual application code. Translation
// and adapter errors remain technical failures outside this boundary.
func CallApplication[T any](operation func() (T, error)) (result T, err error) {
	defer func() {
		if recovered := recover(); recovered != nil {
			err = fmt.Errorf("Application panic: %v", recovered)
		}
	}()
	return operation()
}
