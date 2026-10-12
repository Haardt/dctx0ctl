# Verification and policy

Declarations state which evidence matters. The implementation keeps automatic native results, finite scope, and authored judgments distinguishable.

## `proof` and `test`

A named `proof` or `test` suite is a requirement. A declaration does not automatically implement a checker with that name.

```dctx0
proof TypeConformance
proof OwnershipConformance
test Examples
test Properties
test AdapterConformance
```

Current Rust assessment recognizes its implemented proof categories and finite execution suite membership. Examples, properties, contract/value/operation cases, and lifecycles retain their actual required cases. Undefined or unsupported suite requirements remain UNKNOWN or require an assigned source-bound review; they do not default to PASS.

Do not advertise arbitrary `proof` suites as mathematical proofs of business correctness. Finite property cases establish only their tested scope.

## `review`

Reviews name quality obligations that require explicit judgment:

```dctx0
review Architecture {
  intent "Check conceptual boundary integrity."
  inspect {
    implementation
    dependencyGraph
    decisions
    surfaces
    ownership
    contracts
  }
  criteria { "Persistence remains behind the declared port." }
  question BoundaryIntegrity {
    "Does the implementation preserve the module boundary?"
  }
}
```

`intent` describes this review's local purpose. `inspect` names relevant evidence categories; `criteria` lists requirements; `question` gives a named question. These declarations do not invoke an LLM.

Planner version 2 assigns manual/suite/policy obligations to Review tasks. An external agent authors a substantive source-bound report, records it under normal task guards, and completes the activity separately. A review can be approved and Done/Unverified; approval does not create testing PASS.

Current missing/rejected/stale reports prevent complete acceptance. See [agent evidence and reviews](../agent-evidence.md).

## `policy` and singular `require`

A policy states explicit gate requirements:

```dctx0
policy Release {
  require Properties == PASS
  require Architecture != REQUEST_CHANGES
  require Architecture.highFindings == 0
}
```

The compiler validates references and supported status/count comparisons. The core assesses executable gates against actual outcomes. A policy review cannot override failed native execution. Missing current review information does not default to zero findings.

Distinguish policy `require` from module `requires { Port }` and operation `requires BooleanExpression`: they respectively gate evidence, declare dependencies, and constrain valid execution inputs.

## Evidence and provenance

Use PASS, FAIL, and UNKNOWN with their scope and origin. Reports pin requirements and execution inputs; source-bound reviews remain authored judgments. Automatic results are retained even when an applicable inspection changes the effective assessment.

A complete acceptance is a successful configured finite scope plus current required authored evidence and delivered work. It is not a guarantee for every input or a generated approval. [Evidence and replay](../evidence.md) explains saved reports and their limits.
