# CaseCli: a small Go application designed with DCTX0

Convert text to upper or lower case using Go's Unicode simple case mappings.
The specification declares two pure operations and eight concrete examples:
mixed case, empty input, punctuation/digits, and non-ASCII text.

```sh
go build -o casecli ./cmd/casecli
./casecli upper 'Hello World'
./casecli lower 'Hello World'
```

Output is `HELLO WORLD` and `hello world`. Missing arguments or an unknown
command exit with code 2. The CLI joins additional text arguments with spaces.

## DCTX0 workflow

Install the published DCTX0 release and Go yourself. From this directory:

```sh
dctx0 install --project . --agent codex,claude
dctx0 profile go create --project . --agents
dctx0 check dctx0
dctx0 tasks plan dctx0 --project . --target go
dctx0 tasks waves dctx0
dctx0 verify init dctx0 --project . --target go \
  --subject CaseCli.Text.upper --subject CaseCli.Text.lower
dctx0 verify run dctx0 --project .
```

`verify init` preserves the supplied adapter and creates the remaining testing
transport files. The adapter calls the production functions in `textcase`.
DCTX0 checks 24 finite cases: 8 examples and 16 generated operation cases.
Generated cases validate String results; the eight examples establish the stated
conversion expectations. This is not a proof of case conversion for all Unicode.
CLI argument parsing, stdout and exit codes are checked separately in the
container acceptance; they are outside the DCTX0 operation plan.

Published v0.1.0 reports finite-plan PASS but blocks automatic task adoption
for this spec with `INCOMPLETE_REQUIREMENTS`/`REQUIREMENT_MAPPING`, including
`pure` and example facts. Tasks must not be described as Done/PASS on that basis.

Version 0.1.1 fixes this mapping: mandatory operation cases account
for the declared `pure` requirement, and executed examples account for their
`given`/`when`/`expect` clauses. With complete native provenance, the four
eligible CaseCli tasks are automatically adopted. A failing example prevents
adoption; repeated successful verification preserves completed task history.
This correction is not included in the published v0.1.0 binary. Use v0.1.1 or newer.

## Reproduce the isolated release acceptance

From the repository root, with Docker and the existing `golang:1.25.5` image:

```sh
python3 scripts/container_case_acceptance.py
```

The runner installs the real GitHub release in a fresh container, tests skills
for Codex and Claude, profile/AGENTS setup, preservation of local edits, task
claims and pinned profiles, actual CLI calls, native verification, a deliberately
incorrect Upper implementation, and broken/repaired replay. It records runtime
and child peak RSS alongside functional checks. Go compiler/worker provenance
is retained in DCTX0 native reports. No host credentials are mounted and no
external coding-agent session is launched. The container is stopped and kept
for inspection; use `--keep-running` to leave it running.

Evidence is saved under `target/container-case/<container-name>/`.
