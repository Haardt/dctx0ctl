# Tutorial: build WorkshopHub

This tutorial builds a realistic specification for `WorkshopHub`: workshops are published and cancelled, participants register after payment authorization, ports expose constructive state, and release policy combines mechanical, tested and reviewed evidence.

The goal is not merely valid syntax. Each step adds one of the four connected models and explains why the relationship belongs in the specification.

## 1. Create the project boundary

```text
workshop-hub/
├── package.json
├── tsconfig.json
├── dctx0/
│   ├── workspace.dctx0
│   └── workshops.dctx0
├── src/
└── .dctx0/testing/       # external adapter, fixtures, native artifacts
```

Use exactly one workspace across configured sources:

```dctx0
// dctx0/workspace.dctx0
workspace WorkshopHub {}
```

```dctx0
// dctx0/workshops.dctx0
domain Workshops {
  // the rest of this tutorial
}
```

Top-level domains in separate files join the same logical workspace. Fully qualified IDs follow containment, for example `WorkshopHub.Workshops.WorkshopBooking.AdminApi.publishWorkshop`.

## 2. Establish domain vocabulary

Model identifiers, refined values and finite variants before operations:

```dctx0
value WorkshopId = UUID
value RegistrationId = UUID
value ParticipantId = UUID
value PaymentId = UUID

value Email = String {
  length 3..254
  format Email
}

value SeatCount = Int { range 1..500 }

value Money {
  amount: Decimal
  currency: String
  where amount >= 0.0
}

type WorkshopStatus = Draft | Published | Cancelled
type RegistrationStatus = Confirmed | Cancelled
```

Names such as `WorkshopId` stop raw UUIDs from becoming interchangeable. Refinements guide validation and generated values. Enums make states declaration-aware values used consistently by examples and contracts.

## 3. Separate identity, boundary data and facts

```dctx0
entity Workshop {
  id: WorkshopId
  title: String
  capacity: SeatCount
  registered: Int
  status: WorkshopStatus
  key id
}

message WorkshopView {
  id: WorkshopId
  title: String
  capacity: SeatCount
  registered: Int
  status: WorkshopStatus
}

event WorkshopPublished { workshopId: WorkshopId }
failure WorkshopNotFound { workshopId: WorkshopId }
```

The entity represents owned persistent identity. The message is a stable boundary projection. The event is an observable fact, and the failure is a caller-visible outcome. The key is mandatory once the entity participates in port state or a lifecycle.

## 4. Describe the outside system without coupling to it

```dctx0
external PaymentProvider {
  surface CardPayments {
    operation authorize(request: PaymentRequest)
      -> PaymentReceipt
      ! PaymentDeclined
      ! PaymentUnavailable
  }
}
```

This gives the external capability an architectural name. Application code should still depend on an internal port:

```dctx0
port PaymentPort {
  operation authorize(request: PaymentRequest)
    -> PaymentReceipt
    ! PaymentDeclined
    ! PaymentUnavailable
  effects {
    observe PaymentProvider.CardPayments.authorize
  }
}
```

The use case calls `PaymentPort.authorize`; the port adapter integrates with the provider. `observe` preserves the architectural relation without generating a provider method directly.

## 5. Give the module ownership and intent

```dctx0
module WorkshopBooking {
  intent """
    Own workshop publication and paid registration while keeping storage and
    payment-provider details behind explicit ports.
  """

  decision RetrySafeRegistration {
    choose """Use a caller-supplied RegistrationId."""
    because """A retried request can retain stable business identity."""
    avoid """Do not use an email address as registration identity."""
  }

  owns {
    value WorkshopId
    value RegistrationId
    entity Workshop
    entity Registration
    event WorkshopPublished
    event RegistrationConfirmed
  }

  requires {
    WorkshopStore
    RegistrationStore
    PaymentPort
    PaymentProvider
  }
}
```

`intent` explains the boundary. `decision` preserves an important trade-off. `owns` establishes write authority, while module `requires` exposes architectural dependencies. None of these declarations alone executes behavior.

## 6. Make persistence constructive

```dctx0
port WorkshopStore {
  operation find(id: WorkshopId)
    -> Option<Workshop>
    ! PersistenceUnavailable {
    state observes Workshop by id
  }
  effects { read Workshop }

  operation publish(id: WorkshopId)
    -> WorkshopView
    ! WorkshopNotFound
    ! InvalidWorkshopState
    ! PersistenceUnavailable {
    state replaces Workshop by id
  }
  effects { write Workshop }
}
```

Effects describe persistence obligations. State clauses give the core a model for bounded native checks with actual before/after observations. `read` and `write` generate no methods—the methods are `find` and `publish` themselves.

## 7. Offer use cases through a surface

```dctx0
surface AdminApi {
  operation publishWorkshop(id: WorkshopId)
    -> WorkshopView
    ! WorkshopNotFound
    ! InvalidWorkshopState
    ! PersistenceUnavailable {
    requires exists Workshop(id)
    ensures success implies updates Workshop(id)
    ensures success implies emits WorkshopPublished() exactly 1
  }
  effects {
    write Workshop
    call WorkshopStore.publish
    emit WorkshopPublished(workshopId: id)
  }
}
```

