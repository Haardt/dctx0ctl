import {expect,test} from 'bun:test';
import {health,healthHandler,HealthClient,Connectivity} from './external.ts';
test('HTTP client and consumer execute the actual local handler',async()=>{
  let ready=true;const server=Bun.serve({hostname:'127.0.0.1',port:0,fetch:healthHandler(()=>ready)});
  try {
    const client=new HealthClient(server.url.href),consumer=new Connectivity(()=>client.status());
    expect(await client.status()).toEqual({ready:true});expect(await consumer.ready()).toBe(true);
    ready=false;expect(await consumer.ready()).toBe(false);
    expect((await fetch(new URL('/missing',server.url))).status).toBe(404);
    expect((await fetch(new URL('/health',server.url),{method:'POST'})).status).toBe(405);
  }finally{await server.stop(true);}
});
test('real HTTP errors and invalid response payloads are rejected',async()=>{
  let invalid=false;const server=Bun.serve({hostname:'127.0.0.1',port:0,fetch:()=>invalid?Response.json({ready:'yes'}):new Response('Unavailable',{status:503})});
  try {
    const client=new HealthClient(server.url.href);await expect(client.status()).rejects.toThrow('503');
    invalid=true;await expect(client.status()).rejects.toThrow();
  }finally{await server.stop(true);}
  expect(()=>new HealthClient('relative')).toThrow();expect(()=>health({ready:true,extra:1})).toThrow();
});
