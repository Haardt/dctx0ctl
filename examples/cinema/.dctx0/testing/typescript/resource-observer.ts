/** Testing-only observation of actual explicitly executed SQLite statements. */
import {AsyncLocalStorage} from 'node:async_hooks';
import type {Database} from 'bun:sqlite';
import type {ResourceObservation,TestContext} from './contract.ts';
const root='HomeCinema.MovieNight.';
const tables:Record<string,string>={films:'Film',comments:'Comment',ratings:'Rating',polls:'ScreeningPoll',votes:'FilmVote'};
export function observeSqlite(db:Database,context:TestContext){
 const scope=new AsyncLocalStorage<string>();
 const original=db.query;
 const descriptor=Object.getOwnPropertyDescriptor(db,'query');
 const query=function(this:Database,sql:string){
  const statement=Reflect.apply(original,db,[sql]);
  return new Proxy(statement,{get(target,key){
   const value=Reflect.get(target,key,target);
   if(typeof value!=='function')return value;
   if(!['get','all','run','values'].includes(String(key)))return value.bind(target);
   return (...args:unknown[])=>{
    const operation=scope.getStore();
    try{return Reflect.apply(value,target,args);}finally{
     if(operation){
      const read=/^\s*SELECT\b/i.test(sql);
      const write=/^\s*(INSERT|UPDATE|DELETE|REPLACE)\b/i.test(sql);
      const writeTarget=sql.match(/^\s*(?:INSERT(?:\s+OR\s+\w+)?\s+INTO|REPLACE\s+INTO|UPDATE(?:\s+OR\s+\w+)?|DELETE\s+FROM)\s+([a-z_][a-z0-9_]*)/i);
      const matches=read?[...sql.matchAll(/\b(?:FROM|JOIN)\s+([a-z_][a-z0-9_]*)/gi)]:writeTarget?[writeTarget]:[];
      const access:ResourceObservation['access']=write?'write':'read';
      if(!read&&!write||matches.length===0)context.recordResource({operation,resource:'unmodeled:sqlite-statement',access});
      else for(const match of matches){const table=match[1].toLowerCase();context.recordResource({operation,resource:tables[table]?root+tables[table]:'unmodeled:sqlite-table:'+table,access});}
     }
    }
   };
  }});
 };
 Object.defineProperty(db,'query',{configurable:true,writable:true,value:query});
 return {
  run<T>(operation:string,body:()=>T):T{return scope.run(operation,body);},
  close(){if(descriptor)Object.defineProperty(db,'query',descriptor);else Reflect.deleteProperty(db,'query');},
 };
}
