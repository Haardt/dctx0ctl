package applicationtests

import (
	"crypto/sha256"
	"errors"
	"flag"
	"fmt"
	"strings"
	"testing"
	"time"
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
	client.decoderTest(t, decode, project, false)
}

// StatelessDecoderTest can use packets when this exact registration is bound
// with --stateless-decoder. Both callbacks must depend only on their arguments
// and immutable data, with no effects or lifecycle hooks across cases.
func (client *Client) StatelessDecoderTest(t *testing.T, decode Decoder, project Projector) {
	t.Helper()
	client.decoderTest(t, decode, project, true)
}

func (client *Client) decoderTest(t *testing.T, decode Decoder, project Projector, stateless bool) {
	t.Helper()
	if decode == nil || project == nil {
		t.Fatal("production decoder and result projector are required")
	}
	test := t.Name()
	kind, expected := "register", "registered"
	if stateless {
		kind, expected = "register-decoder", "decoder-registered"
	}
	registered, err := client.exchange(map[string]any{"kind": kind, "test": test})
	if err != nil || registered.Kind != expected {
		t.Fatalf("register decoder test: %v", err)
	}
	defer func() {
		closed, err := client.exchange(map[string]any{"kind": "close-test", "test": test})
		if err != nil || closed.Kind != "test-closed" {
			t.Errorf("finalize decoder test: %v", err)
		}
	}()
	// Subtest filters must be applied by testing.T before any native calls.
	batch := stateless && registered.Batch && !decoderSubtestsFiltered()
	limit := min(client.limit, registered.FrameLimit)
	if batch && (limit <= 96 || registered.CaseTimeoutSeconds <= 0 || registered.CaseTimeoutSeconds > 86_400) {
		t.Fatal("invalid decoder packet budgets")
	}
	caseTimeout := time.Duration(registered.CaseTimeoutSeconds) * time.Second
	for {
		request := map[string]any{"kind": "next", "test": test}
		if batch {
			request["kind"] = "next-decoder-batch"
			request["max_message_bytes"] = limit
		}
		next, err := client.exchange(request)
		if err != nil {
			t.Fatal(err)
		}
		switch next.Kind {
		case "no-more-cases":
			return
		case "decoder-batch":
			if !batch {
				t.Fatal("unexpected decoder packet")
			}
			client.runDecoderBatch(t, next.Cases, limit, caseTimeout, decode, project)
		case "case":
			if next.Form != "decoder" || next.Context == 0 || next.Directive == nil {
				t.Fatal("unexpected decoder case directive")
			}
			client.runDecoderCase(t, decoderCase{Context: next.Context, ID: next.ID, Directive: next.Directive}, decode, project)
		default:
			t.Fatal("unexpected decoder reply")
		}
	}
}

func decoderSubtestsFiltered() bool {
	run, skip := flag.Lookup("test.run"), flag.Lookup("test.skip")
	return (run != nil && strings.Contains(run.Value.String(), "/")) ||
		(skip != nil && strings.Contains(skip.Value.String(), "/"))
}

type decodedObservation struct {
	Kind    string         `json:"kind"`
	Raw     []int          `json:"raw"`
	Outcome decoderOutcome `json:"outcome"`
}

func decodeObservation(t *testing.T, action *directive, decode Decoder, project Projector) *decodedObservation {
	t.Helper()
	if action.Kind != "decode" {
		t.Fatal("unsupported decoder action")
	}
	raw := make([]byte, len(action.Raw))
	for i, octet := range action.Raw {
		if octet < 0 || octet > 255 {
			t.Fatal("invalid raw decoder octet")
		}
		raw[i] = byte(octet)
	}
	actual, rejected := productionDecode(decode, raw, action.Type)
	var outcome decoderOutcome
	if rejected != nil {
		outcome = *rejected
	} else {
		outcome = projectDecoded(project, actual, action.Type)
	}
	// Record the bytes after the native call so mutations remain observable.
	octets := make([]int, len(raw))
	for i, octet := range raw {
		octets[i] = int(octet)
	}
	return &decodedObservation{Kind: "decoded", Raw: octets, Outcome: outcome}
}

func (entry decoderCase) name() string {
	return fmt.Sprintf("case_%x", sha256.Sum256([]byte(entry.ID)))
}

func (client *Client) runDecoderCase(t *testing.T, entry decoderCase, decode Decoder, project Projector) {
	t.Helper()
	t.Run(entry.name(), func(t *testing.T) {
		defer client.closeDecoderCase(t, entry)
		if entry.Directive.Kind == "verdict" {
			verdictError(t, entry.Directive.Result, entry.ID)
			return
		}
		client.observeDecoderCase(t, entry, decodeObservation(t, entry.Directive, decode, project), client.limit)
	})
}

