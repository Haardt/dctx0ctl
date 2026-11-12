package applicationtests

import (
	"bufio"
	"bytes"
	"encoding/json"
	"errors"
	"net"
	"strings"
	"testing"
	"time"
)

func TestProductionDecoderReceivesRawBytesAndProjectionErrorsStaySeparate(t *testing.T) {
	raw := []byte{0xff, 0, '{', '"'}
	called := false
	actual, outcome := productionDecode(func(input []byte, _ TypeRef) (any, error) {
		called = true
		if !bytes.Equal(input, raw) {
			t.Fatal("raw input changed")
		}
		return nil, Reject("")
	}, raw, TypeRef{})
	if !called || actual != nil || outcome == nil || outcome.Kind != "rejected" || outcome.Reason == nil || *outcome.Reason != "" {
		t.Fatal("production rejection was lost")
	}
	projected := projectDecoded(func(any, TypeRef) (Value, error) { return Value{}, errors.New("codec") }, nil, TypeRef{})
	if projected.Kind != "binding-error" || projected.Message == nil || *projected.Message != "codec" {
		t.Fatal("codec rejection was confused with production")
	}
	_, crashed := productionDecode(func([]byte, TypeRef) (any, error) { panic("production") }, raw, TypeRef{})
	if crashed == nil || crashed.Kind != "exception" {
		t.Fatal("production panic was not technical")
	}
	projected = projectDecoded(func(any, TypeRef) (Value, error) { panic("projector") }, nil, TypeRef{})
	if projected.Kind != "binding-error" {
		t.Fatal("projector panic was not a binding error")
	}
	_, technical := productionDecode(func([]byte, TypeRef) (any, error) { return nil, errors.New("I/O failed") }, raw, TypeRef{})
	if technical == nil || technical.Kind != "exception" {
		t.Fatal("technical decoder error counted as rejection")
	}
	wire, err := json.Marshal(outcome)
	if err != nil || !bytes.Contains(wire, []byte(`"reason":""`)) {
		t.Fatal("empty reason omitted")
	}
}

func TestFramesPreserveExactPayloadsAndRejectCorrelationsVersionsAndBudgets(t *testing.T) {
	for mutation := 0; mutation < 4; mutation++ {
		clientConnection, serverConnection := net.Pipe()
		client, err := NewClient(clientConnection, 1024, time.Second)
		if err != nil {
			t.Fatal(err)
		}
		done := make(chan error, 1)
		go func() {
			defer serverConnection.Close()
			wire, err := readFrame(bufio.NewReader(serverConnection), 1024)
			if err != nil {
				done <- err
				return
			}
			var request frame
			if err = decodeStrict(wire, &request); err != nil {
				done <- err
				return
			}
			if !bytes.Contains(request.Payload, []byte("90071992547409931234567890")) {
				done <- errors.New("exact integer lost")
				return
			}
			response := frame{ProtocolVersion: ProtocolVersion, ID: request.ID, Payload: json.RawMessage(`{"kind":"registered"}`)}
			if mutation == 1 {
				response.ID++
			}
			if mutation == 2 {
				response.ProtocolVersion++
			}
			if mutation == 3 {
				response.Payload = json.RawMessage(`{"kind":"registered","status":"PASS"}`)
			}
			wire, err = json.Marshal(response)
			if err == nil {
				_, err = serverConnection.Write(append(wire, '\n'))
			}
			done <- err
		}()
		response, err := client.exchange(map[string]any{"kind": "register", "value": Value{Kind: "integer", Value: json.RawMessage(`"90071992547409931234567890"`)}})
		if mutation == 0 && (err != nil || response.Kind != "registered") {
			t.Fatalf("valid frame: %v", err)
		}
		if mutation != 0 && err == nil {
			t.Fatalf("mutation %d accepted", mutation)
		}
		_ = client.Close()
		if err := <-done; err != nil {
			t.Fatal(err)
		}
	}
	for _, limit := range []int{0, maxFrameBytes + 1} {
		if _, err := NewClient(nil, limit, time.Second); err == nil {
			t.Fatal("invalid budget accepted")
		}
	}
	if _, err := readFrame(bufio.NewReader(strings.NewReader("{}\n")), 2); err == nil {
		t.Fatal("oversized frame accepted")
	}
	if _, err := readFrame(bufio.NewReader(strings.NewReader("{}")), 100); err == nil {
		t.Fatal("unterminated frame accepted")
	}
	if err := decodeStrict([]byte("{} {}"), new(frame)); err == nil {
		t.Fatal("trailing packet accepted")
	}
}
