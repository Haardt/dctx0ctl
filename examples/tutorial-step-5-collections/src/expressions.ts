import {count, title} from './board.ts';
import {choice, type Choice} from './data.ts';
export function score(a: bigint, b: bigint): bigint {
  const left=count(a), right=count(b);
  if (right === 0n) throw new RangeError('A nonzero second count is required');
  return (left + right) * 2n - left;
}
export function eligible(value: bigint): boolean {return count(value) !== 0n;}
export function describe(value: string): string {return 'Task: '+title(value);}
export function named(value: string): Choice {return choice({tag:'named',title:value});}
