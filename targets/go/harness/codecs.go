// Testing representations only. Call the real production validator after DecodeInput.
package main

import (
	"encoding/json"
	"errors"
	"fmt"
	"math"
	"math/big"
	"regexp"
	"strconv"
)

type codecRef struct {
	Name      string     `json:"name"`
	Symbol    *string    `json:"symbol_id"`
	Arguments []codecRef `json:"arguments"`
}
type codecData struct {
	Alias  *codecRef `json:"alias"`
	Fields []struct {
		Name string   `json:"name"`
		Type codecRef `json:"ty"`
	} `json:"fields"`
	Variants []struct {
		Name    string    `json:"name"`
		Payload *codecRef `json:"payload"`
	} `json:"variants"`
}
type codecOptions struct {
	Integer string            `json:"integer"`
	Option  string            `json:"option"`
	Enum    string            `json:"enum"`
	Types   map[string]string `json:"types"`
}

var codecDeclarations map[string]codecData
var codecConfiguration codecOptions

func init() {
	if err := json.Unmarshal([]byte(codecDescriptor), &codecDeclarations); err != nil {
		panic(err)
	}
	if err := json.Unmarshal([]byte(codecSettings), &codecConfiguration); err != nil {
		panic(err)
	}
}

var codecInteger = regexp.MustCompile(`^-?[0-9]+$`)
var codecDecimal = regexp.MustCompile(`^-?[0-9]+(?:\.[0-9]+)?$`)

func codecError(path, message string) error { return fmt.Errorf("wire codec %s: %s", path, message) }
func codecRead(v Value, out any) error      { return json.Unmarshal(v.Value, out) }
func codecValue(kind string, value any) (Value, error) {
	b, e := json.Marshal(value)
	return Value{Kind: kind, Value: b}, e
}
func codecReference(name string) (codecRef, error) {
	if _, ok := codecDeclarations[name]; !ok {
		return codecRef{}, codecError(name, "unknown selected declaration")
	}
	return codecRef{Symbol: &name}, nil
}

// Decode requires structural conformance, never semantic or business validity.
func Decode(name string, value Value) (any, error) {
	r, e := codecReference(name)
	if e != nil {
		return nil, e
	}
	return codecDecode(r, value, "", name, 0, false)
}