The signature is caller-visible. Effects state actual collaborations. Supported preconditions and postconditions exercise generated cases against actual observations. The surface `write Workshop` exactly mirrors the directly called port operation's write.

## 8. Pin down concrete behavior

```dctx0
example "publish a draft workshop" {
  given WorkshopStore.publish(workshopId)
    returns WorkshopView {
      id = workshopId
      title = "Executable Specifications"
      capacity = 20
      registered = 0
      status = Published
    }
  when result = AdminApi.publishWorkshop(workshopId)
  expect result == WorkshopView {
    id = workshopId
    title = "Executable Specifications"
    capacity = 20
    registered = 0
    status = Published
  }
  expect emits WorkshopPublished { workshopId = workshopId }
}
```

The managed worker invokes the actual adapter-bound application. Declared collaborator replies control the scenario; the Rust core assesses its observed results and events. Missing observers and unsupported obligations remain explicit gaps.

Add explicit failure paths too:

```dctx0
example "unknown workshop" {
  given WorkshopStore.find(workshopId) returns None
  when result = AdminApi.getWorkshop(workshopId)
  expect result fails WorkshopNotFound
}
```

## 9. Generalize pure rules

```dctx0
surface CapacityRules {
  operation remainingSeats(capacity: SeatCount, registered: Int) -> Int {
    pure
  }
}

invariant RemainingSeatsMatchRegistrationCount {
  forall capacity: SeatCount
  forall registered: Int
  assert registered >= 0 and registered <= capacity implies
    CapacityRules.remainingSeats(capacity, registered) == capacity - registered
}
```

This rule calls the actual pure application operation for admissible registration counts. The algorithm stays in application code; generated native cases check that remaining capacity matches the domain contract. The two declarations belong inside `WorkshopBooking`.

An invariant over arbitrary input lists does not itself enforce persistence uniqueness. A one-registration-per-participant rule needs an operation/state contract or an assigned review supported by real persistence checks.

## 10. Add the entity lifecycle

```dctx0
lifecycle WorkshopLifecycle for Workshop using status {
  state Draft initial
  state Published
  state Cancelled terminal

  transition WorkshopBooking.AdminApi.publishWorkshop
    from Draft
    to Published
    when true

  transition WorkshopBooking.AdminApi.cancelWorkshop
    from Published
    to Cancelled
    when true
}
```

The compiler validates the declared lifecycle. The core projects connected finite paths; actual execution needs reviewed inputs and state observers. A compiled lifecycle does not by itself prove the transitions.

## 11. Preserve non-mechanical demands

```dctx0
constraint """A declined payment never creates a registration."""

review Architecture {
  intent """Check boundaries that are not completely mechanical."""
  inspect {
    implementation
    dependencyGraph
    decisions
    surfaces
    ownership
    contracts
  }
  criteria {
    "Storage and payment implementations remain behind declared ports."
    "Public messages contain no provider-specific payment fields."
  }
  question BoundaryIntegrity {
    """Does the implementation preserve the declared boundaries?"""
  }
}
```

The constraint is a narrative requirement. The current planner assigns manual obligations to Review tasks; a reviewer inspects actual sources and records bound reports. Review judgment stays separate from native testing.

## 12. Gate release with explicit evidence

```dctx0
proof TypeConformance
proof SurfaceConformance
proof ArchitectureConformance
proof OwnershipConformance
proof EffectConformance
test Examples
test Properties
test AdapterConformance
test ExternalConformance

policy Release {
  require TypeConformance == PASS
  require SurfaceConformance == PASS
  require ArchitectureConformance == PASS
  require OwnershipConformance == PASS
  require EffectConformance == PASS
  require Examples == PASS
  require Properties == PASS
  require AdapterConformance == PASS
  require ExternalConformance != FAIL
  require Architecture != REQUEST_CHANGES
  require Architecture.highFindings == 0
}
```

These suite names declare requirements; their presence does not imply every automatic checker exists. Native execution, central suite aggregation, and assigned source-bound reviews establish the current effective result. An explicit gate cannot excuse an executed failure.

## 13. Check, plan, and connect the application

```sh
dctx0 check /path/to/workshop-hub/dctx0
dctx0 tasks plan /path/to/workshop-hub/dctx0
dctx0 verify init /path/to/workshop-hub/dctx0 --project /path/to/workshop-hub \
  --target typescript --subject WorkshopHub.Workshops.WorkshopBooking
dctx0 verify plan /path/to/workshop-hub/dctx0 --project /path/to/workshop-hub
```

The [assembled WorkshopHub specification](tutorial/complete-workshophub.md) is compiler-checked by this site's validation runner. It is a modeling tutorial, not a shipped native WorkshopHub application. Symbolic inputs need reviewed typed fixtures; native execution needs an actual application, adapter, and observers.

After implementing those boundaries, use `verify run --no-adopt` and the ordinary review/task workflow. Continue with [TypeScript adapters](typescript-adapters.md), or use [MiniBoard](miniboard.md) for sixteen applications with retained execution evidence.
