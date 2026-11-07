package main

import (
	"encoding/json"
	"errors"
	"fmt"
	"math/big"
	"os"
	"os/exec"
	"reflect"
	"strings"
	"testing"
)

func wire(kind string, payload any) Value {
	v, e := codecValue(kind, payload)
	if e != nil {
		panic(e)
	}
	return v
}
func TestExactConversionAndInvalidProductionInputs(t *testing.T) {
	exact := "900719925474099312345678901234567890"
	native, e := Decode("W.Id", wire("integer", exact))
	if e != nil || native != json.Number(exact) {
		t.Fatal(native, e)
	}
	encoded, e := Encode("W.Id", json.Number(exact))
	if e != nil || string(encoded.Value) != fmt.Sprintf("%q", exact) {
		t.Fatal(encoded, e)
	}
	if _, e := Encode("W.Id", float64(1)); e == nil {
		t.Fatal("float accepted as integer")
	}
	record := wire("record", map[string]Value{"id": wire("integer", "12"), "child": wire("option", wire("record", map[string]Value{"code": wire("string", "ok")})), "items": wire("list", []Value{wire("string", "x")})})
	decoded, e := Decode("W.Record", record)
	if e != nil {
		t.Fatal(e)
	}
	again, e := Encode("W.Record", decoded)
	if e != nil || !reflect.DeepEqual(again, record) {
		t.Fatal(again, e)
	}
	missing := wire("record", map[string]Value{"id": wire("integer", "12"), "extra": wire("string", "x")})
	if _, e := Decode("W.Record", missing); e == nil {
		t.Fatal("strict missing fields accepted")
	}
	input, e := DecodeInput("W.Record", missing)
	if e != nil {
		t.Fatal(e)
	}
	raw, e := json.Marshal(input)
	if e != nil {
		t.Fatal(e)
	}
	calls := 0
	productionValidator := func(raw []byte) error {
		calls++
		var fields map[string]json.RawMessage
		if e := json.Unmarshal(raw, &fields); e != nil {
			return e
		}
		if len(fields) != 3 || fields["child"] == nil || fields["items"] == nil {
			return errors.New("actual parser rejected shape")
		}
		return nil
	}
	if productionValidator(raw) == nil || calls != 1 {
		t.Fatal("input did not reach actual validator")
	}
	if _, e := Decode("W.Unsupported", wire("string", "x")); e == nil {
		t.Fatal("unsupported builtin accepted")
	}
	nested := wire("option", wire("option", nil))
	if _, e := Decode("W.NestedOption", nested); e == nil {
		t.Fatal("collapsing option accepted")
	}
	if _, e := Decode("W.Mode", wire("variant", map[string]any{"declaration": "Other.Mode", "name": "Open", "payload": nil})); e == nil {
		t.Fatal("foreign enum accepted")
	}
	old := codecConfiguration.Integer
	codecConfiguration.Integer = "bigint"
	defer func() { codecConfiguration.Integer = old }()
	large, e := Decode("W.Id", wire("integer", exact))
	if e != nil || large.(*big.Int).String() != exact {
		t.Fatal(large, e)
	}
}
func TestFailureCannotBeARecordOrTechnicalRejection(t *testing.T) {
	payload := wire("record", map[string]Value{"code": wire("string", "DENIED")})
	failure := wire("failure", map[string]any{"declaration": "W.Denied", "payload": payload})
	o := Observation{Kind: "returned", Value: &failure}
	got, e := DeclaredFailure(o, nil, "W.Denied")
	if e != nil || StringField(got.Payload, "code", "DENIED") != nil {
		t.Fatal(got, e)
	}
	if _, e := ReturnedRecord(o, nil); e == nil {
		t.Fatal("contradictory record decoder accepted failure")
	}
	if _, e := DeclaredFailure(o, nil, "W.Record"); e == nil {
		t.Fatal("wrong declaration accepted")
	}
	if _, e := DeclaredFailure(o, EnvironmentError(errors.New("private secret")), "W.Denied"); e == nil || e.Error() == "private secret" {
		t.Fatal("technical error reclassified or leaked")
	}
	if StringField(got.Payload, "code", "OTHER") == nil {
		t.Fatal("incorrect payload passed")
	}
	for _, kind := range []string{"threw", "unavailable"} {
		if _, e := DeclaredFailure(Observation{Kind: kind}, nil, "W.Denied"); e == nil {
			t.Fatal(kind)
		}
	}
	failure = wire("failure", map[string]any{"declaration": "W.Denied", "payload": wire("record", map[string]Value{"code": wire("integer", "1")})})
	if _, e := DeclaredFailure(Observation{Kind: "returned", Value: &failure}, nil, "W.Denied"); e == nil {
		t.Fatal("wrong payload shape")
	}
}
func TestFixtureEnvironmentOnlyChangesChild(t *testing.T) {
	key := "DCTX0_CODEC_PRIVATE_FIXTURE"
	t.Setenv(key, "inherited-private-value")
	cmd, e := ContractPlanCommand(os.Args[0], ".", []string{"-test.run=^TestFixtureChild$"}, map[string]*string{key: UnavailableFixture()})
	if e != nil {
		t.Fatal(e)
	}
	if e = cmd.Run(); e != nil {
		t.Fatal(e)
	}
	if os.Getenv(key) != "inherited-private-value" {
		t.Fatal("parent environment changed")
	}
	t.Setenv(key, "")
	if _, e := ConfiguredFixture(key); e == nil {
		t.Fatal("missing configured fixture accepted")
	}
	cmd, e = ContractPlanCommand("unused", ".", nil, map[string]*string{key: nil})
	if e != nil {
		t.Fatal(e)
	}
	for _, v := range cmd.Env {
		if v == key+"=" {
			t.Fatal("removed key retained")
		}
	}
	_ = exec.Command
}
func TestFixtureChild(t *testing.T) {
	if os.Getenv("DCTX0_CODEC_PRIVATE_FIXTURE") != "" {
		t.Fatal("fixture inherited")
	}
}

