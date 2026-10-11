export class CounterService {
  static created = 0;
  static disposed = 0;
  constructor() { CounterService.created++; }
  count(value: unknown): bigint {
    if (typeof value !== 'bigint' || value < 0n || value > 3n) {
      throw new TypeError('Count must be an integer from zero through three');
    }
    return value;
  }
  increment(value: bigint): bigint {
    this.count(value);
    if (value === 3n) throw new RangeError('A full board cannot be incremented');
    return value + 1n;
  }
  static dispose(_service: CounterService) { CounterService.disposed++; }
}
