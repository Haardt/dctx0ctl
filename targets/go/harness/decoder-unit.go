// Compiler-checked direct decoder roots. This file is generated testing code.
package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	// __DECODER_IMPORTS__
)

type decoderAdapter struct{}
type decoderSession struct{}

func (decoderAdapter) Describe() (AdapterDescription, error) {
	return AdapterDescription{ProtocolVersion: 1, Subjects: []SubjectDescription{
			// __DECODER_SUBJECTS__
	}, Artifacts: []string{}, Capabilities: []string{"values"}, Representations: map[string]string{}}, nil
}
func (decoderAdapter) Begin(string, TestContext) (TestSession, error) { return decoderSession{}, nil }
func (decoderSession) Close() error                                   { return nil }
func (decoderSession) Invoke(Invocation) (Observation, error) {
	return Observation{}, AdapterError(fmt.Errorf("Decoder units do not expose operations"))
}
func decoderJSON(value Value) (json.RawMessage, error) {
	switch value.Kind {
	case "unit":
		return json.RawMessage("null"), nil
	case "integer", "decimal", "float":
		var text string
		if err := json.Unmarshal(value.Value, &text); err != nil {
			return nil, err
		}
		return json.RawMessage(text), nil
	case "record":
		var fields map[string]Value
		if err := json.Unmarshal(value.Value, &fields); err != nil {
			return nil, err
		}
		result := map[string]json.RawMessage{}
		for key, field := range fields {
			raw, err := decoderJSON(field)
			if err != nil {
				return nil, err
			}
			result[key] = raw
		}
		return json.Marshal(result)
	case "list":
		var items []Value
		if err := json.Unmarshal(value.Value, &items); err != nil {
			return nil, err
		}
		result := []json.RawMessage{}
		for _, item := range items {
			raw, err := decoderJSON(item)
			if err != nil {
				return nil, err
			}
			result = append(result, raw)
		}
		return json.Marshal(result)
	case "boolean", "string":
		return value.Value, nil
	default:
		// Unsupported wire variants are outside the JSON factory types accepted
		// by this target. Preserve that invalid shape as an explicit extra member.
		return json.Marshal(map[string]string{"$dctx0_unsupported_wire_kind": value.Kind})
	}
}

// Encode the ACTUAL returned production value, never echo an expected result.
func decoderValue(value any) (Value, error) {
	var kind string
	var payload any
	switch actual := value.(type) {
	case nil:
		return Value{Kind: "unit"}, nil
	case json.Number:
		kind, payload = "integer", actual.String()
	case string:
		kind, payload = "string", actual
	case bool:
		kind, payload = "boolean", actual
	case map[string]any:
		kind = "record"
		fields := map[string]Value{}
		for key, field := range actual {
			encoded, err := decoderValue(field)
			if err != nil {
				return Value{}, err
			}
			fields[key] = encoded
		}
		payload = fields
	case []any:
		kind = "list"
		items := []Value{}
		for _, item := range actual {
			encoded, err := decoderValue(item)
			if err != nil {
				return Value{}, err
			}
			items = append(items, encoded)
		}
		payload = items
	default:
		return Value{}, fmt.Errorf("Unsupported actual decoder result")
	}
	raw, err := json.Marshal(payload)
	return Value{Kind: kind, Value: raw}, err
}

func callDecoder[T any](subject string, value Value, decode func([]byte) (T, error)) (ValueCheck, error) {
	raw, err := decoderJSON(value)
	if err != nil {
		return ValueCheck{}, AdapterError(err)
	}
	println("DCTX0_UNIT_PRODUCTION_CALL:" + subject)
	actual, err := decode(raw)
	if err != nil {
		reason := err.Error()
		return ValueCheck{Kind: "rejected", Reason: &reason}, nil
	}
	encoded, err := json.Marshal(actual)
	if err != nil {
		return ValueCheck{}, AdapterError(err)
	}
	reader := json.NewDecoder(bytes.NewReader(encoded))
	reader.UseNumber()
	var result any
	if err = reader.Decode(&result); err != nil {
		return ValueCheck{}, AdapterError(err)
	}
	observed, err := decoderValue(result)
	if err != nil {
		return ValueCheck{}, AdapterError(err)
	}
	return ValueCheck{Kind: "accepted", Value: &observed}, nil
}

func (decoderSession) Validate(subject string, value Value) (ValueCheck, error) {
	switch subject {
	// __DECODER_CALLS__
	default:
		return ValueCheck{}, AdapterError(fmt.Errorf("Unregistered decoder %s", subject))
	}
}

var adapter ProjectAdapter = decoderAdapter{}

func init() { println("DCTX0_UNIT_WORKER_START") }
