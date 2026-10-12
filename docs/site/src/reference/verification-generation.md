# Deterministic input generation

Implemented as a pure Rust planning component in `dctx0-verification`.
`generate_values(ir, type, budgets)` returns a typed ValueCorpus;
`generate_inputs(ir, parameters, budgets)` returns complete named parameter
assignments. Neither executes production code, publishes compiler history nor
provides adoption authority. Complete Cinema verification remains the active goal.

Generator version `dctx0-values-1` uses seed 42 by default and a fixed SplitMix64
algorithm. Targets do not choose inputs. Scalars retain exact Integer/Decimal
text and a separate finite Float tag. UUIDs and RFC3339 timestamps have explicit
representatives. Integer ranges include endpoints and adjacent values; small
finite ranges, enums, Boolean domains, options and record products are fully
enumerated when they fit the case budget. Larger products are bounded deterministic
samples. Decimal ranges retain exact endpoints and representative valid values.

Lists include empty and size boundaries through the configured collection limit
(default 4), even when full products do not fit. A List corpus is always reported
nonexhaustive because the modeled List type admits larger lengths. The selected
Boolean lists through size 4 contain all 31 distinct values. Parameterless
operations have one empty assignment, not an empty valid input space.

All selected values pass the common IR contract checker; rejected where/range
candidates are filtered before planning. Missing declarations, unsupported
refinements, recursion/work/byte exhaustion and no generated valid inputs are
reported as issues, never success. Nonempty partial corpora retain their issues.
`exhaustive` is false for every unknown or bounded domain. Explicit specification
examples are not inputs to this sampling API and must be preserved separately by
the upcoming plan projection; generated-case limits cannot discard them.

Records use complete field contracts; variants preserve declaration/alternative
and payload. Canonical JSON removes duplicate candidates deterministically.
Parameter combinations preserve declaration order when selecting tuples; the
returned named maps have sorted keys. Input generation rejects duplicate/empty
parameter names and does not deliver half an assignment when a parameter lacks a
valid domain. The algorithm and serialization, seed and budgets must be pinned in
native-run provenance before adoption can be implemented.

## Evidence

The compatibility test compiles the complete unmodified Cinema workspace and
checks generated values for all 45 data declarations against their IR contracts.
Independent tests cover boundaries, exhaustive Boolean products, alias ranges,
variants, lists/options, different seeds, impossible/recursive types, malformed
ranges and resource exhaustion. This is input-planning evidence, not native
Cinema execution.

`PATH=/home/held/.cargo/bin:$PATH python3 scripts/validate.py --coverage --samples 1 --iterations 1`
passes 155 all-feature Rust tests, 8 Python tests, 12,065/12,065 merged source lines
and 1,016/1,016 LLVM functions. Raw LLVM totals and all 80 release workloads are in
checkpoint evidence (historical evidence removed during repository cleanup). The development sample
configuration differs from the accepted baseline; performance is explicitly
unassessed and no baseline is promoted.

New `verification-generate` workloads measure all data-contract corpora with
release code for both complete compiler examples. In the retained one-sample run:
Cinema 25.604 ms / 10,113,024 peak RSS bytes; runatron-next 3,188.240 ms / 71,806,976
peak RSS bytes. RSS is post-exec Linux VmHWM, including IR and phase preparation;
it is not incremental allocation. These phases perform no native application
execution or SQLite operations. Single samples are not a noise-adjusted regression
claim. Use the default-sample coupled runner for the final acceptance gate.
