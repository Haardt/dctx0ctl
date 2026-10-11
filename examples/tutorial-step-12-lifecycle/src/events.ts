import {taskId} from './board.ts';
import {object} from './data.ts';
export type TaskViewed={taskId:string};
export function taskViewed(value: unknown): TaskViewed {
  const fields=object(value,['taskId']);return {taskId:taskId(fields.taskId)};
}
export function missingTask(value: unknown): {taskId:string} {
  const fields=object(value,['taskId']);return {taskId:taskId(fields.taskId)};
}
export class MissingTask extends Error {
  readonly payload:{taskId:string};
  constructor(id: string){super('Task does not exist');this.name='MissingTask';this.payload=missingTask({taskId:id});}
}

export class TaskConflict extends Error {
  readonly payload:{taskId:string};
  constructor(id: string){super('Task identity already exists');this.name='TaskConflict';this.payload=missingTask({taskId:id});}
}
