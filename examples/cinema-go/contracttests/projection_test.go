// Testing-only projection shared in shape with the legacy project binding.
// Generated metadata supplies types; it never supplies expected results.
package contracttests

import (
	"cinema/domain"
	"encoding/json"
	"fmt"
	app "github.com/Haardt/dctx0ctl/applicationtests/go"
	"math"
	"math/big"
	"strconv"
	"testing"
)

type Value = app.Value
type typeRef = app.TypeRef
type field struct {
	Name string  `json:"name"`
	Type typeRef `json:"ty"`
}
type declaration struct {
	Alias    *typeRef `json:"alias"`
	Fields   []field  `json:"fields"`
	Variants []struct {
		Name string `json:"name"`
	} `json:"variants"`
}

type valueCodec struct {
	ApplicationRoot string                         `json:"-"`
	Data            map[string]declaration         `json:"data"`
	Operations      map[string]operationDescriptor `json:"operations"`
}

func value(kind string, v any) Value { b, _ := json.Marshal(v); return Value{Kind: kind, Value: b} }
func (codec *valueCodec) encode(v any, ty typeRef) (Value, error) {
	if ty.Symbol != nil {
		d, ok := codec.Data[*ty.Symbol]
		if !ok {
			return Value{}, fmt.Errorf("Missing type descriptor")
		}
		if d.Alias != nil {
			return codec.encode(v, *d.Alias)
		}
		if len(d.Variants) > 0 {
			s, ok := v.(string)
			if !ok {
				return Value{}, fmt.Errorf("Expected variant")
			}
			return value("variant", map[string]any{"declaration": *ty.Symbol, "name": s, "payload": nil}), nil
		}
		record, ok := v.(domain.Record)
		if !ok || len(record) != len(d.Fields) {
			return Value{}, fmt.Errorf("Expected exact application record")
		}
		fields := map[string]Value{}
		for _, f := range d.Fields {
			x, err := codec.encode(record[f.Name], f.Type)
			if err != nil {
				return Value{}, fmt.Errorf("%s: %w", f.Name, err)
			}
			fields[f.Name] = x
		}
		return value("record", fields), nil
	}
	switch ty.Name {
	case "Option":
		if v == nil {
			return value("option", nil), nil
		}
		x, err := codec.encode(v, ty.Arguments[0])
		return value("option", x), err
	case "List":
		items, ok := v.([]any)
		if !ok {
			return Value{}, fmt.Errorf("Expected list")
		}
		out := make([]Value, len(items))
		for i, item := range items {
			x, err := codec.encode(item, ty.Arguments[0])
			if err != nil {
				return Value{}, err
			}
			out[i] = x
		}
		return value("list", out), nil
	case "Int":
		n, err := domain.ExactInteger(v)
		if err != nil {
			return Value{}, err
		}
		return value("integer", n.String()), nil
	case "Float":
		f, ok := v.(float64)
		if !ok || math.IsNaN(f) || math.IsInf(f, 0) {
			return Value{}, fmt.Errorf("Expected finite float")
		}
		return value("float", strconv.FormatFloat(f, 'g', -1, 64)), nil
	case "String", "UUID", "Timestamp":
		s, ok := v.(string)
		if !ok {
			return Value{}, fmt.Errorf("Expected text")
		}
		return value("string", s), nil
	case "Boolean":
		b, ok := v.(bool)
		if !ok {
			return Value{}, fmt.Errorf("Expected boolean")
		}
		return value("boolean", b), nil
	case "Unit":
		if v != nil {
			return Value{}, fmt.Errorf("Expected void")
		}
		return Value{Kind: "unit"}, nil
	}
	return Value{}, fmt.Errorf("Unsupported type %s", ty.Name)
}

const root = "HomeCinema.MovieNight."

