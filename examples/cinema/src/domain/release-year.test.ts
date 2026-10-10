import { describe, expect, test } from "bun:test";
import { releaseYear, type ReleaseYear } from "./release-year.ts";

describe("MovieNight.ReleaseYear", () => {
  test.each([1888, 1999, 2100])("preserves valid integer %s", (input) => {
    const value: ReleaseYear = releaseYear(input);
    expect(value).toBe(input);
    expect(typeof value).toBe("number");
  });

  test.each([1887, 2101, -1, Number.MAX_SAFE_INTEGER])(
    "rejects out-of-range integer %s",
    (input) => expect(() => releaseYear(input)).toThrow(RangeError),
  );

  test.each([1888.5, 2099.5, NaN, Infinity, -Infinity, "1999", null, undefined, {}, true])(
    "rejects non-integer input %s",
    (input) => expect(() => releaseYear(input)).toThrow(TypeError),
  );

  test("covers every permitted integer", () => {
    for (let year = 1888; year <= 2100; year++) {
      expect(releaseYear(year)).toBe(year);
    }
  });
});
