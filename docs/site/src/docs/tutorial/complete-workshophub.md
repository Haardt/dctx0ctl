# Complete WorkshopHub specification

This assembled example is intentionally shown in one file. A real project may keep `workspace WorkshopHub {}` in `workspace.dctx0` and the top-level `domain Workshops` in another configured source.

<!-- dctx0:check -->
```dctx0
workspace WorkshopHub {
  domain Workshops {
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

    failure WorkshopNotFound { workshopId: WorkshopId }
    failure CapacityReached { workshopId: WorkshopId }
    failure InvalidWorkshopState { workshopId: WorkshopId }
    failure PaymentDeclined { reason: String }
    failure PaymentUnavailable { reason: String }
    failure PersistenceUnavailable { reason: String }

    entity Workshop {
      id: WorkshopId
      title: String
      capacity: SeatCount
      registered: Int
      status: WorkshopStatus
      key id
    }
    entity Registration {
      id: RegistrationId
      workshopId: WorkshopId
      participantId: ParticipantId
      email: Email
      status: RegistrationStatus
      key id
    }

    message WorkshopView {
      id: WorkshopId
      title: String
      capacity: SeatCount
      registered: Int
      status: WorkshopStatus
    }
    message RegistrationDraft {
      id: RegistrationId
      workshopId: WorkshopId
      participantId: ParticipantId
      email: Email
      status: RegistrationStatus
    }
    message RegistrationView {
      id: RegistrationId
      workshopId: WorkshopId
      participantId: ParticipantId
      email: Email
      status: RegistrationStatus
    }
    message PaymentRequest {
      registrationId: RegistrationId
      amount: Money
    }
    message PaymentReceipt { paymentId: PaymentId }

    event WorkshopPublished { workshopId: WorkshopId }
    event WorkshopCancelled { workshopId: WorkshopId }
    event RegistrationConfirmed {
      registrationId: RegistrationId
      workshopId: WorkshopId
    }

    external PaymentProvider {
      surface CardPayments {
        operation authorize(request: PaymentRequest)
          -> PaymentReceipt
          ! PaymentDeclined
          ! PaymentUnavailable
      }
    }

    module WorkshopBooking {
      intent """
        Own workshop publication and paid registration while keeping storage
        and payment-provider details behind explicit ports.
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
        event WorkshopCancelled
        event RegistrationConfirmed
      }
      requires {
        WorkshopStore
        RegistrationStore
        PaymentPort
        PaymentProvider
      }

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

        operation cancel(id: WorkshopId)
          -> WorkshopView
          ! WorkshopNotFound
          ! InvalidWorkshopState
          ! PersistenceUnavailable {
          state replaces Workshop by id
        }
        effects { write Workshop }
      }

      port RegistrationStore {
        operation insert(request: RegistrationDraft)
          -> RegistrationView
          ! PersistenceUnavailable {
          state inserts Registration from request
        }
        effects { write Registration }
      }

      port PaymentPort {
        operation authorize(request: PaymentRequest)
          -> PaymentReceipt
          ! PaymentDeclined
          ! PaymentUnavailable
        effects { observe PaymentProvider.CardPayments.authorize }
      }

      surface AdminApi {
        operation getWorkshop(id: WorkshopId)
          -> WorkshopView
          ! WorkshopNotFound
          ! PersistenceUnavailable
        effects {
          read Workshop
          call WorkshopStore.find
        }

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

        operation cancelWorkshop(id: WorkshopId)
          -> WorkshopView
          ! WorkshopNotFound
          ! InvalidWorkshopState
          ! PersistenceUnavailable {
          requires exists Workshop(id)
          ensures success implies updates Workshop(id)
          ensures failure is InvalidWorkshopState implies preserves Workshop(id)
          ensures success implies emits WorkshopCancelled() exactly 1
        }
        effects {
          write Workshop
          call WorkshopStore.cancel
          emit WorkshopCancelled(workshopId: id)
        }
      }

      surface RegistrationApi {
        operation register(workshopId: WorkshopId, registration: RegistrationDraft, payment: PaymentRequest)
          -> RegistrationView
          ! WorkshopNotFound
          ! CapacityReached
          ! PaymentDeclined
          ! PaymentUnavailable
          ! PersistenceUnavailable
        effects {
          read Workshop
          write Registration
          call WorkshopStore.find
          call PaymentPort.authorize
          call RegistrationStore.insert
          emit RegistrationConfirmed(registrationId: registration.id, workshopId: workshopId)
        }
      }

      constraint """Only published workshops accept registrations."""
      constraint """A workshop never has more confirmed registrations than seats."""
      constraint """A declined payment never creates a registration."""

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

      example "unknown workshop" {
        given WorkshopStore.find(workshopId) returns None
        when result = AdminApi.getWorkshop(workshopId)
        expect result fails WorkshopNotFound
      }

      example "successful paid registration" {
        given WorkshopStore.find(workshopId)
          returns Some(Workshop {
            id = workshopId
            title = "Executable Specifications"
            capacity = 20
            registered = 7
            status = Published
          })
        given PaymentPort.authorize(payment)
          returns PaymentReceipt { paymentId = paymentId }
        given RegistrationStore.insert(registration)
          returns RegistrationView {
            id = registrationId
            workshopId = workshopId
            participantId = participantId
            email = "ada@example.com"
            status = Confirmed
          }
        when result = RegistrationApi.register(workshopId, registration, payment)
        expect result == RegistrationView {
          id = registrationId
          workshopId = workshopId
          participantId = participantId
          email = "ada@example.com"
          status = Confirmed
        }
        expect emits RegistrationConfirmed {
          registrationId = registrationId
          workshopId = workshopId
        }
      }

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
    }

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
  }
}
```

This is a compiler-validated contract, not a native application acceptance. The capacity invariant calls a real pure application operation; native execution is needed to assess it. Symbolic example values need typed fixtures, and stateful native checks need actual observers.

Build the application and a testing-only adapter before running it. Use [the current workflow](../workflow.md) and [TypeScript adapters](../typescript-adapters.md). For a complete executable teaching series, see [MiniBoard](../miniboard.md).
