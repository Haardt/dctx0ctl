package main

import (
	"encoding/json"
	"fixture/production"
	"fmt"
	"math/big"
)

type actualAdapter struct{}
type actualSession struct{}

var adapter ProjectAdapter = actualAdapter{}

func (actualAdapter) Describe() (AdapterDescription, error) {
	return AdapterDescription{ProtocolVersion: 1, Subjects: []SubjectDescription{{Subject: "W.Api.next", Role: "operation", Artifact: "production/next.go", EntryPoint: "production.Next"}, {Subject: "W.Count", Role: "validator", Artifact: "production/next.go", EntryPoint: "production.ValidateCount"}}, Artifacts: []string{"production/next.go"}, Capabilities: []string{"sessions", "invocation", "events", "values"}, Representations: map[string]string{"Int": "math/big.Int"}}, nil
}
func (actualAdapter) Begin(string, TestContext) (TestSession, error) { return actualSession{}, nil }
func (actualSession) Close() error                                   { return nil }
func (actualSession) Invoke(invocation Invocation) (Observation, error) {
	var text string
	if e := json.Unmarshal(invocation.Arguments[0].Value, &text); e != nil {
		return Observation{}, AdapterError(e)
	}
	input, ok := new(big.Int).SetString(text, 10)
	if !ok {
		return Observation{}, AdapterError(fmt.Errorf("unrepresentable integer"))
	}
	result := production.Next(input)
	raw, e := json.Marshal(result.String())
	if e != nil {
		return Observation{}, AdapterError(e)
	}
	v := Value{Kind: "integer", Value: raw}
	return Observation{Kind: "returned", Value: &v}, nil
}
func (actualSession) Validate(subject string, value Value) (ValueCheck, error) {
	// Preserve representable negative shapes at the actual production JSON boundary.
	var native any
	switch value.Kind {
	case "option", "variant", "failure":
		// Authored foreign-type mapping: retain the complete tagged envelope.
		// Nullable options cannot preserve Some(None); generated conversion is
		// deliberately unavailable for that shape.
		native = value
	default:
		var err error
		native, err = DecodeInput(subject, value)
		if err != nil {
			return ValueCheck{}, AdapterError(err)
		}
	}
	raw, err := json.Marshal(native)
	if err != nil {
		return ValueCheck{}, AdapterError(err)
	}
	if !production.ValidateCount(raw) {
		reason := "production count parser rejected input"
		return ValueCheck{Kind: "rejected", Reason: &reason}, nil
	}
	return ValueCheck{Kind: "accepted", Value: &value}, nil
}
