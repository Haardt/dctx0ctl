import { expect, test } from "bun:test";
import { filmId } from "./film-id.ts";
import { candidateOrderWinner } from "./candidate-order-winner.ts";
import type { VotingRules } from "./voting-rules.ts";

const a = filmId("00000000-0000-4000-8000-000000000001");
const b = filmId("00000000-0000-4000-8000-000000000002");
const c = filmId("00000000-0000-4000-8000-000000000003");
const rules: VotingRules = { candidateOrderWinner };

test("FilmId accepts UUID text and canonicalizes hexadecimal case", () => {
  expect(filmId("ABCDEF00-0000-4000-8000-000000000001"))
    .toBe("abcdef00-0000-4000-8000-000000000001");
  expect(filmId("00000000-0000-0000-0000-000000000000"))
    .toBe("00000000-0000-0000-0000-000000000000");
});

test.each(["not-a-uuid", "00000000-0000-4000-8000-00000000000g", "", 1, null])(
  "FilmId rejects malformed boundary value %s",
  (input) => expect(() => filmId(input)).toThrow(TypeError),
);

test("delivered type and operation contracts compose into the concrete rules boundary", () => {
  expect(rules.candidateOrderWinner([a, b, b], [a, b])).toBe(b);
});

test("modeled tie example follows candidate order rather than incoming vote order", () => {
  expect(rules.candidateOrderWinner([b, a], [a, b])).toBe(a);
  expect(rules.candidateOrderWinner([a, b], [b, a])).toBe(b);
});

test("modeled reversal invariant holds over bounded exhaustive input combinations", () => {
  const orders = [[], [a], [b], [a, b], [b, a], [a, b, c], [c, b, a]];
  let lists = [[] as string[]];
  for (let length = 0; length <= 5; length++) {
    for (const votes of lists) {
      for (const candidates of orders) {
        expect(rules.candidateOrderWinner(votes, candidates))
          .toBe(rules.candidateOrderWinner([...votes].reverse(), candidates));
      }
    }
    lists = lists.flatMap((votes) => [a, b, c].map((vote) => [...votes, vote]));
  }
});

test("recorded boundary decision returns None without eligible votes", () => {
  expect(rules.candidateOrderWinner([], [a, b])).toBeUndefined();
  expect(rules.candidateOrderWinner([a], [])).toBeUndefined();
  expect(rules.candidateOrderWinner([c], [a, b])).toBeUndefined();
});

test("ineligible votes cannot displace the eligible winner", () => {
  expect(rules.candidateOrderWinner([c, c, c, a], [a, b])).toBe(a);
});

test("pure operation leaves frozen inputs untouched and repeats deterministically", () => {
  const votes = Object.freeze([b, a, b]);
  const candidates = Object.freeze([a, b]);
  expect(rules.candidateOrderWinner(votes, candidates)).toBe(b);
  expect(rules.candidateOrderWinner(votes, candidates)).toBe(b);
  expect(votes).toEqual([b, a, b]);
  expect(candidates).toEqual([a, b]);
});
