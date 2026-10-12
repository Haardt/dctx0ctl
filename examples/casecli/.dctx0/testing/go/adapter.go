package main

import (
	"casecli/textcase"
	"encoding/json"
	"fmt"
)

type caseAdapter struct{}
type caseSession struct{}

var adapter ProjectAdapter = caseAdapter{}

func (caseAdapter) Describe() (AdapterDescription, error) {
	return AdapterDescription{ProtocolVersion: 1, Subjects: []SubjectDescription{
		{Subject: "CaseCli.Text.upper", Role: "operation", Artifact: "textcase/textcase.go", EntryPoint: "textcase.Upper"},
		{Subject: "CaseCli.Text.lower", Role: "operation", Artifact: "textcase/textcase.go", EntryPoint: "textcase.Lower"},
	}, Artifacts: []string{"textcase/textcase.go", "cmd/casecli/main.go"}, Capabilities: []string{"sessions", "invocation", "events"}, Representations: map[string]string{"String": "Go UTF-8 string; Unicode simple case conversion"}}, nil
}
func (caseAdapter) Begin(string, TestContext) (TestSession, error) { return caseSession{}, nil }
func (caseSession) Close() error                                   { return nil }
func (caseSession) Invoke(i Invocation) (Observation, error) {
	if len(i.Arguments) != 1 || i.Arguments[0].Kind != "string" {
		return Observation{}, fmt.Errorf("one String argument required")
	}
	var text string
	if err := json.Unmarshal(i.Arguments[0].Value, &text); err != nil {
		return Observation{}, err
	}
	var result string
	switch i.Subject {
	case "CaseCli.Text.upper":
		result = textcase.Upper(text)
	case "CaseCli.Text.lower":
		result = textcase.Lower(text)
	default:
		return Observation{}, fmt.Errorf("unknown subject %s", i.Subject)
	}
	raw, err := json.Marshal(result)
	if err != nil {
		return Observation{}, err
	}
	value := Value{Kind: "string", Value: raw}
	return Observation{Kind: "returned", Value: &value}, nil
}
