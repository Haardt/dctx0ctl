/** Independent production fixture; imports no dctx0 code. */
export function next(input:bigint):bigint{return input+1n;}
/** Independent runtime boundary; inputs remain unknown until production checks them. */
export function validateCount(value:unknown):boolean{return typeof value==='bigint'&&value>=1n&&value<=2n;}
