// This application's own service names and dependency structure.
export class SequenceService {
  constructor(private readonly increment: number) {}

  calculateNext(amount: number): number {
    return amount + this.increment;
  }
}

export function boundedAmount(value: unknown): number {
  if (typeof value !== "number" || !Number.isInteger(value) || value < 0 || value > 3) {
    throw new RangeError("Amount outside the supported integer range");
  }
  return value;
}
