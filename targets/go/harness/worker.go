// Generated testing worker. Production code imports none of these declarations.
package main

import (
	"bufio"
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"
	"sync"
	"unicode/utf8"
)

const protocolVersion = 1
const maxMessageBytes = 67108864
const maxErrorMessageRunes = 4096
const maxRequestID = 9007199254740991

var subjects = map[string]bool{__SUBJECTS__}

// Incoming wire shapes.

type requestEnvelope struct {
	ProtocolVersion int             `json:"protocol_version"`
	ID              json.RawMessage `json:"id"`
	Request         json.RawMessage `json:"request"`
}

type metricsRequest struct {
	Kind string `json:"kind"`
}

type describeRequest struct {
	Kind string `json:"kind"`
}

type resolveDependencyRequest struct {
	Kind      string `json:"kind"`
	Mode      string `json:"mode"`
	Specifier string `json:"specifier"`
	Importer  string `json:"importer"`
}

type dependencyResolvedReply struct {
	Kind string `json:"kind"`
	Path string `json:"path"`
}
type beginRequest struct {
	Kind   string `json:"kind"`
	CaseID string `json:"case_id"`
}

type invokeRequest struct {
	Kind       string     `json:"kind"`
	Invocation Invocation `json:"invocation"`
}

type validateRequest struct {
	Kind    string `json:"kind"`
	Subject string `json:"subject"`
	Value   Value  `json:"value"`
}

type observeRequest struct {
	Kind     string   `json:"kind"`
	Entities []string `json:"entities"`
}

type endRequest struct {
	Kind string `json:"kind"`
}

type shutdownRequest struct {
	Kind string `json:"kind"`
}

type mockReplyRequest struct {
	Kind        string      `json:"kind"`
	RequestID   uint64      `json:"request_id"`
	Observation Observation `json:"observation"`
}

// Outgoing wire shapes.

type messageEnvelope struct {
	ProtocolVersion int `json:"protocol_version"`
	Message         any `json:"message"`
}

type replyMessage struct {
	Kind  string `json:"kind"`
	ID    uint64 `json:"id"`
	Reply any    `json:"reply"`
}

type mockCallMessage struct {
	Kind       string     `json:"kind"`
	RequestID  uint64     `json:"request_id"`
	Invocation Invocation `json:"invocation"`
}

type technicalError struct {
	Kind     string  `json:"kind"`
	ID       *uint64 `json:"id"`
	Category string  `json:"category"`
	Message  string  `json:"message"`
}

type measuredReply struct {
	Kind           string  `json:"kind"`
	PeakRSSBytes   *uint64 `json:"peak_rss_bytes"`
	RuntimeVersion string  `json:"runtime_version"`
	Platform       string  `json:"platform"`
}

type describedReply struct {
	Kind        string             `json:"kind"`
	Description AdapterDescription `json:"description"`
}

type begunReply struct {
	Kind   string `json:"kind"`
	CaseID string `json:"case_id"`
}

type invokedReply struct {
	Kind        string                   `json:"kind"`
	Observation Observation              `json:"observation"`
	Events      []VerificationEvent      `json:"events"`
	Calls       *[]InvocationObservation `json:"calls,omitempty"`
	Resources   *[]ResourceObservation   `json:"resources,omitempty"`
}

type valueCheckedReply struct {
	Kind    string     `json:"kind"`
	Outcome ValueCheck `json:"outcome"`
}

type observedReply struct {
	Kind   string             `json:"kind"`
	States map[string][]Value `json:"states"`
}

type endedReply struct {
	Kind string `json:"kind"`
}

type byeReply struct {
	Kind string `json:"kind"`
}

// Session state is confined to the executor goroutine.

var (
	session             TestSession
	events              []VerificationEvent
	calls               []InvocationObservation
	resources           []ResourceObservation
	observesCalls       bool
	observesResources   bool
	observationBytes    int
	observationCount    int
	observationOverflow bool
	epoch               int
	stopped             bool
)

type mockResult struct {
	observation Observation
	err         error
}

var (
	pendingMu sync.Mutex
	pending          = map[uint64]chan mockResult{}
	nextMock  uint64 = 1
)

type queuedRequest struct {
	id            uint64
	request       json.RawMessage
	shutdown      bool
	protocolID    *uint64
	protocolError error
}

type requestQueue struct {
	mu     sync.Mutex
	cond   *sync.Cond
	items  []queuedRequest
	closed bool
}

func newRequestQueue() *requestQueue {
	queue := &requestQueue{}
	queue.cond = sync.NewCond(&queue.mu)
	return queue
}