// DecodeInput preserves representable invalid shapes for the actual production parser.
func DecodeInput(name string, value Value) (any, error) {
	r, e := codecReference(name)
	if e != nil {
		return nil, e
	}
	return codecDecode(r, value, "", name, 0, true)
}
func codecDecode(r codecRef, v Value, mode, path string, depth int, input bool) (any, error) {
	if depth > 128 {
		return nil, codecError(path, "conversion depth exceeds 128")
	}
	var d codecData
	if r.Symbol != nil {
		var ok bool
		d, ok = codecDeclarations[*r.Symbol]
		if !ok {
			return nil, codecError(path, "unknown declaration")
		}
		if mode == "" {
			mode = codecConfiguration.Types[*r.Symbol]
		}
		if d.Alias != nil {
			return codecDecode(*d.Alias, v, mode, path, depth+1, input)
		}
	}
	kind := ""
	if r.Symbol != nil {
		kind = "record"
		if len(d.Variants) > 0 {
			kind = "variant"
		}
	} else {
		switch r.Name {
		case "Int":
			kind = "integer"
		case "String", "UUID", "Timestamp":
			kind = "string"
		case "Boolean":
			kind = "boolean"
		case "Float", "Duration":
			kind = "float"
		case "Decimal":
			kind = "decimal"
		case "List", "Bytes":
			kind = "list"
		case "Option", "Dctx0Option", "Some", "None":
			kind = "option"
		case "Unit":
			kind = "unit"
		default:
			return nil, codecError(path, "unsupported builtin "+r.Name)
		}
	}
	if !input && v.Kind != kind {
		return nil, codecError(path, "expected "+kind+", received "+v.Kind)
	}
	child := func(ref codecRef, value Value, suffix string) (any, error) {
		return codecDecode(ref, value, "", path+suffix, depth+1, input)
	}
	switch v.Kind {
	case "unit":
		return nil, nil
	case "string":
		var s string
		e := codecRead(v, &s)
		return s, e
	case "boolean":
		var b bool
		e := codecRead(v, &b)
		return b, e
	case "integer":
		var s string
		if e := codecRead(v, &s); e != nil {
			return nil, e
		}
		if len(s) > 65536 || !codecInteger.MatchString(s) {
			return nil, codecError(path, "invalid exact integer text")
		}
		n, ok := new(big.Int).SetString(s, 10)
		if !ok {
			return nil, codecError(path, "invalid integer")
		}
		if mode == "" {
			mode = codecConfiguration.Integer
		}
		if mode == "bigint" {
			return n, nil
		}
		return json.Number(n.String()), nil
	case "decimal":
		var s string
		if e := codecRead(v, &s); e != nil {
			return nil, e
		}
		if len(s) > 65536 || !codecDecimal.MatchString(s) {
			return nil, codecError(path, "invalid exact decimal text")
		}
		return s, nil
	case "float":
		var s string
		if e := codecRead(v, &s); e != nil {
			return nil, e
		}
		n, e := strconv.ParseFloat(s, 64)
		if e != nil || math.IsInf(n, 0) || math.IsNaN(n) {
			return nil, codecError(path, "expected finite Float")
		}
		return n, nil
	case "record":
		var fields map[string]Value
		if e := codecRead(v, &fields); e != nil {
			return nil, e
		}
		if fields == nil {
			return nil, codecError(path, "expected record fields")
		}
		if !input {
			if len(fields) != len(d.Fields) {
				return nil, codecError(path, "record fields differ")
			}
			for _, f := range d.Fields {
				if _, ok := fields[f.Name]; !ok {
					return nil, codecError(path+"."+f.Name, "missing field")
				}
			}
		}
		out := map[string]any{}
		for name, value := range fields {
			ref := codecRef{Name: codecBuiltin(value.Kind)}
			for _, f := range d.Fields {
				if f.Name == name {
					ref = f.Type
					break
				}
			}
			n, e := child(ref, value, "."+name)
			if e != nil {
				return nil, e
			}
			out[name] = n
		}
		return out, nil
	case "list":
		var values []Value
		if e := codecRead(v, &values); e != nil {
			return nil, e
		}
		if values == nil {
			return nil, codecError(path, "expected list")
		}
		out := make([]any, len(values))
		for i, value := range values {
			ref := codecRef{Name: codecBuiltin(value.Kind)}
			if len(r.Arguments) == 1 {
				ref = r.Arguments[0]
			}
			if r.Name == "Bytes" {
				ref = codecRef{Name: "Int"}
			}
			n, e := child(ref, value, fmt.Sprintf("[%d]", i))
			if r.Name == "Bytes" {
				n, e = codecDecode(ref, value, "number", fmt.Sprintf("%s[%d]", path, i), depth+1, input)
			}
			if e != nil {
				return nil, e
			}
			if !input && r.Name == "Bytes" {
				number, ok := n.(json.Number)
				if !ok {
					return nil, codecError(path, "byte representation unavailable")
				}
				b, e := number.Int64()
				if e != nil || b < 0 || b > 255 {
					return nil, codecError(path, "byte outside 0..255")
				}
			}
			out[i] = n
		}
		return out, nil
	case "option":
		if string(v.Value) == "null" {
			if !input && r.Name == "Some" {
				return nil, codecError(path, "expected present option")
			}
			return nil, nil
		}
		if !input && r.Name == "None" {
			return nil, codecError(path, "expected absent option")
		}
		var inner Value
		if e := codecRead(v, &inner); e != nil {
			return nil, e
		}
		ref := codecRef{Name: codecBuiltin(inner.Kind)}
		if len(r.Arguments) == 1 {
			ref = r.Arguments[0]
		}
		n, e := child(ref, inner, ".Some")
		if e != nil {
			return nil, e
		}
		if n == nil {
			return nil, codecError(path, "present option collapses into absence")
		}
		return n, nil
	case "variant":
		if r.Symbol == nil || len(d.Variants) == 0 {
			return nil, codecError(path, "unexpected variant needs an explicit native mapping")
		}
		var tag struct {
			Declaration string `json:"declaration"`
			Name        string `json:"name"`
			Payload     *Value `json:"payload"`
		}
		if e := codecRead(v, &tag); e != nil {
			return nil, e
		}
		if r.Symbol != nil && tag.Declaration != *r.Symbol {
			return nil, codecError(path, "foreign variant declaration")
		}
		var ref *codecRef
		known := false
		payloads := false
		for _, alt := range d.Variants {
			if alt.Payload != nil {
				payloads = true
			}
			if alt.Name == tag.Name {
				known = true
				ref = alt.Payload
			}
		}
		if !input && !known {
			return nil, codecError(path, "unknown alternative "+tag.Name)
		}
		if mode == "" {
			mode = codecConfiguration.Enum
		}
		if payloads {
			mode = "tagged"
		}
		if mode == "string" {
			if tag.Payload != nil {
				return nil, codecError(path, "payload cannot be represented as string enum")
			}
			return tag.Name, nil
		}
		var payload any
		if tag.Payload != nil {
			if ref == nil {
				if !input {
					return nil, codecError(path, "unexpected variant payload")
				}
				ref = &codecRef{Name: codecBuiltin(tag.Payload.Kind)}
			}
			n, e := child(*ref, *tag.Payload, "."+tag.Name)
			if e != nil {
				return nil, e
			}
			payload = n
		} else if ref != nil && !input {
			return nil, codecError(path, "missing variant payload")
		}
		return map[string]any{"kind": tag.Name, "payload": payload}, nil
	default:
		return nil, codecError(path, "unsupported wire kind "+v.Kind)
	}
}
func codecBuiltin(kind string) string {
	switch kind {
	case "integer":
		return "Int"
	case "float":
		return "Float"
	case "decimal":
		return "Decimal"
	case "string":
		return "String"
	case "boolean":
		return "Boolean"
	case "list":
		return "List"
	case "option":
		return "Option"
	case "unit":
		return "Unit"
	case "record", "variant":
		return "String"
	}
	return "Unsupported"
}

