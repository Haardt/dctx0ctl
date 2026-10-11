/** Testing-only observation of real accesses to the reviewed Board arrays. */
import {AsyncLocalStorage} from 'node:async_hooks';
import type {Board} from '../../../src/entities.ts';
import type {TestContext} from './contract.ts';
export function observeBoard(board:Board,context:TestContext) {
  const scope=new AsyncLocalStorage<string>();
  const original=new Map<string,PropertyDescriptor>();
  const bindings=[['tasks','Tutorial.Board.Task'],['entries','Tutorial.Board.Note']] as const;
  for(const [field] of bindings) {
    const descriptor=Object.getOwnPropertyDescriptor(board,field);
    if(!descriptor||!Array.isArray(descriptor.value))throw new Error('No actual Board array for '+field);
    original.set(field,descriptor);
  }
  for(const [field,resource] of bindings) {
    const descriptor=original.get(field)!;
    const report=(access:'read'|'write')=>{
      const operation=scope.getStore();if(operation)context.recordResource({operation,resource:resource!,access});
    };
    const observed=new Proxy(descriptor.value as unknown[],{
      get(target,key,receiver){if(key==='length'||typeof key==='string'&&/^\d+$/.test(key))report('read');return Reflect.get(target,key,receiver);},
      set(target,key,value,receiver){report('write');return Reflect.set(target,key,value,receiver);},
      deleteProperty(target,key){report('write');return Reflect.deleteProperty(target,key);},
    });
    Object.defineProperty(board,field!,{...descriptor,value:observed});
  }
  return {
    run<T>(operation:string,body:()=>T):T{return scope.run(operation,body);},
    observe<T>(resource:string,body:()=>T):T{
      try{return body();}finally{const operation=scope.getStore();if(operation)context.recordResource({operation,resource,access:'observe'});}
    },
    close(){for(const [field,descriptor]of original)Object.defineProperty(board,field,descriptor);scope.disable();},
  };
}
