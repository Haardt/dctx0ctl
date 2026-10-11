import {count,taskId,title} from './board.ts';
import {object,status,type Status,type Option} from './data.ts';
export type Task={id:string;title:string;status:Status};
export type Note={taskId:string;sequence:bigint;text:string};
export type TaskSummary={id:string;title:string};
export function task(value: unknown): Task {
  const record=object(value,['id','title','status']);return {id:taskId(record.id),title:title(record.title),status:status(record.status)};
}
export function note(value: unknown): Note {
  const record=object(value,['taskId','sequence','text']);return {taskId:taskId(record.taskId),sequence:count(record.sequence),text:title(record.text)};
}
export function taskSummary(value: unknown): TaskSummary {
  const record=object(value,['id','title']);return {id:taskId(record.id),title:title(record.title)};
}
export class Board {
  private readonly tasks:Task[];
  private readonly entries:Note[];
  constructor(tasks: unknown[]=[],notes: unknown[]=[]) {
    this.tasks=tasks.map(task);this.entries=notes.map(note);
    if(new Set(this.tasks.map(row=>row.id)).size!==this.tasks.length) throw new Error('Duplicate task identity');
    if(new Set(this.entries.map(row=>row.taskId+':'+row.sequence)).size!==this.entries.length) throw new Error('Duplicate composite note identity');
  }
  list(): TaskSummary[] {return this.tasks.map(row=>taskSummary({id:row.id,title:row.title}));}
  find(id: string): Option<TaskSummary> {
    const key=taskId(id),found=this.tasks.find(row=>row.id===key);
    return found?{tag:'some',value:taskSummary({id:found.id,title:found.title})}:{tag:'none'};
  }
  notes(id: string): Note[] {const key=taskId(id);return this.entries.filter(row=>row.taskId===key).map(note);}
  snapshot(): {Task:Task[];Note:Note[]} {return {Task:this.tasks.map(task),Note:this.entries.map(note)};}
}