func (queue *requestQueue) push(request queuedRequest) {
	queue.mu.Lock()
	queue.items = append(queue.items, request)
	queue.cond.Signal()
	queue.mu.Unlock()
}

func (queue *requestQueue) close() {
	queue.mu.Lock()
	queue.closed = true
	queue.cond.Broadcast()
	queue.mu.Unlock()
}

func (queue *requestQueue) pop() (queuedRequest, bool) {
	queue.mu.Lock()
	defer queue.mu.Unlock()
	for len(queue.items) == 0 && !queue.closed {
		queue.cond.Wait()
	}
	if len(queue.items) == 0 {
		return queuedRequest{}, false
	}
	request := queue.items[0]
	queue.items = queue.items[1:]
	return request, true
}

var queue = newRequestQueue()

var outputMu sync.Mutex

func sendMessage(message any) error {
	outputMu.Lock()
	defer outputMu.Unlock()
	line, err := json.Marshal(messageEnvelope{ProtocolVersion: protocolVersion, Message: message})
	if err != nil {
		return err
	}
	if len(line) > maxMessageBytes {
		return errors.New("Outgoing protocol message exceeds byte limit")
	}
	line = append(line, '\n')
	_, err = os.Stdout.Write(line)
	return err
}

func technical(id *uint64, category string, problem error) {
	message := problem.Error()
	if utf8.RuneCountInString(message) > maxErrorMessageRunes {
		message = string([]rune(message)[:maxErrorMessageRunes])
	}
	_ = sendMessage(technicalError{Kind: "technical-error", ID: id, Category: category, Message: message})
}

func decodeStrict(data []byte, target any) error {
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(target); err != nil {
		return err
	}
	var extra any
	if err := decoder.Decode(&extra); err != io.EOF {
		return errors.New("Unexpected trailing JSON content")
	}
	return nil
}

func probeKind(request json.RawMessage) string {
	var probe struct {
		Kind string `json:"kind"`
	}
	if err := json.Unmarshal(request, &probe); err != nil {
		return ""
	}
	return probe.Kind
}

func parseID(raw json.RawMessage) (uint64, error) {
	text := string(raw)
	if text == "" {
		return 0, errors.New("Invalid protocol version/request ID")
	}
	for _, character := range text {
		if character < '0' || character > '9' {
			return 0, errors.New("Invalid protocol version/request ID")
		}
	}
	id, err := strconv.ParseUint(text, 10, 64)
	if err != nil || id == 0 || id > maxRequestID {
		return 0, errors.New("Invalid protocol version/request ID")
	}
	return id, nil
}

func deliverMock(requestID uint64, observation Observation) bool {
	pendingMu.Lock()
	defer pendingMu.Unlock()
	waiter, ok := pending[requestID]
	if !ok {
		return false
	}
	delete(pending, requestID)
	waiter <- mockResult{observation: observation}
	return true
}

func rejectAll(reason string) {
	pendingMu.Lock()
	waiters := make([]chan mockResult, 0, len(pending))
	for id, waiter := range pending {
		waiters = append(waiters, waiter)
		delete(pending, id)
	}
	pendingMu.Unlock()
	for _, waiter := range waiters {
		waiter <- mockResult{err: errors.New(reason)}
	}
}

func end() (err error) {
	defer func() {
		if recovered := recover(); recovered != nil {
			err = fmt.Errorf("Adapter panicked during cleanup: %v", recovered)
		}
	}()

	previous := session
	session = nil
	events = nil
	calls = nil
	resources = nil
	observationBytes = 0
	observationCount = 0
	observationOverflow = false
	epoch++
	rejectAll("Session ended before collaborator response")
	if previous != nil {
		return previous.Close()
	}
	return nil
}

func projectAdapter() (ProjectAdapter, error) {
	if adapter == nil {
		return nil, errors.New("Missing project adapter methods")
	}
	return adapter, nil
}

func requireSession() (TestSession, error) {
	if session == nil {
		return nil, errors.New("No active testing session")
	}
	return session, nil
}

func contains(values []string, target string) bool {
	for _, value := range values {
		if value == target {
			return true
		}
	}
	return false
}

func normalizeDescription(description AdapterDescription) AdapterDescription {
	if description.Subjects == nil {
		description.Subjects = []SubjectDescription{}
	}
	if description.Artifacts == nil {
		description.Artifacts = []string{}
	}
	if description.Capabilities == nil {
		description.Capabilities = []string{}
	}
	if description.Representations == nil {
		description.Representations = map[string]string{}
	}
	return description
}

