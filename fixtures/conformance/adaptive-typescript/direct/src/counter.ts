// Existing application API: ordinary numbers and exports.
export function count(value: unknown): number {
  if (typeof value !== "number" || !Number.isInteger(value) || value < 0 || value > 3) {
    throw new RangeError("Count must be an integer from zero to three");
  }
  return value;
}

export function nextValue(value: number): number {
  return value + 1;
}
