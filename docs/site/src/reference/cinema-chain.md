# Bounded Cinema task chain — 2026-10-09

This follow-up exercises exactly three existing tasks from the complete Cinema
specification in a fresh project copy:

1. T31: publish `FilmId` as UUID text with boundary validation.
2. T60: publish the pure `VotingRules.candidateOrderWinner` contract.
3. T93: implement that operation using both delivered contracts.

Run `PATH=/home/held/.cargo/bin:$PATH python3 scripts/cinema_chain_acceptance.py`
from the repository root. The script installs the existing locked dependencies
inside an isolated copy, exercises real CLI transitions, and retains context,
results and check reports under `target/cinema-chain-acceptance/`. No other
Cinema task is completed. There is no HTTP API, persistence adapter or full
module integration, and runatron-next is neither tested nor benchmarked.

## Findings

All 15 workflow assertions and 12 focused Bun tests pass; TypeScript strict
checking also passes. The operation initially has two concrete blockers and
cannot start. Delivering FilmId removes exactly one blocker; delivering its
operations contract makes the implementation ready. The resulting context
contains both producer artifact paths. The dependency slice contains exactly
these three tasks, and repeated planning retains their IDs. Completion alone
remains Unverified; imported, actually executed checks produce Done/PASS. Every
other task, including module integration and review, remains Open.

The small typed composition `const rules: VotingRules = { candidateOrderWinner }`
exercises the published boundary against the real implementation. Tests cover
the exact modeled tie example, deterministic candidate order, eligible vote
counts and unchanged frozen inputs. The reversal invariant is checked over
2,548 combinations: every list of three FilmIds of lengths zero through five,
against seven candidate orders. This is bounded evidence, not a proof over all
possible lists. Replacing `count > highest` with `count >= highest` makes the
tie test fail, confirming that it detects a concrete behavioral defect.

Task context and result handoff are sufficient for this limited chain. Two
points require the implementing LLM's judgment:

- The spec fixes tie ordering and reversal invariance but does not explicitly
  define the no-votes/no-eligible-votes outcome. Returning `None` in those cases
  and ignoring ineligible input votes are recorded implementation decisions,
  not additional modeled requirements. The highest eligible vote count supplies
  the winner selection used by this implementation.
- Module-wide member/poll constraints also appear in this pure operation's
  context. Its inputs contain neither member identities nor poll state. Those
  constraints belong to other operations and do not justify building stores,
  vote replacement or a full voting lifecycle here.

`FilmId` uses canonical lowercase UUID strings without a new library.
`VotingRules` consumes readonly arrays and maps `Option.None` to `undefined`.
The implementation performs no I/O and does not mutate either input. See
[FilmId](https://github.com/Haardt/dctx0ctl/blob/main/examples/cinema/src/domain/film-id.ts),
[operation contract](https://github.com/Haardt/dctx0ctl/blob/main/examples/cinema/src/domain/voting-rules.ts),
[implementation](https://github.com/Haardt/dctx0ctl/blob/main/examples/cinema/src/domain/candidate-order-winner.ts) and
[focused checks](https://github.com/Haardt/dctx0ctl/blob/main/examples/cinema/src/domain/voting-rules.test.ts).

## Measurement evidence

Retained evidence (historical reference removed during repository cleanup) records assertions,
commands, input/source/binary hashes, sampled native memory and release samples.
Source hashes match before and after execution. The rotating measurement is
`benchmarks/history/20261009T211849.740363Z-cinema-chain.json`; its scoped schema-2
record is explicitly unassessed against the accepted full-suite baseline.
The baseline and regression tolerances remain unchanged.

| Cinema release phase | Median ms | Peak RSS MiB |
|---|---:|---:|
| Compilation | 2.145 | 6.96 |
| JSON task context | 1.849 | 17.45 |
| Claim/release | 3.333 | 18.36 |

These three representative phases use one warm-up and three samples of three
iterations. Release RSS is post-exec VmHWM including preparation, IR and SQLite.
Separate native commands took 544.8 ms for strict TypeScript checking and
15.3 ms for the 12-test run, with 272.3 and 32.4 MiB sampled process-tree RSS.
Native memory sampling uses 1 ms `/proc` polling and may miss short peaks.
No interpreter or Rust-target execution is implied by these external checks.
