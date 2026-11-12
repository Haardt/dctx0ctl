package contracttests

import (
	"cinema/domain"
	"encoding/json"
	"errors"
	"fmt"
	app "github.com/Haardt/dctx0ctl/applicationtests/go"
	"os"
	"strings"
	"testing"
	"time"
	"unicode/utf8"
)

// Explicit opt-in until native collection and source/build evidence are accepted.
// The production decoder receives bytes directly, before any testing projection.
func TestCinemaRuntimeContracts(t *testing.T) {
	endpoint := os.Getenv("DCTX0_APPLICATION_SOCKET")
	if endpoint == "" {
		t.Skip("application assessment service not selected")
	}
	bytes, err := os.ReadFile(os.Getenv("DCTX0_APPLICATION_PLAN"))
	if err != nil {
		t.Fatal(err)
	}
	var bundle struct {
		Model json.RawMessage `json:"model"`
	}
	if err := json.Unmarshal(bytes, &bundle); err != nil {
		t.Fatal(err)
	}
	codec := &valueCodec{}
	if err := json.Unmarshal(bundle.Model, codec); err != nil {
		t.Fatal(err)
	}
	client, err := app.Dial(endpoint, app.DefaultFrameBytes, 10*time.Second)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		defer client.Close()
		if _, err := client.Finish(); err != nil {
			t.Error(err)
		}
	})
	client.DecoderTest(t, productionDecoder, codec.encode)
}
func productionDecoder(raw []byte, ty app.TypeRef) (any, error) {
	actual, err := domain.DecodeJSON(raw)
	if err != nil {
		if !utf8.Valid(raw) || !json.Valid(raw) {
			return nil, app.Reject(err.Error())
		}
		return nil, err
	}
	if ty.Symbol == nil {
		return nil, fmt.Errorf("Missing native contract identity")
	}
	err = domain.Validate(strings.TrimPrefix(*ty.Symbol, root), actual)
	var contract *domain.ContractError
	if errors.As(err, &contract) {
		return nil, app.Reject(err.Error())
	}
	return actual, err
}
