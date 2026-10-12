package domain

import (
	"math/big"
	"testing"
)

func TestIndependentScalarBoundariesAndWinner(t *testing.T) {
	for _, v := range []int64{1888, 2026, 2100} {
		if err := Validate("ReleaseYear", big.NewInt(v)); err != nil {
			t.Fatal(err)
		}
	}
	for _, v := range []any{big.NewInt(1887), big.NewInt(2101), 1.5, "2026"} {
		if Validate("ReleaseYear", v) == nil {
			t.Fatalf("accepted %v", v)
		}
	}
	if Validate("FilmId", "00000000-0000-0000-0000-000000000000") != nil {
		t.Fatal("nil UUID is structurally valid")
	}
	if Validate("CommentText", "  ") == nil {
		t.Fatal("blank comment accepted")
	}
	if Validate("FilmId", "bad") == nil {
		t.Fatal("bad UUID accepted")
	}
	if Validate("CommentText", "\u0085") != nil || Validate("CommentText", "\ufeff") == nil {
		t.Fatal("contract trim boundary")
	}
	a, b := "a", "b"
	if CandidateOrderWinner([]any{}, []any{a, b}) != nil {
		t.Fatal("empty votes must have no winner")
	}
	if CandidateOrderWinner([]any{b, a}, []any{a, b}) != a {
		t.Fatal("candidate tie order")
	}
	if CandidateOrderWinner([]any{"unknown"}, []any{a}) != nil {
		t.Fatal("unknown vote affected winner")
	}
}
func TestExactJSONAndMalformedInputs(t *testing.T) {
	v, err := DecodeJSON([]byte(`{"total":9007199254740993,"averageStars":0}`))
	if err != nil {
		t.Fatal(err)
	}
	r := v.(Record)
	if r["total"].(*big.Int).String() != "9007199254740993" {
		t.Fatal("integer loss")
	}
	if _, ok := r["averageStars"].(float64); !ok {
		t.Fatal("float field representation")
	}
	for _, bad := range []string{`{`, `{} {}`, `[1,]`} {
		if _, err := DecodeJSON([]byte(bad)); err == nil {
			t.Fatal("invalid JSON accepted")
		}
	}
}
