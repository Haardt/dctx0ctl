// Testing-only support. Child environments never mutate this process.
package main

import (
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strings"
	"testing"
)

func requireDeclaredFailure(t testing.TB, observation Observation, technical error, declaration string) DeclaredFailureValue {
	t.Helper()
	f, e := DeclaredFailure(observation, technical, declaration)
	if e != nil {
		t.Fatal(e)
	}
	return f
}
func requireReturnedRecord(t testing.TB, observation Observation, technical error) map[string]Value {
	t.Helper()
	f, e := ReturnedRecord(observation, technical)
	if e != nil {
		t.Fatal(e)
	}
	return f
}
func requireStringField(t testing.TB, record Value, name, expected string) {
	t.Helper()
	if e := StringField(record, name, expected); e != nil {
		t.Fatal(e)
	}
}

// ConfiguredFixture reads a project-chosen key without printing its value.
func ConfiguredFixture(key string) (*string, error) {
	v, ok := os.LookupEnv(key)
	if !ok || v == "" {
		return nil, fmt.Errorf("fixture prerequisite %s is not configured", key)
	}
	return &v, nil
}

// UnavailableFixture explicitly clears a project-chosen switch in the child only.
func UnavailableFixture() *string { v := ""; return &v }
func ContractPlanCommand(cli, project string, args []string, environment map[string]*string) (*exec.Cmd, error) {
	cmd := exec.Command(cli, args...)
	cmd.Dir = project
	cmd.Env = []string{}
	for key := range environment {
		if key == "" || strings.ContainsAny(key, "=\x00") {
			return nil, fmt.Errorf("invalid fixture environment key")
		}
	}
	for _, entry := range os.Environ() {
		key, _, _ := strings.Cut(entry, "=")
		if _, replaced := environment[key]; !replaced {
			cmd.Env = append(cmd.Env, entry)
		}
	}
	for key, value := range environment {
		if value != nil {
			if strings.ContainsRune(*value, '\x00') {
				return nil, fmt.Errorf("invalid fixture environment value for %s", key)
			}
			cmd.Env = append(cmd.Env, key+"="+*value)
		}
	}
	return cmd, nil
}
func runContractPlan(t testing.TB, args []string, environment map[string]*string) {
	t.Helper()
	project, e := filepath.Abs("../../..")
	if e != nil {
		t.Fatal(e)
	}
	cli := os.Getenv("DCTX0_TEST_CLI")
	if cli == "" {
		cli = "dctx0"
	}
	cmd, e := ContractPlanCommand(cli, project, args, environment)
	if e != nil {
		t.Fatal(e)
	}
	output, e := cmd.Output()
	var report struct {
		Published bool `json:"published"`
		Run       struct {
			Result struct {
				Status string `json:"status"`
			} `json:"result"`
		} `json:"run"`
	}
	if json.Unmarshal(output, &report) != nil {
		t.Fatal("central verification did not return a report; inspect bindings test directly")
	}
	if e != nil || report.Published || report.Run.Result.Status != "PASS" {
		t.Fatalf("central verification incomplete or failed (%s); inspect bindings test directly", report.Run.Result.Status)
	}
}

// fixtureScenario executes an authored test body with a child-only environment.
// Go's test framework runs child cleanups even when the body calls Fatal.
func fixtureScenario(t *testing.T, environment map[string]*string, body func()) {
	t.Helper()
	const marker = "DCTX0_FIXTURE_SCENARIO"
	if os.Getenv(marker) == t.Name() {
		body()
		return
	}
	childEnv := make(map[string]*string, len(environment)+1)
	for k, v := range environment {
		childEnv[k] = v
	}
	name := t.Name()
	childEnv[marker] = &name
	args := []string{"-test.run=^" + regexp.QuoteMeta(t.Name()) + "$", "-test.count=1"}
	for _, name := range []string{"test.timeout", "test.gocoverdir"} {
		if option := flag.Lookup(name); option != nil && option.Value.String() != "" {
			args = append(args, "-"+name+"="+option.Value.String())
		}
	}
	cmd, e := ContractPlanCommand(os.Args[0], ".", args, childEnv)
	if e != nil {
		t.Fatal(e)
	}
	if deadline, ok := t.Deadline(); ok {
		ctx, cancel := context.WithDeadline(context.Background(), deadline)
		defer cancel()
		replacement := exec.CommandContext(ctx, cmd.Path, cmd.Args[1:]...)
		replacement.Env = cmd.Env
		replacement.Dir = cmd.Dir
		cmd = replacement
	}
	if e = cmd.Run(); e != nil {
		t.Fatal("isolated fixture scenario failed; inspect the scenario directly")
	}
}
