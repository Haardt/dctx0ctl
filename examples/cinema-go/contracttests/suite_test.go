package contracttests

import (
	"encoding/json"
	app "github.com/Haardt/dctx0ctl/applicationtests/go"
	"os"
	"strings"
	"testing"
)

// This parent owns one evaluator session; every logical test owns its native
// subtest and every generated case is still iterated only by the library.
func TestCinemaSuite(t *testing.T) {
	client := clientFor(t)
	bytes, err := os.ReadFile(os.Getenv("DCTX0_APPLICATION_PLAN"))
	if err != nil {
		t.Fatal(err)
	}
	var bundle struct {
		Plan struct {
			Registrations []struct {
				Test string `json:"test"`
				Form string `json:"form"`
			} `json:"registrations"`
		} `json:"plan"`
	}
	if err = json.Unmarshal(bytes, &bundle); err != nil {
		t.Fatal(err)
	}
	// Identity runs after all source operation registrations. Fixture resources
	// remain case-local; records from different cases are never fabricated here.
	registrations := bundle.Plan.Registrations
	for _, identity := range []bool{false, true} {
		for _, registration := range registrations {
			if (registration.Form == "entity-identity") != identity {
				continue
			}
			if !strings.HasPrefix(registration.Test, t.Name()+"/") {
				t.Fatal("Foreign suite registration")
			}
			t.Run(strings.TrimPrefix(registration.Test, t.Name()+"/"), func(t *testing.T) {
				factory := func(context *app.CaseContext) (app.OperationFixture, error) {
					return newCinemaFixture(t, context, client.codec)
				}
				switch registration.Form {
				case "decoder":
					client.DecoderTest(t, productionDecoder, client.codec.encode)
				case "operation":
					client.OperationContextTest(t, factory)
				case "state-change":
					client.StateChangeContextTest(t, factory)
				case "assertion":
					client.AssertionTest(t, func(id string, context uint64) (app.OperationFixture, error) {
						return newCinemaFixture(t, &app.CaseContext{ID: id, Context: context}, client.codec)
					})
				case "scenario":
					client.ScenarioTest(t, factory)
				case "lifecycle":
					client.LifecycleTest(t, factory)
				case "entity-identity":
					client.IdentityTest(t)
				default:
					t.Fatal("Unsupported suite form")
				}
			})
		}
	}
}
