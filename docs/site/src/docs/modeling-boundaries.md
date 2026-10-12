# Modeling boundaries

The dctx0 workspace is a logical validation boundary. Do not derive it mechanically from the repository layout.

## Modular monolith

A modular Java or TypeScript monolith normally uses one workspace for the complete application:

```dctx0
workspace CommercePlatform {
  domain Identity {
    module Users { intent "Own user identity and its lifecycle." }
    module AccessControl { intent "Own authorization rules." }
  }

  domain Sales {
    module Orders { intent "Own order placement and fulfillment." }
  }
}
```

Technical Maven, Gradle, JPMS or package-manager modules are implementation structure. A dctx0 module expresses domain ownership. The boundaries may align, but they are not the same concept.

## Microservices in a monorepo

Independently deployed and validated services should normally use separate dctx0 project roots:

```text
platform/
├── identity-service/
│   ├── dctx0/
│   └── .dctx0/testing/verification.json
└── billing-service/
    ├── dctx0/
    └── .dctx0/testing/verification.json
```

Each project then has one workspace, such as `IdentityService` or `BillingService`, with independent hashes and release evidence.

## Boundary rule

- Shared implementation and release policy: one workspace.
- Independent deployment and validation: separate workspace project.
- Technical build module only: usually not a workspace.
- Business capability inside a monolith: domain or dctx0 module.

Composition across independently validated workspaces is a separate concern and should not weaken the single-workspace compilation rule.

