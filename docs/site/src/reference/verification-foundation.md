# Verification foundation API

Implemented 2026-10-10, preceding native harness acceptance. The accepted full
roadmap is in [verification-architecture.md](verification-architecture.md).

`dctx0-core` exports schema/protocol version 1, tagged `VerificationValue`,
`Invocation`, `Observation`, ordered `InvocationObservation`, structured issues,
PASS/FAIL/UNKNOWN and shared budgets. Values encode integers and decimals as text;
JSON numbers never silently round large integers. Records use sorted keys.

`dctx0-interpreter::evaluate` consumes validated IR, one expression, concrete
variables, an ordered observation tape and budgets. Its result is a value, next
invocation, source-related UNKNOWN or application exception. Append a matching
observation and call evaluate again to resume. Equal repeated calls occupy separate
positions. Continuations reject wrong subjects/arguments or unused observations;
a caller must preserve the expression, variables and budget. This version restarts
pure evaluation from the beginning, so its step budget applies to each evaluation,
not a whole future orchestration session. Only evaluation depth/steps and wire
value depth/size are enforced here. Seed, generated-case count, collection-size
and shrink-step preferences are metadata for the still-planned generator/shrinker;
this evaluator never truncates explicitly supplied cases or lists.

Supported expression evaluation includes exact Int/finite Decimal, String,
Boolean, records/fields, variants/payloads, options, lists, unary negation/not,
short-circuit and/or/implies, comparisons/equality/membership, inclusive integral
ranges, string concatenation, count/all/any/contains/distinct/sum/min/max, and pure
operation calls. Builtins: Some/None, lowercase/uppercase/trim, UTF-16 string or list
length, startsWith/endsWith and reverse. Bound collection variables shadow locally.

Integer division truncates toward zero; decimals use exact base-10 arithmetic.
Nonterminating division requires a future explicit rounding implementation and
returns UNKNOWN. Decimal remainder, Float arithmetic, timestamp/duration arithmetic,
round/divide rounding builtins, state and trace evaluation are unsupported. Empty
min/max is UNKNOWN. Unlike the reference's empty descending range, descending
ranges currently produce explicit UNKNOWN; this conservative limitation is not a
new compiler rule. Invalid runtime values and limits retain source locations.
Values returned from a pure call are checked against their declared IR result
contract, including aliases, records, variants, options/lists, UUIDs, RFC3339
timestamps, finite Float representations, modeled inclusive ranges and executable
where conditions. Unsupported refinements stay UNKNOWN. Complete plan projection,
input generation and native verification still remain outstanding.

`dctx0-verification` defines explicit pinned finite plans, scopes, obligations,
cases and evidence. `next_invocation` derives calls from the Rust evaluator.
`assess` evaluates Boolean assertions centrally: false/application exceptions are
FAIL; missing cases/invocations, unsupported obligations, empty corpora, malformed
plans or evidence, non-Boolean assertions and resource exhaustion cannot PASS.
A known FAIL dominates UNKNOWN, while incomplete-obligation IDs remain visible.
Bounded-corpus obligations are listed separately. Case IDs are globally unique;
obligation IDs are unique; every selected subject must have an obligation.

This library assesses a supplied plan. It does **not** prove that supplied
obligations cover all requirements of its named snapshot, authenticate provenance,
execute a worker, infer task requirements, adopt tasks or publish history. Plan
snapshot/check fields are required labels here; store/engine must authenticate
these in the later execution increment. There is deliberately no adoption API.
Type generation, refinements, automatic IR projection, shrink and CLI replay remain
unimplemented. Existing task and SQLite schemas are unchanged.

## Validation and measurements

Run the existing coupled runner:

```sh
PATH=/home/held/.cargo/bin:$PATH python3 scripts/validate.py --coverage
```

The runner additionally measures `verification-evaluate-unbound` and
`verification-assess-unbound` for both complete example IRs. These are core-only
release workloads over modeled expressions with empty variables and no native
observations, so many results intentionally remain UNKNOWN. They measure evaluator
and assessment overhead, **not application correctness or native execution**.
Elapsed time excludes compilation and synthetic-plan preparation; process VmHWM
includes them. The prepared plan is synthetic and is not a full requirement
projection. No native application/harness costs are measured by these workloads.

For individual timing (not a substitute for coupled validation):

```sh
cargo run --release --bin dctx0 -- bench examples/cinema/dctx0 verification-evaluate-unbound 3
cargo run --release --bin dctx0 -- bench examples/runatron-next/dctx0 verification-assess-unbound 3
```

These four workloads are new and unassessed against the accepted baseline. The
runner keeps raw samples and the latest 20 runs, compares established compatible
workloads, and never promotes the baseline automatically.

Recorded checks, coverage and release samples: [verification-validation.md](verification-validation.md).
