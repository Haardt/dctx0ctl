/** Validate the wire timestamp, retaining its original precision and offset. */
export function timestamp(value:unknown):string {
 if(typeof value!=='string')throw new TypeError('Expected RFC3339 timestamp');
 const match=/^(\d{4})-(\d{2})-(\d{2})[Tt ](\d{2}):(\d{2}):(\d{2})(?:\.\d+)?(?:[Zz]|[+-](\d{2}):(\d{2}))$/.exec(value);
 if(!match||match[0].length!==value.length)throw new TypeError('Expected RFC3339 timestamp');
 const [year,month,day,hour,minute,second]=match.slice(1,7).map(Number);
 const leap=year%4===0&&(year%100!==0||year%400===0);
 const days=[31,leap?29:28,31,30,31,30,31,31,30,31,30,31];
 if(month<1||month>12||day<1||day>days[month-1]||hour>23||minute>59||second>60||(match[7]!==undefined&&(Number(match[7])>23||Number(match[8])>59)))throw new TypeError('Expected RFC3339 timestamp');
 return value;
}
