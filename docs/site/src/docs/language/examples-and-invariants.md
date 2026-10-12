# Examples and invariants

The behavior model separates concrete scenarios, pure general rules and narrative guidance. Choosing the correct construct determines what can execute and what kind of evidence it produces.

## `decision`

`decision Name { ... }` records an architecture or domain design choice in the context of its parent.

```dctx0
decision RetrySafeRegistration {
  choose """Use a caller-supplied RegistrationId."""
  because """Retries retain stable business identity."""
  avoid """Do not derive identity from email."""
}
```

Decisions become review and implementation context. They do not execute and should not be used to hide behavior that belongs in an example, contract or invariant.

## `choose`

`choose` states the selected option inside a `decision`. It answers what direction was taken. Use narrative triple strings for readable prose.

## `because`

`because` records the reason for the selected option. It connects trade-off reasoning to the choice without pretending that the reason is mechanically proven.

## `avoid`

`avoid` records a rejected direction or failure mode. It gives reviewers and implementation agents an explicit negative boundary. It does not ban code mechanically unless another contract or verifier rule represents the same demand.

## `constraint`

`constraint` records an important quality or business demand that is not expressed by the closed executable language.

```dctx0
constraint """A declined payment must never create a registration."""
```

A triple-string constraint is narrative; a short identifier is structured. Both remain narrative requirements and receive assigned source-bound conformance review obligations. Do not label a constraint as proof and do not force subjective prose into a fake invariant.

## `example`

`example "name" { ... }` describes one concrete business flow. It executes the real adapter-bound application operation with controlled port results and a fresh event recorder.

```dctx0
example "existing customer" {
  given CustomerStore.find(customerId)
    returns Some(Customer { id = customerId status = Active })
  when result = Public.getCustomer(customerId)
  expect result == CustomerView { id = customerId status = Active }
  expect emits CustomerViewed { customerId = result.id }
}
```

Every example needs at least one expectation. The runtime checks declared interaction order and rejects missing, additional or unexpected port calls and events. Use examples for named success/failure paths, not as a substitute for a general property.

## `given`

`given Port.operation(arguments) returns value` stubs one declared port call. The invocation must match an effect reachable from the tested surface operation.

Supported values include scalar literals, variables, record construction, enum variants, `Some(value)`, `None` and failures supported by the test plan. Stubs are consumed deterministically; they are not arbitrary mock code.

## `returns`

`returns` connects a `given` invocation to its controlled result.

```dctx0
given WorkshopStore.find(workshopId) returns None
```

`returns` is reserved for example stubs. Operation results use `->`.

## `when` in an example

`when resultVariable = Surface.operation(arguments)` selects the single surface invocation under test and names its tagged result.

```dctx0
when result = AdminApi.publishWorkshop(workshopId)
```

The result variable is then available to expectations and field access such as `result.id`. `when` also appears in lifecycle transitions, where it introduces a Boolean transition guard.

## `expect`

`expect` introduces a scenario assertion. Common forms are:

```dctx0
expect result == WorkshopView { id = workshopId status = Published }
expect result fails WorkshopNotFound
expect Success
expect emits WorkshopPublished { workshopId = workshopId }
```

The exact result form compares structured values. `Success` asserts a successful envelope without requiring a payload value. Executed expectations contribute finite native evidence.

## `fails`

Contextual `fails` selects an expected tagged failure, normally after `expect result`:

```dctx0
expect result fails WorkshopNotFound
```

The failure must be declared on the invoked operation. It is distinct from the `failure` declaration keyword and the lowercase postcondition variable `failure`.

## `emits` in an example

Contextual `emits` asserts that a declared event was recorded, optionally with a full payload:

```dctx0
expect emits RegistrationConfirmed {
  registrationId = registrationId
}
```

The operation must declare `emit RegistrationConfirmed` in its effects. The same word is a trace predicate inside `ensures`, where it is normally paired with `exactly N`.

## `invariant`

`invariant Name { ... }` declares a pure, target-neutral property over generated values.

```dctx0
invariant RemainingSeatsMatchRegistrationCount {
  forall capacity: SeatCount
  forall registered: Int
  assert registered >= 0 and registered <= capacity implies
    CapacityRules.remainingSeats(capacity, registered) == capacity - registered
}
```

Invariants may call explicitly pure module-owned surface operations. They cannot call ports or unmarked/stateful operations, or access I/O, time or randomness. Generation uses a deterministic seed and finite budgets, so a successful run establishes finite testing, not a mathematical proof.

## `forall`

`forall name: Type` introduces one typed generated variable inside an invariant. Multiple declarations form the input space for every assertion.

Refined values constrain generation. If no valid value can be generated within the configured budgets, the result is `UNKNOWN`, not an invented test case.

## `assert`

`assert BooleanExpression` adds a pure property to an invariant. At least one assertion is required for meaningful evidence, and the compiler verifies that its result type is `Boolean`.

Use collection binders and the closed built-ins for domain rules. For functional-core calls, mark and bind the operation as `pure`; for stateful operation behavior, use `requires`/`ensures`. Subjective judgment belongs in `constraint` or `review`.

## Scenario versus property

An executed example establishes its named observation within the controlled scenario. An invariant generalizes a pure rule over finitely generated inputs and can execute bound pure operations. Use both when a central rule benefits from a readable example and broader generated coverage.
