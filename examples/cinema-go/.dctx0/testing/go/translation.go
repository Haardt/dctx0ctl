// Testing-only conversion. Model metadata describes types, never expected results.
package main

import (
	"cinema/domain"
	"encoding/json"
	"fmt"
	"math"
	"math/big"
	"os"
	"strconv"
)

const root = "HomeCinema.MovieNight."

type typeRef struct {
	Name      string    `json:"name"`
	Symbol    *string   `json:"symbol_id"`
	Arguments []typeRef `json:"arguments"`
}
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
type operation struct {
	Parameters []field `json:"parameters"`
	Result     typeRef `json:"result"`
}
type model struct {
	Data       map[string]declaration `json:"data"`
	Operations map[string]operation   `json:"operations"`
}

var schema model

func loadModel() error {
	bytes, err := os.ReadFile("model.json")
	if err != nil {
		return err
	}
	return json.Unmarshal(bytes, &schema)
}
func value(kind string, v any) Value { b, _ := json.Marshal(v); return Value{Kind: kind, Value: b} }
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
func encode(v any, ty typeRef) (Value, error) {
	if ty.Symbol != nil {
		d, ok := schema.Data[*ty.Symbol]
		if !ok {
			return Value{}, fmt.Errorf("Missing type descriptor")
		}
		if d.Alias != nil {
			return encode(v, *d.Alias)
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
			x, err := encode(record[f.Name], f.Type)
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
		x, err := encode(v, ty.Arguments[0])
		return value("option", x), err
	case "List":
		items, ok := v.([]any)
		if !ok {
			return Value{}, fmt.Errorf("Expected list")
		}
		out := make([]Value, len(items))
		for i, item := range items {
			x, err := encode(item, ty.Arguments[0])
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
func named(name string) typeRef { s := root + name; return typeRef{Name: name, Symbol: &s} }
