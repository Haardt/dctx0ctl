package applicationtests

import (
	"bufio"
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net"
	"sync"
	"time"
)

// Client serializes only evaluator exchanges. Production calls run outside its
// mutex, with explicit per-case context handles for parallel native tests.
type Client struct {
	mutex      sync.Mutex
	connection net.Conn
	reader     *bufio.Reader
	limit      int
	timeout    time.Duration
	nextID     uint64
}

func Dial(socket string, limit int, timeout time.Duration) (*Client, error) {
	if limit <= 0 || limit > maxFrameBytes || timeout <= 0 {
		return nil, errors.New("invalid application assessment transport budgets")
	}
	connection, err := net.DialTimeout("unix", socket, timeout)
	if err != nil {
		return nil, err
	}
	return NewClient(connection, limit, timeout)
}

// NewClient also supports owned pipes in protocol conformance tests.
func NewClient(connection net.Conn, limit int, timeout time.Duration) (*Client, error) {
	if connection == nil || limit <= 0 || limit > maxFrameBytes || timeout <= 0 {
		if connection != nil {
			_ = connection.Close()
		}
		return nil, errors.New("invalid application assessment connection or budgets")
	}
	return &Client{connection: connection, reader: bufio.NewReader(connection), limit: limit, timeout: timeout, nextID: 1}, nil
}

func (client *Client) Close() error { return client.connection.Close() }

func readFrame(reader *bufio.Reader, limit int) ([]byte, error) {
	var data []byte
	for {
		part, err := reader.ReadSlice('\n')
		if len(data)+len(part) > limit {
			return nil, errors.New("application assessment reply exceeds its budget")
		}
		data = append(data, part...)
		if errors.Is(err, bufio.ErrBufferFull) {
			continue
		}
		if err != nil {
			return nil, fmt.Errorf("incomplete assessment reply: %w", err)
		}
		return data, nil
	}
}

func decodeStrict(data []byte, destination any) error {
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(destination); err != nil {
		return err
	}
	if err := decoder.Decode(new(any)); err != io.EOF {
		return errors.New("extra assessment reply data")
	}
	return nil
}

func (client *Client) exchange(request any) (reply, error) {
	client.mutex.Lock()
	defer client.mutex.Unlock()
	if client.nextID > maxRequestID {
		return reply{}, errors.New("assessment correlation IDs exhausted")
	}
	id := client.nextID
	client.nextID++
	payload, err := json.Marshal(request)
	if err != nil {
		return reply{}, err
	}
	data, err := json.Marshal(frame{ProtocolVersion: ProtocolVersion, ID: id, Payload: payload})
	if err != nil {
		return reply{}, err
	}
	data = append(data, '\n')
	if len(data) > client.limit {
		return reply{}, errors.New("application assessment request exceeds its budget")
	}
	if err = client.connection.SetDeadline(time.Now().Add(client.timeout)); err != nil {
		return reply{}, err
	}
	for len(data) > 0 {
		n, writeError := client.connection.Write(data)
		if writeError != nil {
			return reply{}, writeError
		}
		if n == 0 {
			return reply{}, io.ErrShortWrite
		}
		data = data[n:]
	}
	data, err = readFrame(client.reader, client.limit)
	if err != nil {
		return reply{}, err
	}
	var envelope frame
	if err = decodeStrict(data, &envelope); err != nil {
		return reply{}, err
	}
	if envelope.ProtocolVersion != ProtocolVersion || envelope.ID != id {
		return reply{}, errors.New("assessment version or correlation mismatch")
	}
	var response reply
	if err = decodeStrict(envelope.Payload, &response); err != nil {
		return reply{}, err
	}
	if response.Kind == "rejected" {
		if response.Issue == nil {
			return reply{}, errors.New("assessment rejected without a diagnostic")
		}
		return response, fmt.Errorf("%s: %s", response.Issue.Code, response.Issue.Message)
	}
	return response, nil
}

func (client *Client) Finish() (Summary, error) {
	response, err := client.exchange(map[string]any{"kind": "finish"})
	if err != nil {
		return Summary{}, err
	}
	if response.Kind != "finished" || response.Summary == nil {
		return Summary{}, errors.New("assessment did not finalize")
	}
	summary := *response.Summary
	if !summary.Finalized || summary.Status != "PASS" || summary.CompletedCases != summary.RequiredCases || summary.RequiredCases <= 0 || summary.MissingCases != 0 || summary.MissingTests != 0 || summary.Issues != 0 {
		return summary, fmt.Errorf("central assessment incomplete or failed: %s", summary.Status)
	}
	return summary, nil
}
