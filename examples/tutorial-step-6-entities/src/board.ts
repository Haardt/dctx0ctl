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

export function rawLabel(value: unknown): string {
  if (typeof value !== 'string') throw new TypeError('Expected a text label');
  return value;
}
export function title(value: unknown): string {
  const text = rawLabel(value);
  if (text.length < 1 || text.length > 16 || text.trim().length === 0) throw new TypeError('Title needs 1..16 characters and visible text');
  return text;
}
export const displayTitle = title;
export function echo(value: string): string { return displayTitle(value); }
export function email(value: unknown): string {
  const text = rawLabel(value);
  if (text.length < 3 || text.length > 254 || !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(text)) throw new TypeError('Invalid email');
  return text;
}
export function identifier(value: unknown): string {
  const text = rawLabel(value);
  if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(text)) throw new TypeError('Invalid formatted identifier');
  return text;
}
export function taskId(value: unknown): string {
  const text = rawLabel(value);
  if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(text)) throw new TypeError('Invalid task identity');
  return text;
}
export function link(value: unknown): string {
  const text = rawLabel(value);
  new URL(text);
  return text;
}
export function day(value: unknown): string {
  const text = rawLabel(value);
  if (!/^\d{4}-\d{2}-\d{2}$/.test(text)) throw new TypeError('Invalid date');
  const [year, month, date] = text.split('-').map(Number) as [number, number, number];
  const leap = year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0);
  const lengths = [31, leap ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
  if (month < 1 || month > 12 || date < 1 || date > lengths[month - 1]!) throw new TypeError('Impossible calendar date');
  return text;
}
export function instant(value: unknown): string {
  const text = rawLabel(value);
  const match = /^(\d{4}-\d{2}-\d{2})[Tt ](\d{2}):(\d{2}):(\d{2})(?:\.\d+)?(?:[Zz]|([+-])(\d{2}):(\d{2}))$/.exec(text);
  if (!match || match[0].length !== text.length) throw new TypeError('Invalid timestamp');
  day(match[1]);
  if (Number(match[2]) > 23 || Number(match[3]) > 59 || Number(match[4]) > 60 || Number(match[6] ?? 0) > 23 || Number(match[7] ?? 0) > 59) throw new TypeError('Invalid time or offset');
  return text;
}
export function shortList(value: unknown): bigint[] {
  if (!Array.isArray(value) || value.length < 1 || value.length > 2 || value.some(entry => typeof entry !== 'bigint')) throw new TypeError('Expected one or two exact integers');
  return [...value] as bigint[];
}
