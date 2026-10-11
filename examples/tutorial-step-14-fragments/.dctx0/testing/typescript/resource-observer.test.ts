import {expect,test} from 'bun:test';
import {Board} from '../../../src/entities.ts';
import {observeBoard} from './resource-observer.ts';
import type {ResourceObservation,TestContext} from './contract.ts';
const id='00000000-0000-0000-0000-000000000001';
function context(events:ResourceObservation[]):TestContext {
  return {recordResource:event=>events.push(event),emit(){},recordCall(){},async collaborator(){return {kind:'unavailable',reason:'No collaborator in this focused test'};}};
}
test('actual array reads are scoped, observations are recorded and cleanup restores original descriptors',()=>{
  const board=new Board([{id,title:'Read',status:'Open'}]),events:ResourceObservation[]=[];
  const original=Object.getOwnPropertyDescriptor(board,'tasks')!;
  const observer=observeBoard(board,context(events));
  board.snapshot();expect(events).toEqual([]);
  expect(observer.run('Tutorial.Board.Tasks.list',()=>board.list())).toEqual([{id,title:'Read'}]);
  expect(events.length).toBeGreaterThan(0);expect(events.every(event=>event.resource==='Tutorial.Board.Task'&&event.access==='read'&&event.operation==='Tutorial.Board.Tasks.list')).toBe(true);
  events.length=0;
  expect(observer.run('Tutorial.Board.Diagnostics.timestamp',()=>observer.observe('Tutorial.Board.Signals.Clock.now',()=>42))).toBe(42);
  expect(events).toEqual([{operation:'Tutorial.Board.Diagnostics.timestamp',resource:'Tutorial.Board.Signals.Clock.now',access:'observe'}]);
  observer.close();expect(Object.getOwnPropertyDescriptor(board,'tasks')).toEqual(original);
  board.list();expect(events).toHaveLength(1);
});
test('actual writes and removals are captured even when no permission is declared by the caller',()=>{
  const board=new Board(),events:ResourceObservation[]=[];const observer=observeBoard(board,context(events));
  try {
    observer.run('Tutorial.Board.Writer.save',()=>{const rows=Reflect.get(board,'tasks') as unknown[];rows.push({id,title:'Read',status:'Open'});delete rows[0];});
    expect(events.some(event=>event.access==='write')).toBe(true);
    expect(events.every(event=>event.operation==='Tutorial.Board.Writer.save'&&event.resource==='Tutorial.Board.Task')).toBe(true);
  } finally {observer.close();}
});
test('async scope survives awaiting the application and rejected observations remain actual attempts',async()=>{
  const board=new Board(),events:ResourceObservation[]=[];const observer=observeBoard(board,context(events));
  try {
    await observer.run('Tutorial.Board.Tasks.list',async()=>{await Promise.resolve();board.list();});
    expect(events.every(event=>event.operation==='Tutorial.Board.Tasks.list')).toBe(true);expect(events.length).toBeGreaterThan(0);
    events.length=0;
    expect(()=>observer.run('Tutorial.Board.Diagnostics.timestamp',()=>observer.observe('Tutorial.Board.Signals.Clock.now',()=>{throw new Error('Unavailable');}))).toThrow('Unavailable');
    expect(events).toEqual([{operation:'Tutorial.Board.Diagnostics.timestamp',resource:'Tutorial.Board.Signals.Clock.now',access:'observe'}]);
  }finally{observer.close();}
});
test('missing real storage refuses observation without partially installing a proxy',()=>{
  const board=new Board();const original=Object.getOwnPropertyDescriptor(board,'tasks');Reflect.deleteProperty(board,'entries');
  expect(()=>observeBoard(board,context([]))).toThrow('No actual Board array for entries');
  expect(Object.getOwnPropertyDescriptor(board,'tasks')).toEqual(original);
});
