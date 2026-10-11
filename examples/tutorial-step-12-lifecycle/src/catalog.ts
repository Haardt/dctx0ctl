import {MissingTask,taskViewed,type TaskViewed} from './events.ts';
import {taskId} from './board.ts';
import {Board,type TaskSummary} from './entities.ts';
import type {Option} from './data.ts';
export interface TaskReader {lookup(id: string): Option<TaskSummary>|Promise<Option<TaskSummary>>;}
export class BoardReader implements TaskReader {
  constructor(private readonly board: Board) {}
  lookup(id: string): Option<TaskSummary> {return this.board.find(taskId(id));}
}
export class Catalog {
  constructor(private readonly reader: TaskReader,private readonly publish:(event:TaskViewed)=>void=()=>{}) {}
  async requireTask(id: string): Promise<TaskSummary> {
    const key=taskId(id),found=await this.find(key);
    if(found.tag==='none')throw new MissingTask(key);
    this.publish(taskViewed({taskId:key}));return found.value;
  }
  async find(id: string): Promise<Option<TaskSummary>> {return await this.reader.lookup(taskId(id));}
}
