import type { FilmId } from "./film-id.ts";
import type { VotingRules } from "./voting-rules.ts";

/**
 * Select the most-voted eligible film, resolving ties by candidate order.
 * Boundary decisions: no eligible votes yields None; unknown votes are ignored.
 * Member deduplication and poll state belong to the voting store, outside this pure function.
 */
export const candidateOrderWinner: VotingRules["candidateOrderWinner"] = (votes, candidates) => {
  const eligible = new Set(candidates);
  const counts = new Map<FilmId, number>();
  for (const vote of votes) {
    if (eligible.has(vote)) counts.set(vote, (counts.get(vote) ?? 0) + 1);
  }
  let winner: FilmId | undefined;
  let highest = 0;
  for (const candidate of candidates) {
    const count = counts.get(candidate) ?? 0;
    if (count > highest) {
      highest = count;
      winner = candidate;
    }
  }
  return winner;
};
