import {expect,test} from 'bun:test';
import {Board} from './entities.ts';
import {BoardHttpClient,serveBoard} from './http.ts';
const id='00000000-0000-0000-0000-000000000001';
test('actual board HTTP routing returns public messages and maps absent tasks to None',async()=>{
  const board=new Board([{id,title:'Read',status:'Open'}]),server=serveBoard(board);
  try {
    const client=new BoardHttpClient(server.url.href);
    const page=await fetch(server.url);expect(page.status).toBe(200);expect(page.headers.get('content-type')).toContain('text/html');
    expect(await page.text()).toContain('Dein MiniBoard.');
    expect(await client.list()).toEqual([{id,title:'Read'}]);expect(await client.find(id)).toEqual({tag:'some',value:{id,title:'Read'}});
    expect(await client.find('00000000-0000-0000-0000-000000000002')).toEqual({tag:'none'});
    expect((await fetch(new URL('/missing',server.url))).status).toBe(404);
    expect((await fetch(new URL('/tasks',server.url),{method:'PUT'})).status).toBe(405);
    expect((await fetch(new URL('/tasks/bad',server.url))).status).toBe(400);
    expect((await fetch(new URL('/tasks/%FF',server.url))).status).toBe(400);
    await expect(client.find('bad')).rejects.toThrow();
    expect(await client.rows()).toEqual([{id,title:'Read',status:'Open'}]);
    const added={id:'00000000-0000-0000-0000-000000000003',title:'New',status:'Open' as const};
    expect(await client.insert(added)).toEqual(added);
    expect(await client.replace({...added,status:'Done'})).toEqual({...added,status:'Done'});
    expect(board.listTasks()[1]!.status).toBe('Done');
    await expect(client.insert(added)).rejects.toThrow('already exists');
    await expect(client.replace({...added,id:'00000000-0000-0000-0000-000000000004'})).rejects.toThrow('does not exist');
    const send=(path:string,body:string)=>fetch(new URL(path,server.url),{method:'PATCH',body,headers:{'content-type':'application/json'}});
    expect((await send('/tasks/'+id,JSON.stringify(added))).status).toBe(400);
    expect((await send('/tasks/%FF',JSON.stringify(added))).status).toBe(400);
    expect((await send('/tasks/'+id,'{')).status).toBe(400);
    expect((await send('/tasks/'+id,JSON.stringify({id,title:'',status:'Open'}))).status).toBe(400);
    const copy=board.listTasks();copy[0]!.title='Outside';expect(board.listTasks()[0]!.title).toBe('Read');
  }finally{await server.stop(true);}
});
test('client rejects actual HTTP failures and malformed list/single payloads',async()=>{
  let mode='failure';const server=Bun.serve({hostname:'127.0.0.1',port:0,fetch:()=>mode==='failure'?new Response('Unavailable',{status:503}):Response.json(mode==='list'?{}:{id,title:1})});
  try {
    const client=new BoardHttpClient(server.url.href,{'x-example':'present'});
    await expect(client.list()).rejects.toThrow('503');await expect(client.find(id)).rejects.toThrow('503');
    await expect(client.rows()).rejects.toThrow('503');await expect(client.insert({id,title:'Read',status:'Open'})).rejects.toThrow('503');
    mode='list';await expect(client.list()).rejects.toThrow('Expected task summary list');
    await expect(client.rows()).rejects.toThrow('Expected task list');
    await expect(client.replace({id,title:'Read',status:'Open'})).rejects.toThrow();
    mode='single';await expect(client.find(id)).rejects.toThrow();
  }finally{await server.stop(true);}
  expect(()=>new BoardHttpClient('relative')).toThrow();
});
