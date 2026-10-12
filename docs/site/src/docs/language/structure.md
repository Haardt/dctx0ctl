# Structure and boundaries

Structural declarations create the names and containment used by every other model. They answer where a concept belongs before behavior or verification is added.

## `workspace`

`workspace Name { ... }` names the complete compilation, evidence and release boundary. Exactly one workspace must exist across all configured `.dctx0` files. Its name prefixes canonical symbol IDs and snapshot fingerprints cover the merged workspace. Source hashes retain raw file changes separately.

Use a product name for a jointly released modular application, a service name for an independently validated service, or a bounded-context name when it has its own release evidence. A workspace is not automatically a Git repository, IDE workspace, Maven module or npm package.

```dctx0
workspace WorkshopHub {
  domain Workshops {}
}
```

Additional domains may be top-level declarations in other configured files; they still join the single workspace.

## `fragment`

A top-level fragment extends a semantic owner across files. Source files are recursively discovered in sorted order; composition occurs before resolution and type checking.

<!-- dctx0:check -->
```dctx0
workspace Shop {
  domain Commerce {}
}
fragment Commerce {
  value Year = Int { range 1888..2100 }
  module Catalog {
    intent "Offer catalog rules across composed sources."
    surface Rules {
      operation next(input: Int) -> Int {
        pure
        ensures result == input + 1
      }
    }
  }
}
```

## `domain`

`domain Name { ... }` groups cohesive business language. Values, entities, modules, externals, behavior and verification declarations normally live in a domain.

A domain is the first semantic partition below the workspace. It prevents unrelated vocabularies from becoming one flat namespace, while still allowing one release policy to evaluate the whole workspace.

```dctx0
domain Identity {
  intent """
    Define the language and boundaries for user identity, authentication and
    authorization across the workspace.
  """

  module Users {
    intent """Own user identity and lifecycle behavior."""
  }
}

domain Billing { /* subscriptions and invoices */ }
```

An architecture `intent` can describe a domain directly. It explains why the domain boundary exists and what business context its descendants share. The context is inherited by modules, surfaces and operations inside the domain; their more specific intents add to it rather than replace it. Domain intent is not an acceptance criterion, executable rule or release proof.

Split domains by business meaning. Technical layers such as controller, repository or database are implementation structure and do not make good domains.

## `module`

`module Name { ... }` declares the application slice responsible for domain state and behavior. A module connects ownership, required boundaries, offered surfaces, decisions, examples and constraints.

Give each module a direct, non-empty architecture `intent` so its purpose is reviewable. Duplicate ownership is rejected. Model mutation at the owning boundary; production enforcement needs actual observations and source-bound architecture review.

```dctx0
module WorkshopBooking {
  intent """Own publication and registration behavior."""
  owns { entity Workshop event WorkshopPublished }
  requires { WorkshopStore }
  port WorkshopStore { /* infrastructure-facing operations */ }
  surface AdminApi { /* application-facing operations */ }
}
```

The dctx0 module is a domain ownership boundary. It may align with a source directory or Java module through profiles and testing adapters, but it does not have to mirror the build system.

## `external`

`external Name { ... }` describes a capability outside the workspace. It may contain surfaces and operation signatures so the architecture graph can name the outside contract.

```dctx0
external PaymentProvider {
  surface CardPayments {
    operation authorize(request: PaymentRequest)
      -> PaymentReceipt
      ! PaymentDeclined
  }
}
```

An external operation is descriptive. `observe External.Surface.operation` records architectural participation but creates no callable runtime dependency. If application code actually calls the outside system, model an internal `port` and `call` that port; let the port operation `observe` the external capability.

## `surface`

`surface Name { ... }` groups application behavior callable by a transport, another composed boundary or an executable example. Its operations form a stable domain-facing API.

```dctx0
surface RegistrationApi {
  intent """Offer paid registration without exposing storage or provider details."""
  operation register(request: RegistrationDraft) -> RegistrationView
}
```

Surfaces usually exchange `message` and `value` types rather than persistence `entity` records. Production code owns the boundary, types, and composition root. A testing adapter invokes its actual entry points; framework wiring stays outside the semantic specification.

## `port`

`port Name { ... }` declares a runtime boundary needed by application behavior: persistence, provider access or another capability behind dependency inversion.

```dctx0
port WorkshopStore {
  intent """Provide workshop state without exposing the storage technology."""
  operation find(id: WorkshopId) -> Option<Workshop> {
    state observes Workshop by id
  }
  effects { read Workshop }
}
```

A `call Port.operation` records a resolved semantic dependency and observable interaction obligation. Port `read` and `write` effects are terminal persistence witnesses; they do not create synthetic methods. Optional port `state` contracts enable bounded adapter and operation-contract evidence.

## `operation`

`operation name(parameters) -> Result ! Failure` declares one callable action. Parameters and result are typed, and every visible `! FailureName` must resolve to an explicit `failure` declaration.

```dctx0
operation publishWorkshop(id: WorkshopId)
  -> WorkshopView
  ! WorkshopNotFound
  ! InvalidWorkshopState
```

Use `Unit` when success has no payload. The target adapter reports a tagged language-neutral outcome even when the production success representation carries no payload.

An operation may contain preconditions, postconditions or a port state clause. Its following `effects` block attaches to that operation. Port operations define infrastructure-facing contracts; surface operations expose application behavior. The external adapter binds these contracts to actual entry points, without requiring one production factory/class pattern.

## `intent`

`intent """..."""` explains why an architecture boundary exists. It can occur directly in a workspace, domain, module, external, surface, port or operation. Intents become semantic context for tasks and reviews; they are not executable assertions.

Context accumulates from general to specific (`workspace → domain → module → surface → operation`). A specific intent adds context instead of replacing its parents. Intent guides humans, agents and reviews but is not executable behavior, acceptance criteria, a decision or proof.

```dctx0
module Customers {
  intent """Own stable customer identity behind a small public contract."""
}
```

The `intent` nested inside a `review` is different: it instructs only that review and is not inherited architecture intent.

## Explicit transport exchange

Use `surface Public exposes requestReply` to declare a request/reply boundary; `notification` and `stream` name other exchange shapes. This records the semantic exchange and does not automatically create or verify an HTTP endpoint. Domain types and success/failure contracts remain separate from HTTP response mappings. HTTP mappings and actual transport tests belong to the application/testing target. TypeScript/Bun, Go, and Rust execute native tests; Java currently provides profiles only.

## Pure operations

A module-owned surface operation may carry a direct parameterless `pure` clause. This is an obligation to depend only on explicit inputs, including all transitive helpers. It may call only explicitly pure module-owned surface operations and may not use ports, state, events, external observation or call cycles. See [functional-core operations](functional-core.md).
