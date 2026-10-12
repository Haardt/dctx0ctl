# Assessment 11: known violations survive incomplete effect evidence

Assessment 10 returned at the first unmodeled effect label or unavailable
collaborator observation. Consequently a later, independently observed invalid
collaborator return or event payload could be hidden by UNKNOWN, even when an
additional valid emission satisfied the modeled emission clause.

Assessment 11 retains the first unresolved boundary issue and continues checking
the remaining observed labels, event contracts and collaborator contracts. A
known violation produces FAIL; otherwise the retained unresolved issue produces
UNKNOWN. No observation is invented, and an unknown label never supplies a
modeled dependency or a passing contract.

The regression
`unknown_labels_and_unavailable_calls_do_not_hide_known_contract_violations`
failed before the correction. It now passes for missing and opaque call evidence
preceding an invalid return, and opaque events preceding/following a malformed
known payload alongside a valid required emission. The delegated-resource test
now receives the earlier `UNDECLARED_OPERATION_EFFECT` diagnosis rather than
the later `RESOURCE_CALL_UNDECLARED` diagnosis for the same known violation.

This intentionally changes verification assessment and is bound by
`dctx0-assessment-11` provenance. AST/IR, compiler goldens, production code,
task definitions and prior stored reports are unchanged. The core test suite and
all-target verification-core/Engine Clippy checks pass. The fresh full Cinema
acceptance also passes: 5,655 mandatory cases, 41 adopted tasks, zero incomplete
provenance entries. Its whole result remains UNKNOWN for the existing 27 manual
or undefined-suite obligations; see retained evidence (historical evidence removed during repository cleanup).
Release performance and strict final coverage remain separate outstanding gates.
