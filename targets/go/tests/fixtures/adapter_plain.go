// Project testing adapter fixture without optional testing capabilities.
package main

type plainAdapter struct{}

func (plainAdapter) Describe() (AdapterDescription, error) {
	return AdapterDescription{ProtocolVersion: 1}, nil
}

func (plainAdapter) Begin(_ string, context TestContext) (TestSession, error) {
	return &plainSession{context: context}, nil
}

type plainSession struct {
	context TestContext
}

func (session *plainSession) Invoke(invocation Invocation) (Observation, error) {
	if len(invocation.Arguments) > 0 && invocation.Arguments[0].Kind == "string" {
		return Observation{}, session.context.RecordResource(ResourceObservation{
			Resource: "Counter",
			Access:   "read",
		})
	}
	unit := Value{Kind: "unit"}
	return Observation{Kind: "returned", Value: &unit}, nil
}

func (session *plainSession) Close() error {
	return nil
}

var adapter ProjectAdapter = plainAdapter{}
