// Implement this testing adapter with real project calls. Retain existing code.
package main

import "errors"

type unimplementedAdapter struct{}

func (unimplementedAdapter) Describe() (AdapterDescription, error) {
	return AdapterDescription{ProtocolVersion: 1}, nil
}

func (unimplementedAdapter) Begin(string, TestContext) (TestSession, error) {
	return nil, errors.New("Project testing adapter is not implemented")
}

// adapter is the testing-only project adapter consumed by the generated worker.
var adapter ProjectAdapter = unimplementedAdapter{}
