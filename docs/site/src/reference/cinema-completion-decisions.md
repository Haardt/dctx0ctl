# Cinema acceptance decisions and inspection history

## Current effective acceptance (2026-10-11)

The actual complete example now has 5,674 mandatory native PASS cases and 104
Done/PASS tasks, with zero open tasks. Existing suite and Release semantics are
implemented. Explicit source-bound inspections and structured reviews supplement
automatic evidence; the effective report is PASS with zero open obligations.
The native automatic report is preserved unchanged. Final aggregate coverage is
still running. See current evidence (historical evidence is available in Git history).

The no-eligible-vote outcome remains None according to the executable contract.
Broader rationale wording is recorded as a low, non-action-required review
finding rather than inventing an unconditional winner requirement. No unresolved
high/action-required finding is approved. The earlier proposals below retain
historical diagnostic context and are not the current outstanding-work list.

## Historical checkpoints

## Current status (2026-10-11)

The proposal below is historical. Its runtime validators, composite rating/vote
identities, explicit result projections and modeled voting effects have since
been implemented and verified. They are not outstanding decisions. The current
complete Cinema corpus executes 5,655 mandatory cases with PASS; 41 of 104 tasks
have been adopted with task-specific native evidence. See
[current progress](cinema-verification-progress.md) and
current Unicode corpus validation (historical evidence is available in Git history).

Current clarification (2026-10-11): all 5,660 mandatory cases pass, including
five persisted entity-key checks. 46 tasks are adopted; 58 remain open. Workspace
validation and the user-authorized 97% coverage gate pass (98.19% lines, 97.07%
functions). Final release measurements completed; their baseline comparison is
unassessed and the accepted baseline remains unchanged. See
entity evidence (historical evidence is available in Git history).

The reference implementation already defines the nine standard proof/test suite
names in `../runatron/packages/plugin-dctx0/src/dctx0/language/proof-catalog.ts`
and evaluates their results and Release rules in `verification/policy.ts`.
Their classification as wholly undefined requirements was too broad. Rust must
port this aggregation semantics and supply category-specific actual evidence;
a suite name alone still cannot authorize a fabricated PASS. Integration,
production-adapter, architecture and ownership checks need their own evidence.

The remaining 18 manual obligations comprise ten textual constraints, four
decisions, three reviews and the Release policy. Some constraints overlap
already modeled behavior but lack an explicit evidence mapping. Decisions include
rationale rather than executable assertions. Reviews require recorded review
results. The Release rules are present in the spec; their evaluation is missing
in Rust. Missing review findings must not be treated as zero. These gaps do not
establish a current Cinema implementation failure or justify changing the spec
to match observed behavior. Whole-run status remains UNKNOWN.

## Historical proposal

This proposal derives from stored full run `2558c3e7a6241afca36730b70e0ab684737e13949a936c841c6443b2bdecc950`. At that checkpoint it changed no application, spec, or task state.

## Runtime contract access

Add application-owned runtime validators for these existing declarations, exposed through the external test adapter. Validators must use ordinary application types and imports; production must not depend on dctx0. The Rust verifier retains input generation and acceptance decisions. Mutation tests must remove an actual validation rule and observe a failure.

- `HomeCinema.MovieNight.Comment`
- `HomeCinema.MovieNight.CommentAdded`
- `HomeCinema.MovieNight.CommentNotFound`
- `HomeCinema.MovieNight.CommentOverview`
- `HomeCinema.MovieNight.CommentView`
- `HomeCinema.MovieNight.FilmAvailabilityChanged`
- `HomeCinema.MovieNight.FilmDetails`
- `HomeCinema.MovieNight.FilmNotFound`
- `HomeCinema.MovieNight.FilmOverview`
- `HomeCinema.MovieNight.FilmRated`
- `HomeCinema.MovieNight.FilmSaved`
- `HomeCinema.MovieNight.FilmSummary`
- `HomeCinema.MovieNight.FilmVote`
- `HomeCinema.MovieNight.FilmVoteResult`
- `HomeCinema.MovieNight.PersistenceUnavailable`
- `HomeCinema.MovieNight.PollNotFound`
- `HomeCinema.MovieNight.PollStatus`
- `HomeCinema.MovieNight.RatingSummary`
- `HomeCinema.MovieNight.ScreeningPoll`
- `HomeCinema.MovieNight.VoteCast`
- `HomeCinema.MovieNight.VotingClosed`
- `HomeCinema.MovieNight.VotingCompleted`
- `HomeCinema.MovieNight.VotingOpened`
- `HomeCinema.MovieNight.VotingOverview`

## Result meaning and resource access

The existing application already declares FilmSummary, FilmOverview,
FilmDetails, CommentView, CommentOverview, RatingSummary and VotingOverview in
`src/application/movie-night.ts`. New runtime access should validate these
actual types rather than introduce generated DTOs. Comment corresponds to the
existing CommentView shape; FilmVote corresponds to persistence's local Vote
type, and ScreeningPoll to its local Poll type. Events already exist as the
ApplicationEvent union. Failure payloads currently use the generic
ApplicationFailure payload; typed validation must retain the actual error code
and payload rather than substitute an expected failure.

There is also an identity decision to review: the spec declares `key memberId`
for Rating and FilmVote, while SQLite uses `(film_id, member_id)` and
`(poll_id, member_id)` respectively. Multiple films/polls for the same member
therefore do not establish unique spec entity identity. The verifier correctly
keeps duplicate declared identities UNKNOWN. Do not hide rows in the observer
to compensate for this difference.

Review output projections for catalog overviews/details, comment overviews, rating summaries, film presence, and current voting. Entity-state observations alone do not define these DTO results. Write an explicit intended projection before enabling proof; copying observed output into expectations is invalid.

Review catalog summary reads of Rating, ScreeningPoll and FilmVote against the declared Film effects. Review opening a poll against its currently modeled observation and actual poll creation/vote removal. Decide intended behavior before changing either contract or application; do not broaden effects merely to silence failures.

## Existing violations

Preserve the reported empty-comment discrepancy and extra/missing-field acceptance failures until an explicit behavior decision or implementation repair is made. Preserve historical task evidence and produce new checks for changed sources. Dynamic Awilix dependency provenance and full Rust coverage remain separate unfinished technical requirements.

Performance remains deferred and the benchmark baseline is unchanged.
