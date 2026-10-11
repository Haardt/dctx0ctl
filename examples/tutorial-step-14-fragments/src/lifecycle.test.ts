import {expect,test} from 'bun:test';
import {Board} from './entities.ts';
const id='00000000-0000-4000-8000-000000000055';
test('connected task lifecycle uses the same actual stored entity and independent sessions start fresh',()=>{
  const initial={id,title:'Lifecycle',status:'Open' as const};const board=new Board([initial]);
  expect(board.snapshot().Task[0]!.status).toBe('Open');
  board.replace({...initial,status:'Done'});expect(board.snapshot().Task[0]!.status).toBe('Done');
  board.replace({...initial,status:'Archived'});expect(board.snapshot().Task[0]!.status).toBe('Archived');
  expect(new Board([initial]).snapshot().Task[0]!.status).toBe('Open');
});
