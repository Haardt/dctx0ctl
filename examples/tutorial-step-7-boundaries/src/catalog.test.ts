import {expect,test} from 'bun:test';
import {Board} from './entities.ts';
import {BoardReader,Catalog,type TaskReader} from './catalog.ts';
const id='00000000-0000-0000-0000-000000000001';
test('catalog reads actual board summaries through the production adapter',()=>{
  const board=new Board([{id,title:'Read',status:'Open'}]),reader=new BoardReader(board),catalog=new Catalog(reader);
  expect(reader.lookup(id)).toEqual({tag:'some',value:{id,title:'Read'}});
  expect(catalog.find(id)).toEqual({tag:'some',value:{id,title:'Read'}});
  expect(catalog.find('00000000-0000-0000-0000-000000000002')).toEqual({tag:'none'});
  expect(()=>reader.lookup('bad')).toThrow();expect(()=>catalog.find('bad')).toThrow();
});
test('normal interface injection allows a different reader without changing catalog code',()=>{
  const calls:string[]=[];
  const reader:TaskReader={lookup(key){calls.push(key);return {tag:'none'};}};
  expect(new Catalog(reader).find(id)).toEqual({tag:'none'});expect(calls).toEqual([id]);
});
