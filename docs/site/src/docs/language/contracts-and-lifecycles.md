# Contracts and lifecycles

Operation contracts generalize executable behavior around one call. Lifecycles connect operations into bounded state traces for a keyed entity.

## `requires` as an operation precondition

Inside an operation body, `requires BooleanExpression` restricts valid generated inputs and before-state.

```dctx0
operation publishWorkshop(id: WorkshopId) -> WorkshopView {
  requires exists Workshop(id)
}
```

Parameters are in scope. Generated-case selection and native setup must satisfy the precondition. Rejected/unsupported setup and an empty valid corpus stay visible; no valid execution means UNKNOWN. Keep preconditions generatable with refinements, bounded collections and optional value-only samples.

This use is distinct from module `requires { Port }`, which declares architecture dependencies.

## `ensures`

`ensures BooleanExpression` declares a postcondition over the operation result, failure, state snapshots or recorded traces.

```dctx0
ensures success implies updates Workshop(id)
ensures failure is InvalidWorkshopState implies preserves Workshop(id)
ensures success implies calls WorkshopStore.publish(id) exactly 1
```

Parameters remain in scope. Postconditions additionally receive `success`, `result` and `failure`. State-dependent contracts need constructive state from the relevant called ports. Stateless Unit authorization with an explicit success contract needs no state clause or artificial handoff. State-dependent checks require the adapter and modeled boundaries to expose the relevant entities.

## Postcondition variables

- `success` is Boolean and states whether the tagged result is successful.
- `result` is the success payload and supports field access.
- `failure` is the tagged failure value.
- `failure is FailureName` tests a declared variant.

These are contextual expression names, not top-level declarations.

## State predicates

State predicates compare modeled expectations with actual before/after snapshots and use an entity name plus key value:

| Predicate | Meaning |
| --- | --- |
| `exists Entity(key)` | Entity exists in the relevant snapshot |
| `creates Entity(key)` | Missing before, present after |
| `updates Entity(key)` | Present before and changed after |
| `preserves Entity(key)` | State remains unchanged |
| `removes Entity(key)` | Present before, missing after |

```dctx0
requires exists Customer(id)
ensures success implies updates Customer(id)
```

These predicates need constructive state from directly called port operations. Unsupported projections yield `UNKNOWN` instead of fabricated evidence.

## Trace predicates: `calls`, `emits`, `exactly`

Supported event predicates use recorded observations. The accepted `calls Port.operation(args) exactly N` label remains explicitly opaque/unmodeled in the Rust compiler. It is not a resolved dependency or proof of recorded calls; use declared `call` effects and real interaction observers for native evidence.

```dctx0
ensures calls CustomerStore.find(id) exactly 1
ensures emits CustomerViewed() exactly 1
```

The preceding trace examples are syntax excerpts, not a claim that every trace form is executable. Inspection cannot manufacture missing call observations.

## `lifecycle`

`lifecycle Name for Entity using discriminator { ... }` declares a finite state machine for a keyed entity.

```dctx0
lifecycle WorkshopLifecycle for Workshop using status {
  state Draft initial
  state Published
  state Cancelled terminal

  transition WorkshopBooking.AdminApi.publishWorkshop
    from Draft
    to Published
    when true
}
```

`for` identifies the entity. `using` selects either an entity field or contextual `presence`. A field discriminator maps declared states to field variants; `presence` models creation/removal state without a status field.

## `state` in a lifecycle

`state Name` declares a lifecycle node. Add `initial` to exactly one state and `terminal` to a state after which generated traces should stop.

This is a different context from `state observes|lists|...` on a port operation. Both model state, but lifecycle state is a named node while port state is constructive adapter semantics.

## `initial`

`initial` marks the single starting state. Exactly one is required. Generated lifecycle traces seed the entity in this state and begin transition selection there.

## `terminal`

`terminal` marks a state at which trace execution ends. A lifecycle can have multiple terminal states. Non-terminal dead ends are permitted as stopping points but should represent deliberate domain behavior.

## `transition`

`transition QualifiedOperation from State to State when condition` connects an existing operation to a state change.

The referenced operation and both states must exist. Projection constructs paths from the initial state and retains unreachable transition work as explicit issues. Compiler acceptance alone does not establish coverage of the lifecycle.

## `from` and `to`

`from` names the required source state; `to` names the resulting state. Both are contextual parts of a `transition`, not standalone statements.

## `when` in a transition

`when BooleanExpression` guards a transition. Operation parameters are available to the expression. If rejection is a caller-visible business outcome, declare its failure in the operation contract and provide explicit rejection evidence.

`when` is optional in the lifecycle IR, although writing `when true` can make an unconditional transition obvious. In examples, `when` instead introduces the surface invocation.

## Generated lifecycle semantics

The core projects connected finite lifecycle paths within configured budgets. Actual execution needs reviewed inputs and state observers, and follows the ordered steps in a shared native session. A thrown operation, invalid result envelope, postcondition failure, state mismatch or lifecycle violation produces `FAIL`; a non-constructive state model produces `UNKNOWN`.

Lifecycle and operation-contract results remain finite native evidence.

## Stateless authorization and idempotence

A permission check may succeed with Unit without persisting state or calling another capability:

```dctx0
operation ensure(allowed: Boolean) -> Unit ! Denied {
  ensures success == allowed
}
```

The failure type `Denied` must be declared in the surrounding domain. The contract distinguishes success and rejection; it does not invent a successful state mutation. Supported direct success and rejection examples can also establish behavior.

For idempotent operations, keep existing business states and express the allowed transition and preserved already-completed state. Both first and repeated calls may return the same confirmed view when the product does not need different result variants. Conditional obligations use existing Boolean conditions and `implies`; changing result types is a separate domain contract decision.

Lifecycle execution needs actual results and state observations consistent with the modeled contracts. A planner projection does not authorize an invented production result. Unsupported derivations remain UNKNOWN, with a cause and next step. Rejected calls preserve state and do not count as successful lifecycle transitions. A checker limitation can be inspected through scoped agent evidence rather than changing the domain merely to make the checker pass.
