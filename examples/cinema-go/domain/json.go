package domain

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"math/big"
	"strconv"
	"strings"
)

// DecodeJSON retains integer digits and uses finite floats only for float fields.
func DecodeJSON(data []byte) (any, error) {
	d := json.NewDecoder(bytes.NewReader(data))
	d.UseNumber()
	var raw any
	if err := d.Decode(&raw); err != nil {
		return nil, err
	}
	var extra any
	if err := d.Decode(&extra); err != io.EOF {
		return nil, fmt.Errorf("Trailing JSON")
	}
	return normalizeJSON(raw, "")
}
func normalizeJSON(value any, key string) (any, error) {
	switch v := value.(type) {
	case json.Number:
		if key == "averageStars" || strings.ContainsAny(string(v), ".eE") {
			return strconv.ParseFloat(string(v), 64)
		}
		n, ok := new(big.Int).SetString(string(v), 10)
		if !ok {
			return nil, fmt.Errorf("Invalid integer")
		}
		return n, nil
	case map[string]any:
		out := Record{}
		for k, item := range v {
			x, err := normalizeJSON(item, k)
			if err != nil {
				return nil, err
			}
			out[k] = x
		}
		return out, nil
	case []any:
		out := make([]any, len(v))
		for i, item := range v {
			x, err := normalizeJSON(item, "")
			if err != nil {
				return nil, err
			}
			out[i] = x
		}
		return out, nil
	default:
		return v, nil
	}
}
