# Domain data

Domain declarations give names and types to business concepts. Production code owns its types; runtime contract validation and the external adapter enforce/translate the selected language-neutral values.

## `scalar`

`scalar Name = Type` creates a named alias for a technical primitive or externally defined atomic value.

```dctx0
scalar ExternalCode = String
```

Use it when the main value is technical identity of the representation. Prefer `value` when the name carries domain meaning or needs refinements.

## `value`

`value` declares a domain value with value equality. It can alias another type, refine an alias, or define a record-shaped value.

```dctx0
value WorkshopId = UUID

value Email = String {
  length 3..254
  format Email
}

value Money {
  amount: Decimal
  currency: String
  where amount >= 0.0
}
```

Values are the preferred way to stop raw primitives from losing business meaning. Refinements influence checking, runtime contract validation and property input generation.

## `type`

`type` declares an enum or tagged union. `|` separates variants; a payload in parentheses makes a variant tagged and typed.

```dctx0
type WorkshopStatus = Draft | Published | Cancelled
type PaymentOutcome = Authorized(ExternalCode) | Declined(String)
```

Payload-free variants such as `Published` resolve as typed values in examples, invariants and contracts. Do not quote them. Tagged unions make mutually exclusive outcomes explicit without relying on nullable fields.

## `failure`

`failure Name { fields }` declares a visible, tagged operation failure. Every name following `!` in an operation signature must resolve to one of these declarations.

```dctx0
failure WorkshopNotFound { workshopId: WorkshopId }
failure PersistenceUnavailable { reason: String }
```

Failures are caller-visible domain outcomes, not exception classes, stack traces or logging details. In an `ensures` clause, lowercase `failure` is also the postcondition variable holding the actual tagged failure; `failure is WorkshopNotFound` tests its variant.

## `entity`

`entity Name { fields }` models a business object with stable identity, ownership and possibly a lifecycle.

```dctx0
entity Workshop {
  id: WorkshopId
  status: WorkshopStatus
  key id
}
```

An entity used by state or lifecycle checks needs one valid key declaration. That declaration may contain multiple fields for composite identity. Modules use `owns` to establish mutation responsibility; `read` and `write` effects connect operations to that state. Prefer boundary `message` types over returning persistence entities from public surfaces.

## `message`

`message Name { fields }` declares immutable input or output data at a boundary.

```dctx0
message WorkshopView {
  id: WorkshopId
  status: WorkshopStatus
}
```

Messages decouple surface contracts from persistence shape. A port state contract may return a message read model: conformance tests project matching entity/parameter fields, compatible lists and derived `count` or `total` fields, then fill remaining fields with bounded type-valid values.

## `event`

`event Name { fields }` declares an observable published domain fact.

```dctx0
event WorkshopPublished { workshopId: WorkshopId }
```

A surface operation declares `emit WorkshopPublished` in its effects. This generates an event-publisher dependency and lets examples or contracts verify recorded events. Events describe facts that occurred; they are not commands or internal logging messages.

## Record fields

The current field syntax is `name: Type` inside `value`, `failure`, `entity`, `message` or `event` records:

```dctx0
message RegistrationDraft {
  id: RegistrationId
  email: Email
}
```

## `key`

`key fieldName` selects entity identity. A single key declaration can list several fields, such as `key filmId memberId` in Cinema.

```dctx0
entity Registration {
  id: RegistrationId
  key id
}
```

The key must name a declared field. State contracts use `by` paths to locate it, and lifecycle execution uses it to correlate before/after state. Add a key when the entity participates in either feature; use one key declaration, including all composite fields when required.

## `range`

`range minimum..maximum` applies an inclusive numeric refinement.

```dctx0
value SeatCount = Int { range 1..500 }
```

The minimum may not exceed the maximum. Generation uses the boundary to create valid values and runtime checks enforce it.

## `length`

`length minimum..maximum` applies an inclusive size refinement to `String` or `List<T>` values.

```dctx0
value CandidateIds = List<WorkshopId> { length 1..4 }
```

If the required minimum exceeds `the configured collection budget`, the runner reports non-generatable `UNKNOWN` evidence rather than inventing an invalid value.

## `format`

`format Name` applies one of the closed formats: `Email`, `UUID`, `Uri`, `Date`, or `DateTime`.

```dctx0
value ContactEmail = String { format Email }
```

Format refines validation and generation. It does not add arbitrary regex execution or a user-extensible format registry.

## `where`

Inside a `value`, `where BooleanExpression` adds a typed refinement. `self` refers to an aliased value; record-shaped values may also reference their fields.

```dctx0
value PositiveAmount = Decimal { where self > 0.0 }
value Money { amount: Decimal where amount >= 0.0 }
```

`where` also appears contextually inside collection binders such as `count(item in items where item.active)`. In both roles it filters the admissible set, but a value refinement constrains construction while a collection predicate filters elements during expression evaluation.

## Type interaction rules

- All references must resolve across the merged workspace.
- Generic types use angle brackets: `Option<Customer>`, `List<FilmId>`.
- `Some(value)` and `None` construct option values in examples and expressions.
- Use `Decimal` for money or exact decimals. `/` is rejected for Decimal division because scale and rounding would be hidden; use `divide(left, right, scale, mode)`.
- Supply reviewed typed fixtures for symbolic example inputs. The core owns deterministic corpus generation; adapters must not supply their own business expectations or fabricate assertions.
