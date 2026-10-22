# Editable TypeScript wire codecs

`verify scaffold-codecs` generates representation conversion from a published,
current specification snapshot. It writes testing code only. Production validators,
real application calls, observations and native verification remain separate.
Generation does not check or adopt tasks, execute application code, or establish PASS.

```sh
dctx0 check path/to/spec
dctx0 verify init path/to/spec --project path/to/app --target typescript --subject Shop
dctx0 verify scaffold-codecs path/to/spec --project path/to/app \
  --target typescript --subject Shop \
  --integer number --type Shop.Sequence=bigint --option null --enum string
```

The generated `.dctx0/testing/typescript/wire-codecs.ts` imports the adjacent
`contract.ts` supplied by `verify init`. It exports `encode(qualifiedType, nativeValue)`,
`decode(qualifiedType, wireValue)` and `decodeInput(qualifiedType, wireValue)`. Selection accepts repeated exact type or
namespace `--subject` values and includes the complete referenced type closure.
Without selectors, all data declarations are included. Unknown selectors and
unpublished or drifted specifications are rejected, without an implicit check.
Bun/TypeScript is needed to use the output, not to generate it.

## Explicit representations

| Setting | Default | Alternative |
| --- | --- | --- |
| `--integer` | `number`, checked for safe lossless representation | `bigint` |
| `--option` | absent as `null` | absent as `undefined` |
| `--enum` | payloadless alternatives as strings | `{kind: alternative, payload: null}` |

Variants with payload always use `{kind: alternative, payload: nativePayload}`.
Records retain named fields, lists use arrays, Unit uses `undefined`, and UUID and
Timestamp use strings. Decimal retains exact decimal text (digits with an optional fractional part,
no exponent or plus sign; integer/decimal text is bounded at 65,536 bytes); Float and Duration use
finite numbers. Bytes uses an array of numbers in 0..255. Builtin Some, None,
Dctx0Option and Dctx0Result retain their wire wrappers; Result uses tagged native
Success/Failure alternatives. None uses `null` independently of the Option default.
Record fields must be present, even when their native value is `undefined`.

Repeated `--type QUALIFIED_TYPE=REPRESENTATION` overrides the appropriate setting
for an integer alias, option alias or payloadless enum. Overrides on an outer
alias take precedence over those on its aliased type; record fields start their
own type selection. Unsupported mappings, unknown/unselected types and duplicate
CLI overrides are errors. A per-type CLI override can replace that type's config-file
setting. No user-supplied code is evaluated.

`--config codec-config.json` accepts this complete versioned JSON object:

```json
{
  "schema_version": 1,
  "integer": "number",
  "option": "null",
  "enum": "string",
  "types": { "Shop.Sequence": "bigint" }
}
```

CLI flags override file settings. Unknown fields, duplicate keys and unsupported
versions/representations are rejected. Configuration input is bounded at 1 MiB.

## Editing and regeneration

Use `--dry-run --json` to inspect proposed contents, effective settings, generator
version and source snapshot without creating files or locks. Existing files are
preserved. To compare a regenerated proposal with custom edits, use a fresh
basename such as `--output candidate-wire-codecs.ts`; the command never merges or
overwrites an existing file. Output must stay beside `contract.ts` and cannot
replace the adapter, contract, worker or module observer.

The report's artifact is the proposal, even when its status is `preserved`; it
is not a claim that the existing file equals the proposal. No production source
is generated or changed. There is no schema migration or wire-protocol change.

Adapt nonstandard native mappings explicitly. Unsupported builtins and conversion
errors throw TypeError; integers are never rounded. Conversion depth is bounded
at 128. Present options that would collapse into the configured absence sentinel
are rejected on decoding; nested nullable options may require a custom tagged
native representation. This template converts representations, not semantic
ranges, UUID validity, where clauses or business rules. Invoke the real production
validator after decoding. Keep conversion outside application-call exception
classification, for example with the existing `invokeApplication` helper.

## Conformance and measurements

The Rust target tests generate default and configured codecs, typecheck the output
with the installed Cinema TypeScript compiler, and execute independent native/wire
pairs in real Bun. Cases include integer limits and per-type alias overrides,
records, recursive structures, options, variants, bytes, exact decimal text and
rejections. Native helper LCOV and logs are retained separately under
`target/codec-conformance/`; Rust coverage does not measure TypeScript execution.

`python3 scripts/validate.py --coverage --samples 1 --iterations 1` pairs required
validation with release speed/RSS measurements. `verification-codecs` measures
selection and generation, excluding native compilation/execution. The associated
workflow comparison phases are documented in [workflow efficiency](workflow-efficiency.md).
New workloads remain unassessed until an explicitly authorized baseline promotion.

The 2026-10-22 checkpoint passes 77 Bun codec cases and nine isolated release-CLI
representation checks. Conformance retains generated helper coverage separately
from Rust: 217/294 lines and 31/36 functions across two configurations. No full
TypeScript branch-coverage claim follows. See [workflow efficiency](workflow-efficiency.md)
and `docs/workflow-tools-validation.json` for exact scope, native costs, the local
Go Race blocker and the 81 release speed/RSS workloads.

A codec TypeError is a conversion failure, not evidence that a production factory
rejected an input. Use `decodeInput` for contract tests intentionally supplying the wrong native
shape. It preserves missing/extra fields, wrong primitive kinds and unknown enum
alternatives while applying configured native representations where possible.
Pass its result to the real factory; a permissive/broken factory must remain
observable as accepting that invalid input. Keep conversion outside the factory's
exception handler. Conversion failure is never production rejection.

Lossy or unrepresentable inputs (unsafe number integers, foreign variant identity,
payloads in string enums, collapsing options and native exception mappings) still
fail conversion explicitly and need a custom native representation. `decode`
continues to require the contract's structural shape. Existing editable files are
preserved: generate version 2 under another `--output` basename and merge the
new helper into a customized scaffold.
