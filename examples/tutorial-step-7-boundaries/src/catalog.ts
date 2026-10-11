import {taskId} from './board.ts';
import {Board,type TaskSummary} from './entities.ts';
import type {Option} from './data.ts';
export interface TaskReader {lookup(id: string): Option<TaskSummary>;}
export class BoardReader implements TaskReader {
  constructor(private readonly board: Board) {}
  lookup(id: string): Option<TaskSummary> {return this.board.find(taskId(id));}
}
export class Catalog {
  constructor(private readonly reader: TaskReader) {}
  find(id: string): Option<TaskSummary> {return this.reader.lookup(taskId(id));}
}
