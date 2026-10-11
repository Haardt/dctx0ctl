import {expect,test} from 'bun:test';
import {Board} from './entities.ts';
import {BoardReader,Catalog,type TaskReader} from './catalog.ts';
const id='00000000-0000-0000-0000-000000000001';
test('catalog reads actual board summaries through the production adapter',async()=>{
  const board=new Board([{id,title:'Read',status:'Open'}]),reader=new BoardReader(board),catalog=new Catalog(reader);
  expect(reader.lookup(id)).toEqual({tag:'some',value:{id,title:'Read'}});
  expect(await catalog.find(id)).toEqual({tag:'some',value:{id,title:'Read'}});
  expect(await catalog.find('00000000-0000-0000-0000-000000000002')).toEqual({tag:'none'});
  expect(()=>reader.lookup('bad')).toThrow();await expect(catalog.find('bad')).rejects.toThrow();
});
test('normal interface injection allows a different reader without changing catalog code',async()=>{
  const calls:string[]=[];
  const reader:TaskReader={lookup(key){calls.push(key);return {tag:'none'};}};
  expect(await new Catalog(reader).find(id)).toEqual({tag:'none'});expect(calls).toEqual([id]);
});

test('consumer awaits a collaborator and preserves a real rejection',async()=>{
  const calls:string[]=[];
  const found=new Catalog({async lookup(key){calls.push(key);return {tag:'some',value:{id:key,title:'Async'}};}});
  expect(await found.find(id)).toEqual({tag:'some',value:{id,title:'Async'}});expect(calls).toEqual([id]);
  const failure=new Error('Reader unavailable');
  await expect(new Catalog({async lookup(){throw failure;}}).find(id)).rejects.toBe(failure);
});
