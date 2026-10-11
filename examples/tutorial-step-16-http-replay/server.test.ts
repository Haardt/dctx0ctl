import {expect,test} from 'bun:test';

test('the actual entry point starts the board server with editable real seed tasks',async()=>{
  const previous=process.env.PORT;process.env.PORT='0';
  const {server}=await import('./server.ts');
  try {
    const page=await fetch(server.url);expect(await page.text()).toContain('id="new-task"');
    const rows=await (await fetch(new URL('/board/tasks',server.url))).json() as {id:string,title:string,status:string}[];
    expect(rows.map(row=>row.title)).toEqual(['Spec lesen','Code prüfen','Board ansehen']);
    expect(rows[2]!.status).toBe('Done');
    const first=rows[0]!;
    const response=await fetch(new URL('/tasks/'+first.id,server.url),{method:'PATCH',headers:{'content-type':'application/json'},body:JSON.stringify({...first,status:'Done'})});
    expect(response.status).toBe(200);
    expect((await(await fetch(new URL('/board/tasks',server.url))).json() as {status:string}[])[0]!.status).toBe('Done');
  }finally{
    await server.stop(true);
    if(previous===undefined)delete process.env.PORT;else process.env.PORT=previous;
  }
});
