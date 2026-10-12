# Published verification planning

The pure `dctx0_verification::project_program` projects validated IR without
changing compiler snapshots or task definitions. The engine's
`Project::verification_plan` reads published requirements and validates the exact
check occurrence. The CLI exposes this read-only stage alongside the native run,
history and replay workflow:

```sh
dctx0 check examples/cinema/dctx0
dctx0 verify plan examples/cinema/dctx0 --subject HomeCinema.MovieNight
dctx0 verify plan examples/cinema/dctx0 --subject HomeCinema.MovieNight --json
dctx0 tasks plan examples/cinema/dctx0
dctx0 verify plan examples/cinema/dctx0 --task T31 --task T60 --json
dctx0 verify plan examples/cinema/dctx0 --subject HomeCinema.MovieNight \
  --fixtures reviewed-example-inputs.json --json
dctx0 verify plan examples/cinema/dctx0 --subject HomeCinema.MovieNight \
  --historical-check 1 --json
```

Task IDs are project-local; replace the example IDs with saved tasks. Selected
tasks must belong to the current task plan with the exact selected check
occurrence, even when identical snapshot hashes recur. Historical checks must
identify a successful published occurrence. Historical planning grants no
authority to adopt current tasks. Empty effective selection is rejected. Without
explicit selectors, the CLI uses only the configured subjects/tasks, never the
whole application implicitly. Discovery, setup, run, saved history and replay
are implemented; see [the run workflow](verification-run.md).

For configured applications, planning inspects reviewed `lifecycles.json` input
presence and transition-argument coverage without executing the adapter. Missing
inputs remain explicit planning issues. State-dependent preconditions still need
actual native observations; static availability is not execution evidence.

Current planning requires a successful explicit `check` first. A normal plan
rejects spec files whose source hash differs from the published snapshot; it
never compiles/publishes a new head implicitly. Historical planning deliberately
uses the saved snapshot instead of asserting working-tree freshness. Read-only
planning neither migrates SQLite nor creates lock files or task events.

## Scope and requirements

An explicit namespace selects its declarations. Resolved type, operation,
failure, event and state dependencies supply the required closure. Examples,
invariants and lifecycles that call selected operations are included. Governing
ancestor facts and decisions are retained without selecting unrelated operations.
Unselected symbols remain listed. Opaque trace spellings never add dependency
edges and remain source-located open execution points.

Schema-1 `ProgramPlan` contains complete typed contracts and generated corpora,
operation signatures and modeled clauses, property assertions and selected cases,
ordered example actions, lifecycle transitions and connected prerequisite paths.
Proof/test declarations, policy clauses, reviews, constraints, intents and
decisions retain their original semantic nodes. They are not automatically
converted from prose to assertions. Required capabilities are explicit.

The complete unmodified Cinema plan accounts for 45 data contracts, 26 operations,
12 examples, one invariant with two assertions, one lifecycle with two transitions,
nine proof/test declarations, three reviews, four decisions and the Release policy.
This is planning coverage, not actual application execution or a PASS result.

Each example preserves the original order of mock setup, actual invocations and
checks. A later mock may use an earlier delivered result. Missing typed invocations,
unresolved events, incompatible symbolic types and missing expectations stay open.
The planner never replaces a real invocation with a declared stubbed result.

Symbolic examples need reviewed concrete input bindings. Arbitrarily generated
type-correct data cannot establish that a particular example scenario applies.
Fixtures are JSON in the following shape, using the core's tagged wire values:

```json
{
  "examples": {
    "qualified.example.name": {
      "inputName": {"kind": "integer", "value": "42"}
    }
  }
}
```

Inputs are checked against inferred IR types. Missing, surplus, incompatible or
unsupported values are reported. Binding validation does not assert that the
scenario passes. Free values with conflicting types and quantified bindings
without retained typed occurrences remain unknown instead of receiving guessed
types. Explicit closed assertions survive generated-case limits.

Cinema's concrete sample bindings are in
`examples/cinema/.dctx0/testing/typescript/examples.json`. They supply all 38
symbolic inputs across the 12 examples: consistent film/member/poll IDs, matching
request and returned-record identities, two comments, a five-star rating request,
an availability change to Watched and candidate/result lists with the first film
winning. They were authored from the spec, independently of observed application
outputs. All bindings pass the IR type/refinement checker. These are testing
inputs, not production imports or a successful scenario run. For example:

```sh
dctx0 verify plan examples/cinema/dctx0 --subject HomeCinema.MovieNight \
  --fixtures examples/cinema/.dctx0/testing/typescript/examples.json --json
```

Every lifecycle path starts at the declared initial state and contains the
shortest prerequisite prefix followed by its required transition. Loops remain
actual steps. Unreachable transitions/states, undeclared states and excessive
mandatory path lengths produce issues; they cannot be silently discarded to
establish a complete result. Actual session creation, guards, state observation,
transition execution and replay remain required native work.

Markdown provides the scope and open-point overview. JSON contains the complete
typed requirements and original clauses; it does not truncate required bodies.
Planning's exit code reports whether selection/projection itself was valid, not
whether the application passed verification. Native capability availability and
mandatory execution completion are future run-time decisions.

## Validation and measurements

The coupled runner adds `verification-project` for both complete compiler examples,
bringing release workloads to 82. These workloads project the full workspace and
generate type/parameter/property corpora; they perform no native calls or adoption.
RSS is Linux post-exec VmHWM including prepared IR, not incremental allocation.
Reduced development samples remain incompatible/unassessed against the accepted
baseline. The baseline and thresholds are unchanged.