func (client *Client) closeDecoderCase(t *testing.T, entry decoderCase) {
	t.Helper()
	closed, err := client.exchange(map[string]any{"kind": "close-case", "context": entry.Context, "cleanup_error": nil})
	if err != nil || closed.Kind != "case-closed" || closed.Context != entry.Context {
		t.Errorf("close decoder case: %v", err)
		return
	}
	verdictError(t, closed.Result, entry.ID)
}

func (client *Client) observeDecoderCase(t *testing.T, entry decoderCase, observation *decodedObservation, limit int) {
	t.Helper()
	observed, err := client.exchangeWithin(map[string]any{"kind": "observe", "context": entry.Context, "observation": observation}, limit)
	if err != nil {
		t.Fatal(err)
	}
	if observed.Kind != "action" || observed.Context != entry.Context || observed.Directive == nil || observed.Directive.Kind != "verdict" {
		t.Fatal("missing immediate decoder verdict")
	}
	verdictError(t, observed.Directive.Result, entry.ID)
}

type decoderBatchObservation struct {
	Context     uint64              `json:"context"`
	Observation *decodedObservation `json:"observation"`
}

func (client *Client) runDecoderBatch(t *testing.T, payload []byte, limit int, timeout time.Duration, decode Decoder, project Projector) {
	t.Helper()
	var cases []decoderCase
	if err := decodeStrict(payload, &cases); err != nil || len(cases) == 0 || len(cases) > 16 {
		t.Fatalf("invalid decoder packet: %v", err)
	}
	observations := make([]decoderBatchObservation, len(cases))
	for i, entry := range cases {
		if entry.Context == 0 || entry.ID == "" || entry.Directive == nil || (i > 0 && cases[i-1].Context >= entry.Context) {
			t.Fatal("invalid decoder packet entry")
		}
		observations[i].Context = entry.Context
		if entry.Directive.Kind != "verdict" {
			observations[i].Observation = client.timedDecoderObservation(t, entry, timeout, decode, project)
		}
	}
	client.submitDecoderBatch(t, cases, observations, limit)
}

// On timeout the channel shuts down immediately even if native code hangs.
func (client *Client) timedDecoderObservation(t *testing.T, entry decoderCase, timeout time.Duration, decode Decoder, project Projector) *decodedObservation {
	t.Helper()
	timer := time.AfterFunc(timeout, func() { _ = client.Close() })
	defer timer.Stop()
	observation := decodeObservation(t, entry.Directive, decode, project)
	if !timer.Stop() {
		t.Fatalf("%s: decoder case deadline exceeded", entry.ID)
	}
	return observation
}

func (client *Client) submitDecoderBatch(t *testing.T, cases []decoderCase, observations []decoderBatchObservation, limit int) {
	t.Helper()
	closed, err := client.exchangeWithin(map[string]any{
		"kind": "complete-decoder-batch", "test": t.Name(), "cases": observations,
	}, limit)
	if errors.Is(err, errRequestTooLarge) {
		if len(cases) == 1 {
			entry := cases[0]
			t.Run(entry.name(), func(t *testing.T) {
				defer client.closeDecoderCase(t, entry)
				if observation := observations[0].Observation; observation != nil {
					client.observeDecoderCase(t, entry, observation, limit)
				}
			})
			return
		}
		mid := len(cases) / 2
		client.submitDecoderBatch(t, cases[:mid], observations[:mid], limit)
		client.submitDecoderBatch(t, cases[mid:], observations[mid:], limit)
		return
	}
	if err != nil || closed.Kind != "decoder-batch-closed" {
		t.Fatalf("close decoder packet: %v", err)
	}
	var verdicts []decoderCaseVerdict
	if err := decodeStrict(closed.Cases, &verdicts); err != nil || len(verdicts) != len(cases) {
		t.Fatalf("invalid decoder packet verdicts: %v", err)
	}
	for i, verdict := range verdicts {
		if verdict.Context != cases[i].Context {
			t.Fatal("unexpected decoder packet verdict context")
		}
		entry := cases[i]
		t.Run(entry.name(), func(t *testing.T) {
			switch verdict.Status {
			case "PASS":
			case "FAIL", "UNKNOWN":
				t.Errorf("%s: central %s (see assessment report)", entry.ID, verdict.Status)
			default:
				t.Fatal("invalid decoder packet verdict status")
			}
		})
	}
}
