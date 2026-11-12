package applicationtests

import (
	"crypto/sha256"
	"errors"
	"fmt"
	"testing"
)

// Decoder is the actual production decoder/constructor. Its input is never
// parsed by the test library; invalid bytes reach production unchanged.
type Decoder func(raw []byte, ty TypeRef) (any, error)

// DecoderRejection marks an actual production contract/parse rejection. Other
// errors remain technical exceptions and cannot satisfy a negative contract case.
type DecoderRejection struct{ Reason string }

func (rejection *DecoderRejection) Error() string { return rejection.Reason }
func Reject(reason string) error                  { return &DecoderRejection{Reason: reason} }

// Projector converts the actual decoded value, after the production call.
// Projector failures are binding errors, never production rejections.
type Projector func(actual any, ty TypeRef) (Value, error)

type decoderOutcome struct {
	Kind    string  `json:"kind"`
	Value   *Value  `json:"value,omitempty"`
	Reason  *string `json:"reason,omitempty"`
	Message *string `json:"message,omitempty"`
}

func productionDecode(decode Decoder, raw []byte, ty TypeRef) (actual any, outcome *decoderOutcome) {
	defer func() {
		if recovered := recover(); recovered != nil {
			outcome = &decoderOutcome{Kind: "exception", Message: messagePointer(fmt.Sprint(recovered))}
		}
	}()
	value, err := decode(raw, ty)
	if err != nil {
		var rejection *DecoderRejection
		if errors.As(err, &rejection) {
			return nil, &decoderOutcome{Kind: "rejected", Reason: messagePointer(rejection.Reason)}
		}
		return nil, &decoderOutcome{Kind: "exception", Message: messagePointer(err.Error())}
	}
	return value, nil
}

func projectDecoded(project Projector, actual any, ty TypeRef) (outcome decoderOutcome) {
	defer func() {
		if recovered := recover(); recovered != nil {
			outcome = decoderOutcome{Kind: "binding-error", Message: messagePointer(fmt.Sprint(recovered))}
		}
	}()
	value, err := project(actual, ty)
	if err != nil {
		return decoderOutcome{Kind: "binding-error", Message: messagePointer(err.Error())}
	}
	return decoderOutcome{Kind: "accepted", Value: &value}
}

func messagePointer(message string) *string { return &message }

func verdictError(t *testing.T, result *Verdict, caseID string) {
	t.Helper()
	if result == nil {
		t.Errorf("%s: central assessment omitted its verdict", caseID)
		return
	}
	if result.Status != "PASS" {
		if result.Issue != nil {
			t.Errorf("%s: central %s %s: %s", caseID, result.Status, result.Issue.Code, result.Issue.Message)
		} else {
			t.Errorf("%s: central %s", caseID, result.Status)
		}
	}
}

// DecoderTest owns registration, centrally selected iteration, native subtests
// and case finalization. Catching a decoder failure cannot clear the evaluator.
func (client *Client) DecoderTest(t *testing.T, decode Decoder, project Projector) {
	t.Helper()
	if decode == nil || project == nil {
		t.Fatal("production decoder and result projector are required")
	}
	test := t.Name()
	registered, err := client.exchange(map[string]any{"kind": "register", "test": test})
	if err != nil || registered.Kind != "registered" {
		t.Fatalf("register decoder test: %v", err)
	}
	defer func() {
		closed, err := client.exchange(map[string]any{"kind": "close-test", "test": test})
		if err != nil || closed.Kind != "test-closed" {
			t.Errorf("finalize decoder test: %v", err)
		}
	}()
	for {
		next, err := client.exchange(map[string]any{"kind": "next", "test": test})
		if err != nil {
			t.Fatal(err)
		}
		if next.Kind == "no-more-cases" {
			return
		}
		if next.Kind != "case" || next.Form != "decoder" || next.Context == 0 || next.Directive == nil {
			t.Fatal("unexpected decoder case directive")
		}
		name := fmt.Sprintf("case_%x", sha256.Sum256([]byte(next.ID)))
		t.Run(name, func(t *testing.T) {
			defer func() {
				closed, err := client.exchange(map[string]any{"kind": "close-case", "context": next.Context, "cleanup_error": nil})
				if err != nil || closed.Kind != "case-closed" {
					t.Errorf("close decoder case: %v", err)
					return
				}
				verdictError(t, closed.Result, next.ID)
			}()
			if next.Directive.Kind == "verdict" {
				verdictError(t, next.Directive.Result, next.ID)
				return
			}
			if next.Directive.Kind != "decode" {
				t.Fatal("unsupported decoder action")
			}
			raw := make([]byte, len(next.Directive.Raw))
			for i, octet := range next.Directive.Raw {
				if octet < 0 || octet > 255 {
					t.Fatal("invalid raw decoder octet")
				}
				raw[i] = byte(octet)
			}
			actual, rejected := productionDecode(decode, raw, next.Directive.Type)
			var outcome decoderOutcome
			if rejected != nil {
				outcome = *rejected
			} else {
				outcome = projectDecoded(project, actual, next.Directive.Type)
			}
			// Record exactly the bytes supplied to production, including mutations.
			octets := make([]int, len(raw))
			for i, octet := range raw {
				octets[i] = int(octet)
			}
			observed, err := client.exchange(map[string]any{"kind": "observe", "context": next.Context,
				"observation": map[string]any{"kind": "decoded", "raw": octets, "outcome": outcome}})
			if err != nil {
				t.Fatal(err)
			}
			if observed.Kind != "action" || observed.Context != next.Context || observed.Directive == nil || observed.Directive.Kind != "verdict" {
				t.Fatal("missing immediate decoder verdict")
			}
			verdictError(t, observed.Directive.Result, next.ID)
		})
	}
}
