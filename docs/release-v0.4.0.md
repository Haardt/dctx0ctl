## Faster Go verification and incremental planning

Go application tests can send up to 16 decoder cases in a packet. Review the
decoder and projector for statelessness, use `Client.StatelessDecoderTest` and
prepare the plan with `--stateless-decoder TEST` for each eligible registration.
Rust assesses each case and the native subtest reports that verdict. Plans pin
the reviewed Go sources and build inputs. Prepare existing packet-enabled
plans again after upgrading. Go application-test library 0.3.0 uses application
protocol 2.

Isolated Go decoder cohorts can reuse passing evidence from compatible native
runs. Bind the real production decoder functions with `--unit-groups PATH`.
The CLI compares compiler-selected dependencies and discovery inputs, including
sources excluded by Go build constraints. Existing cohorts may need an explicit
fresh run with `--reuse off` to establish evidence with the updated discovery
hash. The read-only `verify incremental-plan` report shows specification and
task impact, group changes and the basis for possible reuse.

Verification preflight and compact status reports identify missing inputs and
delivery blockers. A cold run without compatible evidence requires explicit
`--reuse off`. Native verification scaffolding can generate editable codecs and
contract tests. Existing production bindings can remain in mixed test plans.

The bundled implementation skill now explains how to measure process starts,
fixture costs and channel overhead, choose the right execution path, and
compare mandatory case IDs, native outcomes and per-case evidence.

[Go application-test workflow](https://github.com/Haardt/dctx0ctl/blob/v0.4.0/docs/site/src/docs/application-tests.md)
