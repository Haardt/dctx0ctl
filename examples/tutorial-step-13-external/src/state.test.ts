import {expect,test} from 'bun:test';
import {Board} from './entities.ts';
import {MissingTask,TaskConflict} from './events.ts';
const id='00000000-0000-0000-0000-000000000001',other='00000000-0000-0000-0000-000000000002';
const row={id,title:'Read',status:'Open' as const};
test('insertion rejects existing keys and replacement rejects absent keys without mutation',()=>{
  const board=new Board();expect(board.insert(row)).toEqual(row);const before=board.snapshot();
  expect(()=>board.insert({...row,title:'Duplicate'})).toThrow(TaskConflict);expect(board.snapshot()).toEqual(before);
  expect(()=>board.replace({...row,id:other})).toThrow(MissingTask);expect(board.snapshot()).toEqual(before);
  expect(board.replace({...row,title:'Changed',status:'Done'})).toEqual({...row,title:'Changed',status:'Done'});
  expect(board.snapshot().Task).toEqual([{...row,title:'Changed',status:'Done'}]);
});
test('upsert changes exactly its keyed row and returned values do not alias stored state',()=>{
  const board=new Board([row]);const added={...row,id:other,title:'Write'};
  expect(board.save(added)).toEqual(added);const returned=board.save({...row,title:'Changed'});returned.title='Outside';
  expect(board.snapshot().Task).toEqual([{...row,title:'Changed'},added]);
  expect(()=>board.save({...row,title:''})).toThrow();expect(board.snapshot().Task).toHaveLength(2);
  expect(board.remove(id)).toBeUndefined();expect(board.snapshot().Task).toEqual([added]);
  expect(board.remove(id)).toBeUndefined();expect(board.snapshot().Task).toEqual([added]);
});
test('composite note upsert preserves other keys and sequences',()=>{
  const board=new Board();const first={taskId:id,sequence:0n,text:'First'},second={taskId:id,sequence:1n,text:'Second'},third={taskId:other,sequence:0n,text:'Third'};
  board.saveNote(first);board.saveNote(second);board.saveNote(third);
  const changed=board.saveNote({...first,text:'Changed'});changed.text='Outside';
  expect(board.snapshot().Note).toEqual([{...first,text:'Changed'},second,third]);
  expect(()=>board.saveNote({...first,sequence:4n})).toThrow();expect(board.snapshot().Note).toHaveLength(3);
});