func normalizeInvocation(invocation Invocation) Invocation {
	if invocation.Arguments == nil {
		invocation.Arguments = []Value{}
	}
	return invocation
}

func marshalObservation(value any) ([]byte, error) {
	var buffer bytes.Buffer
	encoder := json.NewEncoder(&buffer)
	encoder.SetEscapeHTML(false)
	if err := encoder.Encode(value); err != nil {
		return nil, err
	}
	return bytes.TrimSuffix(buffer.Bytes(), []byte("\n")), nil
}

// Recorded observations are budgeted per case and detached at record time.
func observationBudget(value any) ([]byte, error) {
	data, err := marshalObservation(value)
	if err != nil {
		return nil, err
	}
	if observationCount >= 10000 || len(data) > maxMessageBytes-observationBytes {
		observationOverflow = true
		return nil, errors.New("Observation budget exceeded")
	}
	observationBytes += len(data)
	observationCount++
	return data, nil
}

func recordObservation[T any](value T, target *[]T) error {
	data, err := observationBudget(value)
	if err != nil {
		return err
	}
	var stored T
	if err := json.Unmarshal(data, &stored); err != nil {
		return err
	}
	*target = append(*target, stored)
	return nil
}

func peakRSS() (*uint64, error) {
	if runtime.GOOS != "linux" {
		return nil, nil
	}
	status, err := os.ReadFile("/proc/self/status")
	if err != nil {
		return nil, err
	}
	for _, line := range strings.Split(string(status), "\n") {
		if !strings.HasPrefix(line, "VmHWM:") {
			continue
		}
		fields := strings.Fields(line)
		if len(fields) != 3 {
			return nil, errors.New("Malformed VmHWM line")
		}
		kilobytes, err := strconv.ParseUint(fields[1], 10, 64)
		if err != nil {
			return nil, errors.New("Malformed VmHWM value")
		}
		peak := kilobytes * 1024
		return &peak, nil
	}
	return nil, nil
}

var resolutionCache = map[string]string{}

// Go resolves imports through the module-aware toolchain, relative to the
// importer's module; native resolution is evidence, not a closure proof.
func resolveGoPackage(importer string, specifier string) (string, error) {
	key := filepath.Dir(importer) + "\x00" + specifier
	if cached, ok := resolutionCache[key]; ok {
		return cached, nil
	}
	command := exec.Command("go", "list", "-find", "-f", "{{.Dir}}", specifier)
	command.Dir = filepath.Dir(importer)
	output, err := command.Output()
	if err != nil {
		message := err.Error()
		var exitError *exec.ExitError
		if errors.As(err, &exitError) && len(exitError.Stderr) > 0 {
			message = strings.TrimSpace(string(exitError.Stderr))
		}
		return "", fmt.Errorf("Dependency resolution failed: %s", message)
	}
	path := strings.TrimSpace(string(output))
	if path == "" {
		return "", errors.New("Dependency resolution returned no path")
	}
	resolutionCache[key] = path
	return path, nil
}

func execute(item queuedRequest) (err error) {
	defer func() {
		if recovered := recover(); recovered != nil {
			err = fmt.Errorf("%v", recovered)
		}
	}()
	return runRequest(item.id, item.request)
}

