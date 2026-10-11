import {Board,task,taskSummary,type Task,type TaskSummary} from './entities.ts';
import {MissingTask,TaskConflict} from './events.ts';
import {taskId} from './board.ts';
import type {Option} from './data.ts';
import {boardPage} from './ui.ts';
export function boardHandler(board:Board):(request:Request)=>Promise<Response> {
  return async request=>{
    const path=new URL(request.url).pathname;
    if((path==='/tasks'&&request.method==='POST')||(path.startsWith('/tasks/')&&request.method==='PATCH')) {
      try {
        const row=task(await request.json());
        if(request.method==='POST')return Response.json(board.insert(row),{status:201});
        if(row.id!==decodeURIComponent(path.slice(7)))throw new TypeError('Task identity does not match path');
        return Response.json(board.replace(row));
      }catch(error){
        const status=error instanceof MissingTask?404:error instanceof TaskConflict?409:400;
        return new Response(error instanceof Error?error.message:String(error),{status});
      }
    }
    if(request.method!=='GET')return new Response('Method not allowed',{status:405});
    if(path==='/')return new Response(boardPage(),{headers:{'content-type':'text/html; charset=utf-8'}});
    if(path==='/board/tasks')return Response.json(board.listTasks());
    if(path==='/tasks')return Response.json(board.list());
    if(path.startsWith('/tasks/')) {
      try {
        const result=board.find(decodeURIComponent(path.slice(7)));
        return result.tag==='none'?new Response('Task not found',{status:404}):Response.json(result.value);
      }catch(error){return new Response(error instanceof Error?error.message:String(error),{status:400});}
    }
    return new Response('Not found',{status:404});
  };
}
export class BoardHttpClient {
  constructor(private readonly baseURL:string,private readonly headers:HeadersInit={}) {new URL(baseURL);}
  async list():Promise<TaskSummary[]> {
    const response=await fetch(new URL('/tasks',this.baseURL),{headers:this.headers});
    if(!response.ok)throw new Error('Task list failed: '+response.status);
    const value:unknown=await response.json();
    if(!Array.isArray(value))throw new TypeError('Expected task summary list');
    return value.map(taskSummary);
  }
  async find(id:string):Promise<Option<TaskSummary>> {
    const response=await fetch(new URL('/tasks/'+encodeURIComponent(taskId(id)),this.baseURL),{headers:this.headers});
    if(response.status===404)return {tag:'none'};
    if(!response.ok)throw new Error('Task lookup failed: '+response.status);
    return {tag:'some',value:taskSummary(await response.json())};
  }
  async rows():Promise<Task[]> {
    const response=await fetch(new URL('/board/tasks',this.baseURL),{headers:this.headers});
    if(!response.ok)throw new Error('Board tasks failed: '+response.status);
    const value:unknown=await response.json();
    if(!Array.isArray(value))throw new TypeError('Expected task list');
    return value.map(task);
  }
  async insert(value:Task):Promise<Task> {return this.write('/tasks','POST',value);}
  async replace(value:Task):Promise<Task> {return this.write('/tasks/'+encodeURIComponent(taskId(value.id)),'PATCH',value);}
  private async write(path:string,method:string,value:Task):Promise<Task> {
    const row=task(value),headers=new Headers(this.headers);headers.set('content-type','application/json');
    const response=await fetch(new URL(path,this.baseURL),{method,headers,body:JSON.stringify(row)});
    if(response.status===409)throw new TaskConflict(row.id);
    if(response.status===404)throw new MissingTask(row.id);
    if(!response.ok)throw new Error('Task write failed: '+response.status);
    return task(await response.json());
  }
}
export function serveBoard(board:Board,port=0) {
  return Bun.serve({hostname:'127.0.0.1',port,fetch:boardHandler(board)});
}
