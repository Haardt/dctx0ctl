import {expect,test} from 'bun:test';
import {Board} from './entities.ts';
import {BoardReader,Catalog} from './catalog.ts';
import {MissingTask,missingTask,taskViewed,type TaskViewed} from './events.ts';
const id='00000000-0000-0000-0000-000000000001';
test('error and event factories validate the exact payload boundary',()=>{
  expect(taskViewed({taskId:id})).toEqual({taskId:id});expect(missingTask({taskId:id})).toEqual({taskId:id});
  expect(()=>taskViewed({taskId:'bad'})).toThrow();expect(()=>missingTask({taskId:id,extra:true})).toThrow();
  const error=new MissingTask(id);expect(error.payload).toEqual({taskId:id});expect(error.name).toBe('MissingTask');
  expect(()=>new MissingTask('bad')).toThrow();
});
test('actual catalog emits only after successful retrieval and throws the declared error on a miss',async()=>{
  const published:TaskViewed[]=[],reader=new BoardReader(new Board([{id,title:'Read',status:'Open'}]));
  const catalog=new Catalog(reader,event=>published.push(event));
  expect(await catalog.requireTask(id)).toEqual({id,title:'Read'});expect(published).toEqual([{taskId:id}]);
  expect(await new Catalog(reader).requireTask(id)).toEqual({id,title:'Read'});
  try{await catalog.requireTask('00000000-0000-0000-0000-000000000002');throw new Error('Expected MissingTask');}
  catch(error){expect(error).toBeInstanceOf(MissingTask);expect((error as MissingTask).payload.taskId).toBe('00000000-0000-0000-0000-000000000002');}
  expect(published).toHaveLength(1);await expect(catalog.requireTask('bad')).rejects.toThrow();
});