// Encode accepts only the documented testing representation, not arbitrary production structs.
func Encode(name string, value any) (Value, error) {
	r, e := codecReference(name)
	if e != nil {
		return Value{}, e
	}
	return codecEncode(r, value, "", name, 0)
}
func codecEncode(r codecRef, n any, mode, path string, depth int) (Value, error) {
	if depth > 128 {
		return Value{}, codecError(path, "conversion depth exceeds 128")
	}
	var d codecData
	if r.Symbol != nil {
		var ok bool
		d, ok = codecDeclarations[*r.Symbol]
		if !ok {
			return Value{}, codecError(path, "unknown declaration")
		}
		if mode == "" {
			mode = codecConfiguration.Types[*r.Symbol]
		}
		if d.Alias != nil {
			return codecEncode(*d.Alias, n, mode, path, depth+1)
		}
	}
	fail := func() (Value, error) {
		return Value{}, codecError(path, "unsupported or incorrect native representation")
	}
	child := func(ref codecRef, value any, suffix string) (Value, error) {
		return codecEncode(ref, value, "", path+suffix, depth+1)
	}
	if r.Symbol != nil {
		if len(d.Variants) > 0 {
			if mode == "" {
				mode = codecConfiguration.Enum
			}
			for _, a := range d.Variants {
				if a.Payload != nil {
					mode = "tagged"
				}
			}
			var name string
			var payload any
			if mode == "string" {
				var ok bool
				name, ok = n.(string)
				if !ok {
					return fail()
				}
			} else {
				tag, ok := n.(map[string]any)
				if !ok || len(tag) != 2 {
					return fail()
				}
				name, _ = tag["kind"].(string)
				payload = tag["payload"]
			}
			for _, a := range d.Variants {
				if a.Name != name {
					continue
				}
				var v *Value
				if a.Payload != nil {
					x, e := child(*a.Payload, payload, "."+name)
					if e != nil {
						return Value{}, e
					}
					v = &x
				} else if payload != nil {
					return fail()
				}
				return codecValue("variant", map[string]any{"declaration": *r.Symbol, "name": name, "payload": v})
			}
			return fail()
		}
		fields, ok := n.(map[string]any)
		if !ok || len(fields) != len(d.Fields) {
			return fail()
		}
		out := map[string]Value{}
		for _, f := range d.Fields {
			v, ok := fields[f.Name]
			if !ok {
				return fail()
			}
			x, e := child(f.Type, v, "."+f.Name)
			if e != nil {
				return Value{}, e
			}
			out[f.Name] = x
		}
		return codecValue("record", out)
	}
	switch r.Name {
	case "Unit":
		if n != nil {
			return fail()
		}
		return Value{Kind: "unit"}, nil
	case "String", "UUID", "Timestamp":
		if s, ok := n.(string); ok {
			return codecValue("string", s)
		}
	case "Boolean":
		if b, ok := n.(bool); ok {
			return codecValue("boolean", b)
		}
	case "Int":
		if mode == "" {
			mode = codecConfiguration.Integer
		}
		var s string
		if mode == "bigint" {
			b, ok := n.(*big.Int)
			if !ok || b == nil {
				return fail()
			}
			s = b.String()
		} else {
			b, ok := n.(json.Number)
			if !ok {
				return fail()
			}
			s = b.String()
		}
		if len(s) <= 65536 && codecInteger.MatchString(s) {
			return codecValue("integer", s)
		}
	case "Decimal":
		if s, ok := n.(string); ok && len(s) <= 65536 && codecDecimal.MatchString(s) {
			return codecValue("decimal", s)
		}
	case "Float", "Duration":
		if f, ok := n.(float64); ok && !math.IsInf(f, 0) && !math.IsNaN(f) {
			return codecValue("float", strconv.FormatFloat(f, 'g', -1, 64))
		}
	case "Option", "Dctx0Option", "Some", "None":
		if n == nil {
			if r.Name == "Some" {
				return fail()
			}
			return codecValue("option", nil)
		}
		if r.Name == "None" || len(r.Arguments) != 1 {
			return fail()
		}
		v, e := child(r.Arguments[0], n, ".Some")
		if e != nil {
			return Value{}, e
		}
		return codecValue("option", v)
	case "List", "Bytes":
		values, ok := n.([]any)
		if !ok {
			return fail()
		}
		ref := codecRef{Name: "Int"}
		if r.Name != "Bytes" {
			if len(r.Arguments) != 1 {
				return fail()
			}
			ref = r.Arguments[0]
		}
		out := make([]Value, len(values))
		for i, x := range values {
			v, e := child(ref, x, fmt.Sprintf("[%d]", i))
			if r.Name == "Bytes" {
				v, e = codecEncode(ref, x, "number", fmt.Sprintf("%s[%d]", path, i), depth+1)
			}
			if e != nil {
				return Value{}, e
			}
			out[i] = v
		}
		v, e := codecValue("list", out)
		if e != nil {
			return v, e
		}
		_, e = codecDecode(r, v, mode, path, depth, false)
		return v, e
	}
	return fail()
}

