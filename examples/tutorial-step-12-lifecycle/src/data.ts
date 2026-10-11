import { count, taskId, title } from './board.ts';

export type Option<T> = { tag: 'none' } | { tag: 'some'; value: T };
export type Attempt = { tag: 'ok'; value: bigint } | { tag: 'error'; error: string };
export type Status = 'Open' | 'Done' | 'Archived';
export type Choice = { tag: 'empty' } | { tag: 'named'; title: string };
export type Details = {
  id: string; title: string; status: Status; choice: Choice; active: boolean;
  progress: number; estimate: number; amount: string; attachment: Uint8Array;
  note: Option<string>; labels: string[]; attempt: Attempt;
};

export function object(value: unknown, keys: string[]): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new TypeError('Expected an object');
  const record = value as Record<string, unknown>;
  if (Object.keys(record).length !== keys.length || keys.some(key => !Object.hasOwn(record, key))) throw new TypeError('Object fields do not match the boundary');
  return record;
}
export function integer(value: unknown): bigint {
  if (typeof value !== 'bigint') throw new TypeError('Expected an exact integer');
  return value;
}
export function flag(value: unknown): boolean {
  if (typeof value !== 'boolean') throw new TypeError('Expected a boolean');
  return value;
}
export function measure(value: unknown): number {
  if (typeof value !== 'number' || !Number.isFinite(value) || value < 0 || value > 10) throw new TypeError('Progress must be finite and between zero and ten');
  return value;
}
export function estimate(value: unknown): number {
  if (typeof value !== 'number' || !Number.isFinite(value) || value < 0 || value > 3600) throw new TypeError('Estimate must be finite and between zero and 3600');
  return value;
}
export function exactAmount(value: unknown): string {
  if (typeof value !== 'string' || !/^-?\d+(?:\.\d+)?$/.test(value)) throw new TypeError('Expected an exact decimal string');
  return value;
}
export function attachment(value: unknown): Uint8Array {
  if (value instanceof Uint8Array) return new Uint8Array(value);
  if (!Array.isArray(value) || value.some(byte => typeof byte !== 'number' || !Number.isInteger(byte) || byte < 0 || byte > 255)) throw new TypeError('Invalid byte array; truncation is forbidden');
  return new Uint8Array(value);
}
export function noValue(value: unknown): undefined {
  if (value !== undefined) throw new TypeError('Expected no value');
  return undefined;
}
export function maybeTitle(value: unknown): Option<string> {
  const box = object(value, (value as {tag?:unknown} | null)?.tag === 'some' ? ['tag', 'value'] : ['tag']);
  if (box.tag === 'none') return { tag: 'none' };
  if (box.tag === 'some') return { tag: 'some', value: title(box.value) };
  throw new TypeError('Unknown option alternative');
}
export function presentTitle(value: unknown): {tag:'some';value:string} {
  const box = maybeTitle(value);
  if (box.tag !== 'some') throw new TypeError('A value is required');
  return box;
}
export function absent(value: unknown): {tag:'none'} {
  const box = object(value, ['tag']);
  if (box.tag !== 'none') throw new TypeError('Expected an absent value');
  return { tag: 'none' };
}
export function attempt(value: unknown): Attempt {
  const tag = (value as {tag?:unknown} | null)?.tag;
  const box = object(value, tag === 'ok' ? ['tag', 'value'] : ['tag', 'error']);
  if (box.tag === 'ok') return { tag: 'ok', value: count(box.value) };
  if (box.tag === 'error') return { tag: 'error', error: title(box.error) };
  throw new TypeError('Unknown result alternative');
}
export function status(value: unknown): Status {
  if (value !== 'Open' && value !== 'Done' && value !== 'Archived') throw new TypeError('Unknown status');
  return value;
}
export function choice(value: unknown): Choice {
  const tag = (value as {tag?:unknown} | null)?.tag;
  const box = object(value, tag === 'empty' ? ['tag'] : ['tag', 'title']);
  if (box.tag === 'empty') return { tag: 'empty' };
  if (box.tag === 'named') return { tag: 'named', title: title(box.title) };
  throw new TypeError('Unknown choice');
}
export function labels(value: unknown): string[] {
  if (!Array.isArray(value)) throw new TypeError('Expected a label list');
  return value.map(title);
}
export function details(value: unknown): Details {
  const data = object(value, ['id','title','status','choice','active','progress','estimate','amount','attachment','note','labels','attempt']);
  return {
    id: taskId(data.id), title: title(data.title), status: status(data.status), choice: choice(data.choice),
    active: flag(data.active), progress: measure(data.progress), estimate: estimate(data.estimate),
    amount: exactAmount(data.amount), attachment: attachment(data.attachment), note: maybeTitle(data.note),
    labels: labels(data.labels), attempt: attempt(data.attempt),
  };
}
export function keepDetails(value: Details): Details { return details(value); }
export function keepAttempt(value: Attempt): Attempt { return attempt(value); }