func decode(v Value) (any, error) {
	switch v.Kind {
	case "unit":
		return nil, nil
	case "integer":
		var s string
		if err := json.Unmarshal(v.Value, &s); err != nil {
			return nil, err
		}
		n, ok := new(big.Int).SetString(s, 10)
		if !ok {
			return nil, fmt.Errorf("Invalid integer")
		}
		return n, nil
	case "float":
		var s string
		if err := json.Unmarshal(v.Value, &s); err != nil {
			return nil, err
		}
		n, err := strconv.ParseFloat(s, 64)
		if err != nil || math.IsInf(n, 0) || math.IsNaN(n) {
			return nil, fmt.Errorf("Unrepresentable float")
		}
		return n, nil
	case "string":
		var s string
		err := json.Unmarshal(v.Value, &s)
		return s, err
	case "boolean":
		var b bool
		err := json.Unmarshal(v.Value, &b)
		return b, err
	case "option":
		if string(v.Value) == "null" {
			return nil, nil
		}
		var item Value
		if err := json.Unmarshal(v.Value, &item); err != nil {
			return nil, err
		}
		return decode(item)
	case "list":
		var items []Value
		if err := json.Unmarshal(v.Value, &items); err != nil {
			return nil, err
		}
		out := make([]any, len(items))
		for i, item := range items {
			x, err := decode(item)
			if err != nil {
				return nil, err
			}
			out[i] = x
		}
		return out, nil
	case "record":
		var fields map[string]Value
		if err := json.Unmarshal(v.Value, &fields); err != nil {
			return nil, err
		}
		out := domain.Record{}
		for k, item := range fields {
			x, err := decode(item)
			if err != nil {
				return nil, err
			}
			out[k] = x
		}
		return out, nil
	case "variant":
		var variant struct {
			Name    string `json:"name"`
			Payload *Value `json:"payload"`
		}
		if err := json.Unmarshal(v.Value, &variant); err != nil {
			return nil, err
		}
		if variant.Payload != nil {
			return nil, fmt.Errorf("Unsupported payload variant")
		}
		return variant.Name, nil
	case "failure":
		var failure struct {
			Declaration string `json:"declaration"`
			Payload     *Value `json:"payload"`
		}
		if err := json.Unmarshal(v.Value, &failure); err != nil {
			return nil, err
		}
		payload := domain.Record{}
		if failure.Payload != nil {
			x, err := decode(*failure.Payload)
			if err != nil {
				return nil, err
			}
			payload = x.(domain.Record)
		}
		return nil, domain.NewFailure(failure.Declaration[len(root):], payload)
	default:
		return nil, fmt.Errorf("Unsupported wire value %s", v.Kind)
	}
}

// Technical boundary probes are additional tests, not extra semantic manifest cases.
func TestCinemaRawDecoderPreservesIntegerDigitsAndRejectsMalformedBytes(t *testing.T) {
	for _, raw := range [][]byte{{0xff, 0}, []byte(`{"missing":`), []byte(`1 2`)} {
		if _, err := domain.DecodeJSON(raw); err == nil {
			t.Fatalf("production decoder accepted malformed bytes: %q", raw)
		}
	}
	const digits = "90071992547409931234567890"
	actual, err := domain.DecodeJSON([]byte(digits))
	if err != nil {
		t.Fatal(err)
	}
	number, ok := actual.(*big.Int)
	if !ok || number.String() != digits {
		t.Fatal("production decoder changed integer digits")
	}
}

// Shape metadata never supplies a replacement input value. Only an identical
// observed Float may retain its original textual notation, without f64 integers.
func witnessInput(actual, expected Value) (Value, error) {
	if actual.Kind != expected.Kind {
		return actual, nil
	}
	switch actual.Kind {
	case "float":
		var a, b string
		if err := json.Unmarshal(actual.Value, &a); err != nil {
			return Value{}, err
		}
		if err := json.Unmarshal(expected.Value, &b); err != nil {
			return Value{}, err
		}
		observed, err := strconv.ParseFloat(a, 64)
		if err != nil {
			return Value{}, err
		}
		original, err := strconv.ParseFloat(b, 64)
		if err != nil {
			return Value{}, err
		}
		if math.IsInf(observed, 0) || math.IsNaN(observed) || math.IsInf(original, 0) || math.IsNaN(original) {
			return Value{}, fmt.Errorf("unwitnessable float input")
		}
		if math.Float64bits(observed) == math.Float64bits(original) {
			return expected, nil
		}
	case "record":
		var a, b map[string]Value
		if err := json.Unmarshal(actual.Value, &a); err != nil {
			return Value{}, err
		}
		if err := json.Unmarshal(expected.Value, &b); err != nil {
			return Value{}, err
		}
		for key, observed := range a {
			if original, ok := b[key]; ok {
				v, err := witnessInput(observed, original)
				if err != nil {
					return Value{}, err
				}
				a[key] = v
			}
		}
		return value("record", a), nil
	case "list":
		var a, b []Value
		if err := json.Unmarshal(actual.Value, &a); err != nil {
			return Value{}, err
		}
		if err := json.Unmarshal(expected.Value, &b); err != nil {
			return Value{}, err
		}
		for i, observed := range a {
			if i < len(b) {
				v, err := witnessInput(observed, b[i])
				if err != nil {
					return Value{}, err
				}
				a[i] = v
			}
		}
		return value("list", a), nil
	case "option":
		if string(actual.Value) == "null" || string(expected.Value) == "null" {
			return actual, nil
		}
		var a, b Value
		if err := json.Unmarshal(actual.Value, &a); err != nil {
			return Value{}, err
		}
		if err := json.Unmarshal(expected.Value, &b); err != nil {
			return Value{}, err
		}
		v, err := witnessInput(a, b)
		return value("option", v), err
	}
	return actual, nil
}
