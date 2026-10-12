# Tutorial verification semantics

The progressive TypeScript tutorial is tracked in
`examples/tutorial/features.json`. Planned rows are not completed acceptance.
Production code is independent of spec and testing artifacts. The external
adapter translates values and invokes real production validators/operations;
expected results, generation and verdicts remain central Rust semantics.

## Text refinements

`length` measures UTF-16 code units for strings and element count for lists,
matching the reference's JavaScript semantics. `format` supports Email, UUID,
Uri, Date and DateTime. The UUID format follows the reference version 1..5 and
variant 8/9/a/b check; the unrefined UUID builtin retains its broader UUID shape.
Email uses the reference ECMAScript whitespace set, including BOM and excluding
NEL. URI parsing uses centrally pinned `url` 2.5.7 and native production code
uses its normal URL constructor. Date rejects impossible calendar dates; this
intentionally tightens the reference's Date.parse rollover. DateTime requires a
valid RFC3339 timestamp instead of accepting arbitrary Date.parse text.

Generator `dctx0-values-3` adds valid format witnesses and string length
boundaries. Rejection generator `dctx0-rejections-3` adds actual outside-length,
malformed-format and adjacent outside-float-range values, rather than relying
only on wrong representation kinds. Bounded corpora remain bounded and are not
all-input proofs. Required boundaries exceeding the canonical value budget
produce an explicit generation issue.

## Compatibility builtins (implementation underway)

Duration uses the existing neutral Float wire representation and finite native
numbers, matching the reference property validator. Domain nonnegativity is
modeled explicitly with a range rather than silently imposed on this builtin.
Bytes use neutral lists of lossless integers in 0..255 and native Uint8Array in
TypeScript. Out-of-range values must be rejected before native array coercion;
rounding or truncation is not an accepted representation decision.

Some<T> is a present option; None is the absent singleton. Dctx0Option<T> uses
the same neutral Option encoding. Dctx0Result<S,E> uses the existing Variant
wire envelope with declaration `builtin:Dctx0Result`, alternatives Success and
Failure, and a payload checked against the corresponding type argument. These
representations reuse protocol schema 1 rather than introducing target-specific
expectation semantics. The neutral generator and interpreter validate them.

Compiler AST/IR/snapshot schemas and established goldens are not changed by
these executable semantics. Historical evidence remains readable with its
original generator/image provenance; no earlier outcome is overwritten.

## Exact rounding and division

Interpreter `dctx0-interpreter-5` implements `round` and `divide` with exact
rational arithmetic and Down (toward zero), Up (away from zero), HalfUp and
HalfEven. Signed ties retain arbitrary precision. Nonnegative scales are
bounded by the declared evaluation and value budgets before allocating powers
of ten; invalid modes and division by zero produce explicit issues.

As in the original property evaluator, `divide` preserves an exact finite
quotient even when its fractional scale exceeds the requested rounding scale.
For example, `divide(1.0, 8.0, 2, Down)` is 0.125. A nonterminating quotient
uses the explicit scale/mode; callers wanting 0.12 must also apply `round`.
Unlike the reference's early exact-result return, Rust validates scale and mode
for exact quotients too. This makes malformed execution plans fail consistently.
The TypeScript tutorial's ordinary production decimal functions independently
implement these behaviors; the external adapter only transports their results.

## Lifecycle guards and presence

Lifecycle guards now receive the selected operation's typed parameter scope,
matching the existing runtime argument binding. Guards must be Boolean and
cannot inspect after-state. The complete reference compiler cases remain
unchanged; this adds executable scope for parameterized guards rather than
changing their AST/IR representation.

Assessment `dctx0-assessment-14` supports `using presence` through the central
actual keyed-state selector. As in the reference evaluator, the single initial
state represents a present entity and the single terminal state an absent one.
This two-state contract rejects ambiguous state declarations, duplicate matching
keys, missing observers and unavailable input contracts. It does not infer
presence from an operation return value. Ordinary status lifecycles still
observe their actual discriminator field.

## Observed identity violations

Assessment `dctx0-assessment-15` distinguishes known identity violations from
missing observations. A successful `state inserts` result whose complete key
already exists is FAIL; a successful complete-source `state replaces` result
whose key is absent is also FAIL. These facts follow from complete, valid
before/after observations. They were previously classified UNKNOWN, which
blocked adoption but obscured the executed contract violation. Missing or
ambiguous observations and incomplete modeled sources retain UNKNOWN.

The tutorial mutation suite independently replaces actual HTTP insert/replace
with upsert and observes both violations through real requests. Saved-input
replay reproduces the insertion failure and passes after restoring the genuine
implementation. Declared application failures still use their explicit
preservation postconditions; no new failure behavior is inferred from prose.