// ReturnedRecord refuses failures, exceptions and unavailable observations.
func ReturnedRecord(observation Observation, technical error) (map[string]Value, error) {
	if technical != nil {
		return nil, fmt.Errorf("expected returned record; technical failure (%T)", technical)
	}
	if observation.Kind != "returned" || observation.Value == nil || observation.Value.Kind != "record" {
		return nil, fmt.Errorf("expected returned record; received %s", codecOutcome(observation))
	}
	var fields map[string]Value
	if e := codecRead(*observation.Value, &fields); e != nil {
		return nil, e
	}
	if fields == nil {
		return nil, fmt.Errorf("missing record fields")
	}
	return fields, nil
}

type DeclaredFailureValue struct {
	Declaration string `json:"declaration"`
	Payload     Value  `json:"payload"`
}

func DeclaredFailure(observation Observation, technical error, declaration string) (DeclaredFailureValue, error) {
	var f DeclaredFailureValue
	if technical != nil {
		return f, fmt.Errorf("expected declared failure %s; technical failure (%T)", declaration, technical)
	}
	if observation.Kind != "returned" || observation.Value == nil || observation.Value.Kind != "failure" {
		return f, fmt.Errorf("expected declared failure %s; received %s", declaration, codecOutcome(observation))
	}
	if e := codecRead(*observation.Value, &f); e != nil {
		return f, e
	}
	if f.Declaration != declaration {
		return f, fmt.Errorf("expected declared failure %s; received %s", declaration, f.Declaration)
	}
	if f.Payload.Kind == "" {
		d, ok := codecDeclarations[declaration]
		if !ok || d.Alias != nil || len(d.Fields) != 0 || len(d.Variants) != 0 {
			return f, fmt.Errorf("missing failure payload for %s", declaration)
		}
	} else if _, e := Decode(declaration, f.Payload); e != nil {
		return f, e
	}
	return f, nil
}
func StringField(record Value, name, expected string) error {
	if record.Kind != "record" {
		return fmt.Errorf("field %s requires a record, received %s", name, record.Kind)
	}
	var fields map[string]Value
	if e := codecRead(record, &fields); e != nil {
		return e
	}
	v, ok := fields[name]
	if !ok || v.Kind != "string" {
		return fmt.Errorf("expected string field %s", name)
	}
	var s string
	if e := codecRead(v, &s); e != nil {
		return e
	}
	if s != expected {
		return fmt.Errorf("string field %s differs from expected value", name)
	}
	return nil
}

