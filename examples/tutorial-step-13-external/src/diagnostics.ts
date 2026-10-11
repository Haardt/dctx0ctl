export class SystemClock {
  constructor(private readonly source:()=>Date=()=>new Date()) {}
  now(): string {return this.source().toISOString();}
}
export class Diagnostics {
  constructor(private readonly observeClock:()=>string) {}
  timestamp(): string {return this.observeClock();}
}
