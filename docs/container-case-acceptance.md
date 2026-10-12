# Published-release acceptance in a clean Go container

On October 12, 2026, the real GitHub `v0.1.0` installer and binary were exercised
in fresh containers using the already available `golang:1.25.5` image (Debian 13,
Linux x86_64). Containers had two CPUs and a 3 GiB memory limit. No host project
files or credentials were mounted. No external LLM session was started.

The final reproducible run completed successfully. It tested:

- HTTPS installation of the actual GitHub release, including a real
  `curl ... | bash` reinstall and binary version check.
- All three bundled skills installed for both Codex and Claude. A differing
  locally edited skill was rejected without overwriting it.
- Go profile creation and generated `AGENTS.md` guidance. Repeating creation
  preserved an edited profile. Claims froze that profile's custom note.
- Compilation of a fresh CaseCli specification: 12 symbols and two operations.
- Four tasks in two prerequisite waves. Two worker actors received distinct
  ready tasks. Claims were subsequently released; blocked wave-two work remained
  blocked. These actors tested coordination, not autonomous agent execution.
- A real Go CLI and its production conversion functions: mixed case, empty
  input, punctuation/digits, Unicode, missing arguments and an unknown command.
- Central DCTX0 verification: 24 finite cases passed (eight authored examples,
  16 generated operation cases).
- A mutation replacing `strings.ToUpper(text)` with unchanged input: DCTX0
  reported FAIL. Replay failed on the broken source and passed after repair.

The generated cases check String result contracts. Casing behavior is asserted
by the eight examples; this is not an all-input Unicode behavior proof. The CLI
entry point is checked by direct process calls outside the DCTX0 operation plan.

## Finding: finite-plan PASS does not deliver all tasks

Published `v0.1.0` accepted the application behavior but did not automatically
adopt these tasks, including when tasks were explicitly selected. It reported
`INCOMPLETE_REQUIREMENTS` with missing `REQUIREMENT_MAPPING` for `pure` and
example facts. This is a reproducible task-evidence mapping gap in this workflow.
The four tasks were not falsely marked Done/PASS. No specification requirements
were removed or weakened to bypass the guard.

The generated adapter scaffold initially remained insufficient because no event
observer was declared. The completed adapter enables the worker's event stream;
these pure functions produce no events. The final verification was PASS.

## Runtime and RAM evidence

Functional checks were coupled with child elapsed-time and peak-RSS measurements.
The final warm native verification took about **765 ms**, with child/reaped
 descendant peak RSS **70,307,840 bytes**. The real installer reinstall took
about **772 ms**, with peak RSS **16,949,248 bytes**. Native Go tests took about
**3,309 ms**, with peak RSS **99,966,976 bytes**.

These are single samples in the stated container, not a regression baseline.
`wait4` RSS is the maximum reported child/reaped-descendant high-water value,
not collective simultaneous memory and not incremental allocation. The setup
runner explicitly labels its cumulative-child RSS; implementation and final
acceptance runners record per-command child values. Native DCTX0 reports retain
compiler/worker provenance separately from central case assessment.

[Structured measurements and provenance](../benchmarks/history/container-case/20261012T190618.010437Z.json)
retain raw samples. This isolated consumer workload is unassessed against the
accepted core baseline, which was not changed. The runner keeps its latest 20
records under `benchmarks/history/container-case/` with locked atomic publication.

## Reproduction and retained artifacts

```sh
python3 scripts/container_case_acceptance.py
```

This uses the existing Go base image, creates a fresh container, installs the
published release, authors the application/spec, and runs the checks and mutation.
Raw logs are copied to `target/container-case/<container-name>/`. Containers are
stopped and retained for inspection unless `--keep-running` is specified.

The final tested container is `dctx0-case-final`; its application is under
`/work/casecli`, with raw evidence under `/work/evidence`. Restart it with
`docker start dctx0-case-final` if needed.

The checked-in [CaseCli example](../examples/casecli/README.md) contains the
specification, Go production code, tests and adapter. Its README explains how to
create the remaining transport files and repeat verification.