// TechnicalFailure checks explicit attribution, never error message text.
func TechnicalFailure(technical error, category string) error {
	var failure *TechnicalError
	if !errors.As(technical, &failure) || failure.Category != category {
		return fmt.Errorf("expected technical failure category %s", category)
	}
	return nil
}
func UnavailableObservation(observation Observation, technical error) error {
	if technical != nil {
		return fmt.Errorf("expected unavailable observation; received technical failure (%T)", technical)
	}
	if observation.Kind != "unavailable" || observation.Reason == nil || *observation.Reason == "" {
		return fmt.Errorf("expected unavailable observation with a reason")
	}
	return nil
}

// RecordedCalls requires an explicitly complete trace. Observed empty is distinct from absent.
func RecordedCalls(calls []InvocationObservation, available bool, invocation Invocation, count int) error {
	if !available {
		return fmt.Errorf("actual collaborator observations unavailable")
	}
	if count < 0 {
		return fmt.Errorf("negative call count")
	}
	expected, e := json.Marshal(invocation)
	if e != nil {
		return e
	}
	found := 0
	for _, call := range calls {
		actual, e := json.Marshal(call.Invocation)
		if e != nil {
			return e
		}
		if string(actual) == string(expected) {
			found++
		}
	}
	if found != count {
		return fmt.Errorf("expected %d actual calls to %s; received %d", count, invocation.Subject, found)
	}
	return nil
}

func codecOutcome(observation Observation) string {
	if observation.Value != nil {
		return observation.Kind + "/" + observation.Value.Kind
	}
	return observation.Kind + "/missing value"
}
