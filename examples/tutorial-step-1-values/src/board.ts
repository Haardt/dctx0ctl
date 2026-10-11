/** Application-owned representation: integers stay exact, even beyond JS Number. */
export function count(value: unknown): bigint {
  if (typeof value !== 'bigint' || value < 0n || value > 3n) {
    throw new TypeError('Count must be an integer from zero through three');
  }
  return value;
}

export function increment(value: bigint): bigint {
  count(value);
  if (value === 3n) throw new RangeError('A full board cannot be incremented');
  return value + 1n;
}