Current coupled validation passes 173 all-feature Rust tests and 8 Python tests,
13,143/13,143 merged source lines and 1,099/1,099 LLVM functions. Raw LLVM
line/region/instantiation totals are retained separately; no branch completeness
is claimed. The successful development run and earlier incomplete runs are
retained in raw checkpoint evidence (historical evidence is available in Git history).
All 82 workloads completed. The projection sample measures Cinema at 44.857 ms /
17,805,312 peak RSS bytes and runatron-next at 5,594.290 ms / 202,964,992 bytes.
These are single development samples, not a noise-adjusted performance gate.

Full validation continues to use
`PATH=/home/held/.cargo/bin:$PATH python3 scripts/validate.py --coverage`.
The completion checklist remains [Cinema verification progress](cinema-verification-progress.md).
## Explicit DTO projections from observed state

The Rust compiler/interpreter support two additional typed collection forms:
`filter(row in collection where predicate)` retains matching elements in source
order; omitting `where` retains all elements. `map(expression for row in
collection)` creates a list with the expression's type. Both use the central
evaluation budgets and lexical binder scope; they contain no native callbacks.
These are intentional language additions for independently modeled DTO
expectations. Existing IR shapes and original example specs are unchanged.

`first(list)` returns `Option` of the list's element type: `None` for an empty
list and `Some` of its first element otherwise. It composes with filter/map for
presence projections without inventing missing entities or native observations.

`single(list)` returns its sole element. Empty or multi-element lists are
UNKNOWN with RESULT_CARDINALITY_REQUIRED; this function never supplies a dummy
entity. It supports mandatory DTO projections after independently verified
state insertion/upsert, while `first` remains appropriate for optional lookup.

`includesText(text, query)` performs a case-sensitive substring check; explicit
`lowercase` calls model case-insensitive search. `slice(list, offset, limit)`
uses nonnegative exact integer indices and retains source order. An offset past
the end yields an empty list; a limit exceeding the remaining length retains
all remaining elements, including when the exact integer exceeds native number
precision. Negative indices produce UNKNOWN with SLICE_INDEX_REQUIRED. These
functions do not infer filtering, sorting or pagination requirements from prose.

`sortBy(key for row in list)` returns the original rows in stable ascending
key order. Numeric keys use exact comparisons (Float keys retain their actual
finite IEEE-754 value); text keys compare UTF-16 code units. Equal keys preserve
input order, so sorting by ID before a primary key models a deterministic
secondary order. Key evaluation and merge comparisons consume the evaluation
budget. Incompatible or unsupported runtime key values and exhausted budgets
yield UNKNOWN; no partially sorted result is returned.

`sortDescBy(key for row in list)` reverses primary-key comparison while retaining
equal-key input order. `ifElse(condition, yes, no)` evaluates only the chosen
branch and requires matching branch types. Interpreter version 3 also defines
Float/Int/Decimal relational comparisons using the actual finite Float value
against exact numeric values, without silently rounding an integer operand.

`indexOf(list, value)` returns the first matching zero-based position, or -1.
`mean(list)` accepts exact Int/Decimal elements, sums them exactly and explicitly
converts their mean to a finite IEEE-754 Float in Rust; an empty list yields
`None`. `getOrElse(option, fallback)` supplies a fallback of the option's element
type. Both arguments are evaluated, so this is not a lazy conditional. List
search and mean computation consume the evaluation budget. Float literals are
finite values and Float equality compares numeric values, including equivalent
text representations. Interpreter version 2 and assessment version 6 record
these additions and semantic result comparison for state projections.

Assessment version 7 also accepts an entity/result record shape under a
different declaration name when every field name and declared field type is
identical. Field order does not alter a record value. Extra fields, different
nominal field types or enum payloads do not qualify for this equivalence. The
actual values and full before/after state still have to satisfy the modeled
operation; structural compatibility does not assert request-field mappings.

For example, an overview can construct its items from
`map(View { id = row.id, amount = row.amount } for row in filter(item in before.Item where item.id == request.id))`
and derive its total with `count`. An `ensures result == Overview { ... }`
then checks the actual returned DTO against those independently observed rows.
Wrong items, wrong totals and omitted matches fail; an empty matching set has
an empty items list and total zero. This capability does not automatically add
expectations to existing specs or certify unspecified DTO projections.

### Generated state preparation

`ProjectionFixtures.generated_preparations` optionally maps an operation ID to
reviewed entries containing exact canonical `bindings` and ordered `setup`
invocations. Preparations are valid only for nonpure operations and exact members
of the deterministic generated corpus; duplicate entries, undeclared operations,
wrong arity and invalid setup values invalidate the plan. Setup operations and
their resolved dependencies enter the plan closure. The core supplies no expected
result through this configuration.

Native execution opens a fresh session, runs the saved setup, observes actual
state and checks preconditions before invoking the real subject. Generated case
records retain setup. Replay checks setup against the pinned plan as well as
bindings, index and arguments, and repeats it. Existing plans/fixtures without
this optional field retain empty setup. Static unresolved preconditions are still
reported separately; executed evidence must discharge them before full adoption.

### Recorded resolution of state-dependent preconditions

Assessment version 8 can discharge planning-time `STATE_OBSERVER_REQUIRED`
hints originating in a generated operation input corpus. The central Rust checker
requires every exact generated case exactly once, matching bindings, arguments,
setup and operation observation, with PASS assessment and an explicitly recorded
Before snapshot. It independently reruns the operation's complete preconditions
against the last recorded Before snapshot. Missing, duplicated, mismatched,
unverified-phase, failing or unknown evidence prevents discharge. Empty corpora
cannot discharge hints. Manual requirements and other unsupported expressions
remain open.

The native run stores discharged hints in a content-addressed
`application/vnd.dctx0.resolved-preconditions+json` artifact. The pinned planning
report is retained; historical reports are not rewritten. This reconciles actual
evidence with a static planning limitation without asserting unseen state.
