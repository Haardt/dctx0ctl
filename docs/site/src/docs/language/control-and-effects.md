# Control and effects

The control model connects domain vocabulary to application boundaries. It distinguishes architecture relations from actual runtime interactions and persistence obligations.

## `owns`

Inside a module, `owns { ... }` identifies declarations for which that module is the business owner.

```dctx0
owns {
  value WorkshopId
  entity Workshop
  event WorkshopPublished
}
```

Ownership states mutation responsibility. Duplicate module ownership is rejected; actual production ownership needs corresponding implementation evidence and review. Ownership is semantic, not a source-file assignment; technical profiles and the testing adapter describe implementation structure separately.

## `requires` as a module relation

Inside a module, `requires { ... }` declares dependencies on modules, ports, or external declarations.

```dctx0
requires {
  WorkshopStore
  PaymentPort
  PaymentProvider
}
```

This relation makes the architecture graph explicit. It does not itself generate a method call. Runtime invocation still requires a surface effect such as `call PaymentPort.authorize`. The same word has a second meaning inside an operation: [a Boolean precondition](contracts-and-lifecycles.md).

## `effects`

An `effects { ... }` block attaches immediately to the preceding operation and lists interactions the implementation must exhibit.

```dctx0
operation register(request: RegistrationDraft) -> RegistrationView
effects {
  read Workshop
  write Registration
  call WorkshopStore.find
  call RegistrationStore.insert
  emit RegistrationConfirmed(registrationId: request.id, workshopId: request.workshopId)
}
```

Effects are obligations, not a menu of permitted possibilities. The central native evaluator checks supported interaction obligations against actual observations. Missing observers or unsupported meanings remain UNKNOWN; structural syntax alone cannot prove an interaction occurred.

## `read`

`read Entity` says an operation observes persistent entity state. On a port it describes the persistence boundary. Keep surface effects consistent with the directly called ports and expose actual resource/state observations. Declaring a read does not itself establish that production performed it.

`read` does not generate a `readEntity()` method. Only `call Port.operation` creates the dependency used to perform the read.

## `write`

`write Entity` says an operation creates, updates or removes persistent entity state. Declare the corresponding surface and directly called port effects consistently, then observe actual mutation during native execution.

The containing module must own the entity. Move cross-module mutation behind an operation owned by the other module instead of duplicating ownership.

## `call`

`call Port.operation` declares a real typed runtime dependency.

```dctx0
effects { call WorkshopStore.find }
```

Examples can inject declared collaborator replies through adapter boundaries; native operation assessment evaluates observed interactions. A module dependency is not runtime evidence. Opaque labels such as `calls Port.operation(args) exactly N` remain unmodeled in the current compiler: do not present them as resolved calls or execution proof.

Use `call` for executable application collaboration. A module-level `requires` relation alone is not a call, and `observe` is not a substitute.

## `emit`

`emit Event` declares that the operation publishes a domain event.

```dctx0
effects { emit WorkshopPublished }
```

It records an event obligation. Production publication and adapter event observation supply its actual execution evidence. In examples, contextual `expect emits Event { ... }` checks occurrence and payload. Supported event predicates are assessed against recorded events; incomplete observation remains UNKNOWN.

## `observe`

`observe External.Surface.operation` records that an external capability participates architecturally.

```dctx0
port PaymentPort {
  operation authorize(request: PaymentRequest) -> PaymentReceipt
  effects { observe PaymentProvider.CardPayments.authorize }
}
```

It remains declarative evidence and creates no runtime method. The use case calls `PaymentPort.authorize`; the port implementation integrates with the provider. An external observation alone does not establish behavior. Supply an observable contract where the product needs one. A legitimate stateless Unit operation can use an explicit success/rejection contract without inventing state.

## `state` on a port operation

A port operation may have exactly one constructive `state` clause. It tells bounded conformance and contract tests how the operation relates to entity state.

| Form | Meaning |
| --- | --- |
| `state observes Entity by keyPath` | Read one entity selected by a key |
| `state lists Entity` | Read a collection of entity states |
| `state inserts Entity from parameter` | Insert a new entity constructed from a parameter |
| `state upserts Entity from parameter` | Insert or replace from a parameter |
| `state replaces Entity by keyPath` | Replace existing state selected by a key |
| `state removes Entity by keyPath` | Remove state selected by a key |

`by` identifies an operation parameter or field path from which the entity key can be determined. `from` identifies the parameter that constructively supplies the entity.

### `observes`

`observes` selects one existing entity by a parameter or field-path key and models a read. It commonly pairs with `Option<Entity>` or a message projection and a `read Entity` effect.

### `lists`

`lists` exposes a bounded collection of entity state. It can project to `List<Entity>` or a compatible message containing list fields and derived `count`/`total` values.

### `inserts`

`inserts` creates state that did not exist before. Contextual `from` names the parameter capable of constructing the new entity, and the port operation normally declares `write Entity`.

### `upserts`

`upserts` inserts missing state or replaces existing state from a named parameter. Use it only when both outcomes are genuinely part of the port contract.

### `replaces`

`replaces` requires existing state and replaces it using an entity key obtained through contextual `by`.

### `removes`

`removes` deletes existing state selected through contextual `by`. A `Unit` result is appropriate when no success payload is needed.

```dctx0
port RegistrationStore {
  operation insert(request: RegistrationDraft) -> RegistrationView {
    state inserts Registration from request
  }
  effects { write Registration }
}
```

State contracts provide bounded native evidence from real before/after observations and modeled state. They do not prove arbitrary storage behavior for every possible input.

## Keep effect boundaries consistent

If a surface calls a port that writes `Workshop`, make that state effect visible on the surface as well. Keep reads/writes, called boundaries, and entity ownership aligned with the actual product architecture.

Compilation resolves modeled effects. Native resource/call/event checks require real adapter observations; a declaration or structural inventory cannot prove every production dependency. Use source-bound architecture review for obligations beyond the implemented automatic checks.

## Event payload provenance

A record-shaped event requires named payload fields, for example `emit WorkshopPublished(workshopId: id)`. A scalar event uses a positional payload; a fieldless event needs none. Payload expressions may use operation parameters or explicitly named call results, such as `call Store.find as found` followed by `emit Changed(id: found.id)`. Missing, duplicate or incompatible fields are diagnostics, not inferred values. These effect bindings complement example event assertions.
