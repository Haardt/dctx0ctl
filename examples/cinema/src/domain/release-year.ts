/** HomeCinema.MovieNight.ReleaseYear: an integer in the inclusive range 1888..2100. */
export type ReleaseYear = number;

/** Validate a value at the application boundary without changing its numeric representation. */
export function releaseYear(input: unknown): ReleaseYear {
  if (typeof input !== "number" || !Number.isInteger(input)) {
    throw new TypeError("ReleaseYear must be an integer");
  }
  if (input < 1888 || input > 2100) {
    throw new RangeError("ReleaseYear must be between 1888 and 2100 inclusive");
  }
  return input;
}