func runRequest(id uint64, raw json.RawMessage) error {
	send := func(value any) error {
		return sendMessage(replyMessage{Kind: "reply", ID: id, Reply: value})
	}
	switch kind := probeKind(raw); kind {
	case "metrics":
		var request metricsRequest
		if err := decodeStrict(raw, &request); err != nil {
			return err
		}
		peak, err := peakRSS()
		if err != nil {
			return err
		}
		return send(measuredReply{
			Kind:           "measured",
			PeakRSSBytes:   peak,
			RuntimeVersion: runtime.Version(),
			Platform:       runtime.GOOS,
		})
	case "describe":
		var request describeRequest
		if err := decodeStrict(raw, &request); err != nil {
			return err
		}
		current, err := projectAdapter()
		if err != nil {
			return err
		}
		description, err := current.Describe()
		if err != nil {
			return err
		}
		return send(describedReply{Kind: "described", Description: normalizeDescription(description)})
	case "resolve-dependency":
		var request resolveDependencyRequest
		if err := decodeStrict(raw, &request); err != nil {
			return err
		}
		if request.Mode != "import" || request.Specifier == "" || request.Importer == "" ||
			strings.HasPrefix(request.Specifier, "-") || !filepath.IsAbs(request.Importer) {
			return errors.New("Invalid dependency resolution request")
		}
		path, err := resolveGoPackage(request.Importer, request.Specifier)
		if err != nil {
			return err
		}
		return send(dependencyResolvedReply{Kind: "dependency-resolved", Path: path})
	case "begin":
		var request beginRequest
		if err := decodeStrict(raw, &request); err != nil {
			return err
		}
		if request.CaseID == "" {
			return errors.New("Invalid case ID")
		}
		if session != nil {
			return errors.New("Previous session must be closed")
		}
		events = nil
		calls = nil
		resources = nil
		observationBytes = 0
		observationCount = 0
		observationOverflow = false
		epoch++
		current := epoch
		owner, err := projectAdapter()
		if err != nil {
			return err
		}
		description, err := owner.Describe()
		if err != nil {
			return err
		}
		observesCalls = contains(description.Capabilities, "call-observation")
		observesResources = contains(description.Capabilities, "resource-observation")
		context := TestContext{
			Collaborator: func(invocation Invocation) (Observation, error) {
				if epoch != current {
					return Observation{}, errors.New("Session is no longer active")
				}
				pendingMu.Lock()
				requestID := nextMock
				nextMock++
				waiter := make(chan mockResult, 1)
				pending[requestID] = waiter
				pendingMu.Unlock()
				invocation = normalizeInvocation(invocation)
				if err := sendMessage(mockCallMessage{
					Kind:       "mock-call",
					RequestID:  requestID,
					Invocation: invocation,
				}); err != nil {
					pendingMu.Lock()
					delete(pending, requestID)
					pendingMu.Unlock()
					return Observation{}, err
				}
				result := <-waiter
				return result.observation, result.err
			},
			Emit: func(subject string, payload Value) error {
				if epoch != current {
					return errors.New("No active session for event")
				}
				return recordObservation(VerificationEvent{Subject: subject, Payload: payload}, &events)
			},
			RecordCall: func(invocation Invocation, observation Observation) error {
				if epoch != current || !observesCalls {
					return errors.New("No active actual call observer")
				}
				return recordObservation(InvocationObservation{
					Invocation:  normalizeInvocation(invocation),
					Observation: observation,
				}, &calls)
			},
			RecordResource: func(observation ResourceObservation) error {
				if epoch != current || !observesResources {
					return errors.New("No active resource observer")
				}
				if observation.Resource == "" || (observation.Operation != nil && *observation.Operation == "") {
					return errors.New("Invalid resource observation")
				}
				if observation.Access != "read" && observation.Access != "write" && observation.Access != "observe" {
					return errors.New("Invalid resource observation")
				}
				return recordObservation(observation, &resources)
			},
		}
		next, err := owner.Begin(request.CaseID, context)
		if err != nil {
			return err
		}
		if next == nil {
			return errors.New("Invalid project session")
		}
		session = next
		return send(begunReply{Kind: "begun", CaseID: request.CaseID})
	case "invoke":
		var request invokeRequest
		if err := decodeStrict(raw, &request); err != nil {
			return err
		}
		if !subjects[request.Invocation.Subject] {
			return errors.New("Invocation outside selected subjects")
		}
		active, err := requireSession()
		if err != nil {
			return err
		}
		events = nil
		calls = nil
		resources = nil
		observationBytes = 0
		observationCount = 0
		observationOverflow = false
		observation, err := active.Invoke(normalizeInvocation(request.Invocation))
		if err != nil {
			return err
		}
		if observationOverflow {
			return errors.New("Observation budget exceeded")
		}
		storedEvents := events
		if storedEvents == nil {
			storedEvents = []VerificationEvent{}
		}
		reply := invokedReply{Kind: "invoked", Observation: observation, Events: storedEvents}
		if observesCalls {
			storedCalls := calls
			if storedCalls == nil {
				storedCalls = []InvocationObservation{}
			}
			reply.Calls = &storedCalls
		}
		if observesResources {
			storedResources := resources
			if storedResources == nil {
				storedResources = []ResourceObservation{}
			}
			reply.Resources = &storedResources
		}
		return send(reply)
	case "validate":
		var request validateRequest
		if err := decodeStrict(raw, &request); err != nil {
			return err
		}
		if !subjects[request.Subject] {
			return errors.New("Validation outside selected subjects")
		}
		active, err := requireSession()
		if err != nil {
			return err
		}
		validator, ok := active.(ValueValidator)
		if !ok {
			return send(valueCheckedReply{
				Kind: "value-checked",
				Outcome: ValueCheck{
					Kind:   "unavailable",
					Reason: pointerTo("No production value factory/decoder described"),
				},
			})
		}
		outcome, err := validator.Validate(request.Subject, request.Value)
		if err != nil {
			return err
		}
		return send(valueCheckedReply{Kind: "value-checked", Outcome: outcome})
	case "observe":
		var request observeRequest
		if err := decodeStrict(raw, &request); err != nil {
			return err
		}
		active, err := requireSession()
		if err != nil {
			return err
		}
		observer, ok := active.(StateObserver)
		if !ok {
			return AdapterError(errors.New("No actual state observer"))
		}
		for _, entity := range request.Entities {
			if !subjects[entity] {
				return errors.New("Observation outside selected subjects")
			}
		}
		states, err := observer.Observe(request.Entities)
		if err != nil {
			return err
		}
		if states == nil {
			states = map[string][]Value{}
		}
		for entity, rows := range states {
			if rows == nil {
				states[entity] = []Value{}
			}
		}
		return send(observedReply{Kind: "observed", States: states})
	case "end":
		var request endRequest
		if err := decodeStrict(raw, &request); err != nil {
			return err
		}
		if err := end(); err != nil {
			return err
		}
		return send(endedReply{Kind: "ended"})
	case "shutdown":
		var request shutdownRequest
		if err := decodeStrict(raw, &request); err != nil {
			return err
		}
		if err := end(); err != nil {
			return err
		}
		return send(byeReply{Kind: "bye"})
	default:
		return errors.New("Unknown worker request")
	}
}

