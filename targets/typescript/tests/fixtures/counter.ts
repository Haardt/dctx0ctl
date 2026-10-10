/** Independent application fixture; no testing runtime or spec imports. */
export class Counter {
 value=0;
 constructor(private readonly load:()=>Promise<number>,private readonly changed:(value:number)=>void){}
 increment(amount:number){this.value+=amount;this.changed(this.value);return this.value;}
 crash(){throw new Error('Unexpected application exception');}
 async refresh(){this.value=await this.load();this.changed(this.value);return this.value;}
}

export function decodeIncrement(input:unknown):number {if(typeof input!=='number'||!Number.isSafeInteger(input))throw new TypeError('Expected a safe integer');return input;}
