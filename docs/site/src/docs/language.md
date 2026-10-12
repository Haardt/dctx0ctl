# Language reference

This reference covers the current Rust language implementation. It follows the parser, validated IR, compiler, interpreter, and working examples. Compiler acceptance and executable verification support remain distinct. Unless marked as complete, snippets are contextual excerpts that require surrounding declarations.

dctx0 is deliberately declarative. A specification states domain vocabulary, ownership, callable boundaries, observable behavior and required evidence. It contains typed declarative expressions and named example results; it does not contain application algorithm bodies, loops, embedded TypeScript, SQL, or HTTP clients.

## The four connected models

| Model | Question | Main declarations |
| --- | --- | --- |
| Domain | What business things exist? | `scalar`, `value`, `type`, `failure`, `entity`, `message`, `event` |
| Control | Who owns them and what may be called? | `module`, `owns`, `requires`, `port`, `surface`, `external`, `operation`, `effects` |
| Behavior | What rules and observable flows hold? | `intent`, `decision`, `constraint`, `example`, `invariant`, operation contracts, `pure`, `lifecycle` |
| Verification | What evidence gates release? | `proof`, `test`, `review`, `policy` |

The models are complementary. An `entity` without an owner leaves mutation responsibility unclear. A surface `operation` without effects hides its collaborations. An effect without an `example`, contract or test suite may be statically verified but not behaviorally exercised. A passing test without a `policy` may still be insufficient for release.

## Canonical statement keywords

The main statement families are listed below; top-level `fragment` additionally extends a semantic owner across source files. Follow the links for purpose, placement, interactions, validation rules and examples.

| Family | Keywords |
| --- | --- |
| Structure and boundaries | [`workspace`, `domain`, `module`, `external`, `surface`, `port`, `operation`, `intent`](language/structure.md) |
| Domain data | [`scalar`, `value`, `type`, `failure`, `entity`, `message`, `event`, `key`, `range`, `length`, `format`, `where`](language/data.md) |
| Ownership and effects | [`owns`, `requires`, `effects`, `read`, `write`, `call`, `emit`, `observe`, `state`](language/control-and-effects.md) |
| Scenarios and pure rules | [`decision`, `choose`, `because`, `avoid`, `constraint`, `example`, `given`, `when`, `expect`, `fails`, `emits`, `invariant`, `forall`, `assert`](language/examples-and-invariants.md) |
| Contracts and state machines | [`requires`, `ensures`, `state`, `lifecycle`, `transition`](language/contracts-and-lifecycles.md) |
| Functional core | [`pure` and invariant calls to bound pure operations](language/functional-core.md) |
| Evidence and policy | [`proof`, `test`, `review`, `inspect`, `criteria`, `question`, `policy`, `require`](language/verification-and-policy.md) |

Some words are contextual rather than statement starters: `returns`, `for`, `using`, `from`, `to`, `initial`, `terminal`, `by`, `exactly`, `in`, `is`, `not`, `and`, `or`, `implies`, `self`, `success`, `result` and the postcondition variable `failure`. They are explained where they participate and summarized in [Expressions and built-ins](language/expressions-and-builtins.md). `returns` is valid only in `given ... returns ...`; operation results always use `->`.

## Source and lexical rules

- Identifiers start with a letter or `_`; following characters may include letters, digits, `_`, `.`, and `-`.
- Qualified identifiers use dots, for example `WorkshopBooking.AdminApi.publishWorkshop`.
- Normal strings use `"..."` and support escapes such as `\n` and `\t`.
- Narrative prose uses triple strings: `"""..."""`.
- Comments use `// ...` or `/* ... */`.
- Integer, decimal and scientific numeric literals are supported. A decimal point must be followed by digits.
- Leaf statements end at a newline or semicolon.
- Declarations continue after `->`, `!`, `|` and contextual `returns`. Record expressions may span lines.  keep an operation parameter list on one physical line.
- `{ ... }` contains declarations or clauses, `( ... )` contains parameters/calls, `[ ... ]` constructs lists and `< ... >` supplies generic type arguments.

## Built-in types

| Type | Purpose |
| --- | --- |
| `String` | Text |
| `Boolean` | `true` or `false` |
| `Int` | Integral number |
| `Float` | Floating-point number |
| `Decimal` | Exact decimal value; use `divide` for explicit rounding |
| `UUID` | UUID identity value |
| `Timestamp` | Point in time |
| `Duration` | Amount of time |
| `Bytes` | Opaque byte data; often needs project samples |
| `Unit` | Successful operation with no payload |
| `Option<T>` | Optional value, constructed with `Some(value)` or `None` |
| `List<T>` | Ordered collection |

Domain contracts normally use `Option<T>` and an operation signature with visible failures. Production targets own their representations; the testing adapter translates them to the versioned SDK values.

## Names, scope and source files

Exactly one named `workspace` must exist across discovered sources. Additional top-level `domain` declarations join that workspace; `fragment QualifiedOwner { ... }` contributes to an existing semantic owner. Logical IDs follow containment:

```text
WorkshopHub.Workshops.WorkshopBooking.RegistrationApi.register
```

Unqualified references resolve by lexical proximity. Use a sufficiently qualified name when short names collide. Split files by cohesive domain or submodel, not into technical buckets such as `entities.dctx0` and `operations.dctx0` that separate related language.

## Choose the right behavior construct

| Claim | Construct | Evidence |
| --- | --- | --- |
| One concrete business flow | `example` | Executed against the actual adapter-bound operation |
| Pure rule over generated values | `invariant` | Finite property cases |
| Operation pre-/postcondition | `requires` / `ensures` | Generated operation cases and state/trace snapshots |
| Allowed entity state sequence | `lifecycle` | Bounded generated traces |
| Important non-mechanical demand | `constraint` | Assigned source-bound conformance review |
| Contextual quality judgment | `review` | Structured evidence-bound review |
| Why a boundary exists | architecture `intent` | Inherited review and implementation context, not proof |

Resolve conflicting narrative and executable requirements explicitly. Do not invent a product decision or rewrite a contract to fit a failing implementation.

## Language limits are a feature

The built-in expression and function catalog is closed. Plugins cannot add evaluators, and a specification cannot escape into implementation code. This makes parsing, canonical compilation, generation and mechanical checks reproducible for identical inputs and configuration. Put algorithms and infrastructure in application code; use dctx0 to state the observable contract they must satisfy.
