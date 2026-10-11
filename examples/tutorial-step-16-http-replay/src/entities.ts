import {MissingTask,TaskConflict} from './events.ts';
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
  listTasks(): Task[] {return this.tasks.map(task);}
  find(id: string): Option<TaskSummary> {
    const key=taskId(id),found=this.tasks.find(row=>row.id===key);
    return found?{tag:'some',value:taskSummary({id:found.id,title:found.title})}:{tag:'none'};
  }
  notes(id: string): Note[] {const key=taskId(id);return this.entries.filter(row=>row.taskId===key).map(note);}
  insert(value: unknown): Task {
    const row=task(value);
    if(this.tasks.some(entry=>entry.id===row.id))throw new TaskConflict(row.id);
    this.tasks.push(row);return task(row);
  }
  replace(value: unknown): Task {
    const row=task(value),index=this.tasks.findIndex(entry=>entry.id===row.id);
    if(index<0)throw new MissingTask(row.id);
    this.tasks[index]=row;return task(row);
  }
  save(value: unknown): Task {
    const row=task(value),index=this.tasks.findIndex(entry=>entry.id===row.id);
    if(index<0)this.tasks.push(row);else this.tasks[index]=row;
    return task(row);
  }
  remove(id: string): undefined {
    const key=taskId(id),index=this.tasks.findIndex(entry=>entry.id===key);
    if(index>=0)this.tasks.splice(index,1);return undefined;
  }
  saveNote(value: unknown): Note {
    const row=note(value),index=this.entries.findIndex(entry=>entry.taskId===row.taskId&&entry.sequence===row.sequence);
    if(index<0)this.entries.push(row);else this.entries[index]=row;
    return note(row);
  }
  snapshot(): {Task:Task[];Note:Note[]} {return {Task:this.tasks.map(task),Note:this.entries.map(note)};}
}
