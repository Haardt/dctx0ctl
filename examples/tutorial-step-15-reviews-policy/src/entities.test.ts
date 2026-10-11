import {expect,test} from 'bun:test';
import {Board,task,note,taskSummary} from './entities.ts';
const id='00000000-0000-4000-8000-000000000001',other='00000000-0000-4000-8000-000000000002';
const first={id,title:'Read',status:'Open' as const};
const entry={taskId:id,sequence:0n,text:'Book'};
test('entity and message factories preserve the declared boundaries',()=>{
  expect(task(first)).toEqual(first);expect(note(entry)).toEqual(entry);expect(taskSummary({id,title:'Read'})).toEqual({id,title:'Read'});
  expect(()=>task({...first,extra:1})).toThrow();expect(()=>task({...first,status:'Unknown'})).toThrow();
  expect(()=>note({...entry,sequence:4n})).toThrow();expect(()=>note({...entry,text:''})).toThrow();
  expect(()=>taskSummary(first)).toThrow();expect(()=>taskSummary({id:'invalid',title:'Read'})).toThrow();
});
test('actual board reads expose messages and isolated state snapshots',()=>{
  const second={id:other,title:'Write',status:'Done'};
  const board=new Board([first,second],[entry,{taskId:other,sequence:0n,text:'Later'},{taskId:id,sequence:1n,text:'Again'}]);
  expect(board.list()).toEqual([{id,title:'Read'},{id:other,title:'Write'}]);
  expect(board.find(id)).toEqual({tag:'some',value:{id,title:'Read'}});
  expect(board.find('00000000-0000-4000-8000-000000000003')).toEqual({tag:'none'});
  expect(board.notes(id)).toEqual([entry,{taskId:id,sequence:1n,text:'Again'}]);
  const observed=board.snapshot();observed.Task[0]!.title='Changed';observed.Note[0]!.text='Changed';
  expect(board.snapshot().Task[0]!.title).toBe('Read');expect(board.snapshot().Note[0]!.text).toBe('Book');
  expect(new Board().snapshot()).toEqual({Task:[],Note:[]});
  expect(()=>board.find('wrong')).toThrow();expect(()=>board.notes('wrong')).toThrow();
});
test('single and composite keys reject duplicate identities without conflating note sequences',()=>{
  expect(()=>new Board([first,first])).toThrow();expect(()=>new Board([],[entry,entry])).toThrow();
  expect(new Board([],[entry,{...entry,sequence:1n}]).snapshot().Note).toHaveLength(2);
});
