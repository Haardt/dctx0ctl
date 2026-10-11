import {exactAmount} from './data.ts';
type Mode = 'down'|'up'|'half-up'|'half-even';
function parts(value: unknown): {coefficient:bigint;scale:number} {
  const text=exactAmount(value), [whole,fraction='']=text.split('.');
  return {coefficient:BigInt(whole!+fraction),scale:fraction.length};
}
function render(coefficient: bigint, scale: number): string {
  const negative=coefficient<0n, digits=(negative?-coefficient:coefficient).toString().padStart(scale+1,'0');
  const whole=scale===0?digits:digits.slice(0,-scale);
  const fraction=scale===0?'':digits.slice(-scale).replace(/0+$/,'');
  return (negative?'-':'')+whole+(fraction?'.'+fraction:'');
}
function quotient(numerator: bigint, denominator: bigint, mode: Mode): bigint {
  const whole=numerator/denominator, remainder=numerator%denominator;
  if(remainder===0n || mode==='down') return whole;
  const direction=numerator<0n?-1n:1n, twice=(remainder<0n?-remainder:remainder)*2n;
  if(mode==='up' || twice>denominator || twice===denominator && (mode==='half-up' || whole%2n!==0n)) return whole+direction;
  return whole;
}
function rounded(value: unknown, mode: Mode): string {
  const number=parts(value);
  if(number.scale<=2) return render(number.coefficient,number.scale);
  return render(quotient(number.coefficient,10n**BigInt(number.scale-2),mode),2);
}
export function down(value: unknown): string {return rounded(value,'down');}
export function up(value: unknown): string {return rounded(value,'up');}
export function halfUp(value: unknown): string {return rounded(value,'half-up');}
export function halfEven(value: unknown): string {return rounded(value,'half-even');}
export function third(value: unknown): string {
  const number=parts(value);
  if(number.coefficient%3n===0n) return render(number.coefficient/3n,number.scale);
  return render(quotient(number.coefficient*100n,3n*10n**BigInt(number.scale),'half-even'),2);
}
