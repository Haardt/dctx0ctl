// Package applicationtests connects native application tests to the central Rust evaluator.
// Production packages must not import this testing-only module.
package applicationtests

import "encoding/json"

const ProtocolVersion uint32 = 2
const LibraryVersion = "0.2.0"
const DefaultFrameBytes = 1_048_576
const maxFrameBytes = 67_108_864
const maxRequestID uint64 = 9_007_199_254_740_991

// Value preserves exact numeric text and the core's tagged value representation.
type Value struct {
	Kind  string          `json:"kind"`
	Value json.RawMessage `json:"value,omitempty"`
}

type TypeRef struct {
	Name      string    `json:"name"`
	Symbol    *string   `json:"symbol_id"`
	Arguments []TypeRef `json:"arguments"`
}

type Issue struct {
	Code    string          `json:"code"`
	Message string          `json:"message"`
	Span    json.RawMessage `json:"span"`
}

type Verdict struct {
	Status string `json:"status"`
	Issue  *Issue `json:"issue,omitempty"`
}

type Summary struct {
	AssessmentVersion string  `json:"assessment_version"`
	Group             string  `json:"group"`
	Finalized         bool    `json:"finalized"`
	RequiredCases     int     `json:"required_cases"`
	CompletedCases    int     `json:"completed_cases"`
	MissingCases      int     `json:"missing_cases"`
	MissingTests      int     `json:"missing_tests"`
	Issues            int     `json:"issues"`
	Status            string  `json:"status"`
	IssueCode         *string `json:"issue_code"`
}

type directive struct {
	Invocation *Invocation `json:"invocation,omitempty"`
	Phase      string      `json:"phase,omitempty"`
	Entities   []string    `json:"entities,omitempty"`
	Kind       string      `json:"kind"`
	Subject    string      `json:"subject,omitempty"`
	Type       TypeRef     `json:"ty,omitempty"`
	Raw        []int       `json:"raw,omitempty"`
	Result     *Verdict    `json:"result,omitempty"`
}

type reply struct {
	Kind        string       `json:"kind"`
	Mocks       []string     `json:"mocks,omitempty"`
	Observation *Observation `json:"observation,omitempty"`
	Context     uint64       `json:"context,omitempty"`
	ID          string       `json:"id,omitempty"`
	Form        string       `json:"form,omitempty"`
	Directive   *directive   `json:"directive,omitempty"`
	Result      *Verdict     `json:"result,omitempty"`
	Summary     *Summary     `json:"summary,omitempty"`
	Issue       *Issue       `json:"issue,omitempty"`
}

type frame struct {
	ProtocolVersion uint32          `json:"protocol_version"`
	ID              uint64          `json:"id"`
	Payload         json.RawMessage `json:"payload"`
}
