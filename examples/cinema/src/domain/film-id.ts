/** HomeCinema.MovieNight.FilmId uses canonical UUID text at TypeScript boundaries. */
export type FilmId = string;

export function filmId(input: unknown): FilmId {
  if (typeof input !== "string" || !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(input)) {
    throw new TypeError("FilmId must be UUID text");
  }
  return input.toLowerCase();
}