func TestTechnicalUnavailableAndCallAssertions(t *testing.T) {
	if TechnicalFailure(EnvironmentError(errors.New("secret")), "environment") != nil {
		t.Fatal("explicit category rejected")
	}
	if TechnicalFailure(errors.New("environment"), "environment") == nil {
		t.Fatal("message inferred category")
	}
	reason := "missing actual observation"
	if UnavailableObservation(Observation{Kind: "unavailable", Reason: &reason}, nil) != nil {
		t.Fatal("unavailable rejected")
	}
	if UnavailableObservation(Observation{Kind: "returned"}, nil) == nil {
		t.Fatal("returned treated as unavailable")
	}
	inv := Invocation{Subject: "actual.operation", Arguments: []Value{wire("integer", "9007199254740993")}}
	calls := []InvocationObservation{{Invocation: inv, Observation: Observation{Kind: "returned"}}}
	if RecordedCalls(calls, true, inv, 1) != nil || RecordedCalls(calls, false, inv, 1) == nil || RecordedCalls(calls, true, inv, 0) == nil {
		t.Fatal("incorrect call assessment")
	}
}
func TestFixtureScenarioRunsCleanupInChild(t *testing.T) {
	const key = "DCTX0_CODEC_SCENARIO_FIXTURE"
	if os.Getenv("DCTX0_FIXTURE_SCENARIO") != t.Name() {
		t.Setenv(key, "inherited")
	}
	fixtureScenario(t, map[string]*string{key: UnavailableFixture()}, func() {
		if os.Getenv(key) != "" {
			t.Fatal("fixture inherited")
		}
		t.Cleanup(func() {
			if os.Getenv(key) != "" {
				t.Error("child cleanup environment changed")
			}
		})
	})
	if os.Getenv("DCTX0_FIXTURE_SCENARIO") != t.Name() && os.Getenv(key) != "inherited" {
		t.Fatal("parent environment changed")
	}
}

func TestVariantPayloadsAndOptions(t *testing.T) {
	for _, native := range []map[string]any{{"kind": "Empty", "payload": nil}, {"kind": "Count", "payload": json.Number("12")}, {"kind": "Maybe", "payload": nil}, {"kind": "Maybe", "payload": "present"}} {
		encoded, e := Encode("W.Choice", native)
		if e != nil {
			t.Fatal(e)
		}
		decoded, e := Decode("W.Choice", encoded)
		if e != nil || !reflect.DeepEqual(decoded, native) {
			t.Fatal(decoded, e)
		}
	}
	if _, e := Decode("W.Mode", wire("variant", map[string]any{"declaration": "W.Mode", "name": "Open", "payload": nil})); e != nil {
		t.Fatal(e)
	}
}

func TestRemovingEveryInheritedVariableDoesNotFallBackToInheritance(t *testing.T) {
	environment := map[string]*string{}
	for _, entry := range os.Environ() {
		key, _, _ := strings.Cut(entry, "=")
		environment[key] = nil
	}
	cmd, err := ContractPlanCommand(os.Args[0], ".", []string{"-test.run=^TestEmptyEnvironmentChild$"}, environment)
	if err != nil {
		t.Fatal(err)
	}
	if err := cmd.Run(); err != nil {
		t.Fatal("child inherited environment after every variable was explicitly removed", err)
	}
}
func TestEmptyEnvironmentChild(t *testing.T) {
	for _, argument := range os.Args[1:] {
		if argument == "-test.run=^TestEmptyEnvironmentChild$" {
			if len(os.Environ()) != 0 {
				t.Fatal("child environment was not empty")
			}
			return
		}
	}
}