func protocolFailure(id *uint64, err error) {
	rejectAll("Protocol violation")
	queue.push(queuedRequest{protocolID: id, protocolError: err})
}

func handleLine(line string) {
	var envelope requestEnvelope
	if err := decodeStrict([]byte(line), &envelope); err != nil {
		protocolFailure(nil, err)
		return
	}
	id, idErr := parseID(envelope.ID)
	if envelope.ProtocolVersion != protocolVersion || idErr != nil {
		var report *uint64
		if idErr == nil {
			report = &id
		}
		protocolFailure(report, errors.New("Invalid protocol version/request ID"))
		return
	}
	if len(envelope.Request) == 0 {
		protocolFailure(&id, errors.New("Unexpected protocol fields"))
		return
	}
	kind := probeKind(envelope.Request)
	if kind == "mock-reply" {
		var request mockReplyRequest
		if err := decodeStrict(envelope.Request, &request); err != nil {
			protocolFailure(&id, err)
			return
		}
		if !deliverMock(request.RequestID, request.Observation) {
			protocolFailure(&id, errors.New("Unknown collaborator response"))
		}
		return
	}
	if stopped {
		protocolFailure(&id, errors.New("Worker already shutting down"))
		return
	}
	if kind == "shutdown" {
		stopped = true
	}
	queue.push(queuedRequest{id: id, request: envelope.Request, shutdown: kind == "shutdown"})
}

func readInput() {
	reader := bufio.NewReader(os.Stdin)
	for {
		line, err := reader.ReadString('\n')
		content := strings.TrimSuffix(line, "\n")
		content = strings.TrimSuffix(content, "\r")
		if len(content) > maxMessageBytes {
			protocolFailure(nil, errors.New("Incoming protocol message exceeds byte limit"))
		} else if len(content) > 0 || err == nil {
			handleLine(content)
		}
		if err != nil {
			break
		}
	}
	rejectAll("Worker input closed")
	queue.close()
}

func runExecutor() {
	for {
		item, ok := queue.pop()
		if !ok {
			break
		}
		if item.protocolError != nil {
			cleanup := end()
			technical(item.protocolID, "protocol", item.protocolError)
			if cleanup != nil {
				technical(item.protocolID, "cleanup", cleanup)
			}
			continue
		}
		if err := execute(item); err != nil {
			id := item.id
			cleanup := end()
			category := "unclassified"
            var attributed *TechnicalError
            if errors.As(err, &attributed) && (attributed.Category == "adapter" || attributed.Category == "environment") { category = attributed.Category }
            technical(&id, category, err)
			if cleanup != nil {
				technical(&id, "cleanup", cleanup)
			}
			continue
		}
		if item.shutdown {
			break
		}
	}
	if cleanup := end(); cleanup != nil {
		technical(nil, "cleanup", cleanup)
	}
}

func main() {
	go readInput()
	runExecutor()
}
