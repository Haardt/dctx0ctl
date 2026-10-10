import type { FilmId } from "./film-id.ts";

/** Pure VotingRules boundary; undefined represents the modeled Option.None. */
export interface VotingRules {
  candidateOrderWinner(votes: readonly FilmId[], candidates: readonly FilmId[]): FilmId | undefined;
}
