import {count, rawLabel} from './board.ts';
import {flag, type Option} from './data.ts';
export function counts(value: unknown): bigint[] {
  if (!Array.isArray(value)) throw new TypeError('Expected a count list');
  return value.map(count);
}
export function size(value: unknown): bigint {return BigInt(counts(value).length);}
export function total(value: unknown): bigint {return counts(value).reduce((sum,entry)=>sum+entry,0n);}
export function allPositive(value: unknown): boolean {return counts(value).every(entry=>entry>0n);}
export function anyPositive(value: unknown): boolean {return counts(value).some(entry=>entry>0n);}
export function containsZero(value: unknown): boolean {return counts(value).includes(0n);}
export function unique(value: unknown): bigint[] {return [...new Set(counts(value))];}
function nonempty(value: unknown): bigint[] {
  const list=counts(value); if(list.length===0) throw new RangeError('Expected at least one count'); return list;
}
export function minimum(value: unknown): bigint {return nonempty(value).reduce((a,b)=>a<b?a:b);}
export function maximum(value: unknown): bigint {return nonempty(value).reduce((a,b)=>a>b?a:b);}
export function positive(value: unknown): bigint[] {return counts(value).filter(entry=>entry>0n);}
export function doubled(value: unknown): bigint[] {return counts(value).map(entry=>entry*2n);}
export function ascending(value: unknown): bigint[] {return counts(value).sort((a,b)=>a<b?-1:a>b?1:0);}
export function descending(value: unknown): bigint[] {return counts(value).sort((a,b)=>a>b?-1:a<b?1:0);}
export function head(value: unknown): Option<bigint> {
  const list=counts(value); return list.length===0?{tag:'none'}:{tag:'some',value:list[0]!};
}
export function only(value: unknown): bigint {
  const list=counts(value); if(list.length!==1) throw new RangeError('Expected exactly one count'); return list[0]!;
}
export function average(value: unknown): Option<number> {
  const list=counts(value); return list.length===0?{tag:'none'}:{tag:'some',value:Number(list.reduce((sum,entry)=>sum+entry,0n))/list.length};
}
export function zeroIndex(value: unknown): bigint {return BigInt(counts(value).indexOf(0n));}
export function fallback(value: unknown): bigint {const list=counts(value);return list[0]??-1n;}
export function backwards(value: unknown): bigint[] {return counts(value).reverse();}
export function window(value: unknown): bigint[] {return counts(value).slice(1,4);}
export function lower(value: unknown): string {return rawLabel(value).toLowerCase();}
export function upper(value: unknown): string {return rawLabel(value).toUpperCase();}
export function clean(value: unknown): string {return rawLabel(value).trim();}
export function textSize(value: unknown): bigint {return BigInt(rawLabel(value).length);}
export function starts(value: unknown): boolean {return rawLabel(value).startsWith('Task');}
export function ends(value: unknown): boolean {return rawLabel(value).endsWith('done');}
export function includes(value: unknown): boolean {return rawLabel(value).includes('Read');}
export function optional(value: unknown, present: unknown): Option<bigint> {
  const entry=count(value);return flag(present)?{tag:'some',value:entry}:{tag:'none'};
}
