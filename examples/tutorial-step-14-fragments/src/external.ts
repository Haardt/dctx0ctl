import {object,flag} from './data.ts';
export type Health={ready:boolean};
export function health(value:unknown):Health {
  const record=object(value,['ready']);return {ready:flag(record.ready)};
}
export function healthHandler(ready:()=>boolean):(request:Request)=>Response {
  return request=>{
    if(new URL(request.url).pathname!=='/health')return new Response('Not found',{status:404});
    if(request.method!=='GET')return new Response('Method not allowed',{status:405});
    return Response.json(health({ready:ready()}));
  };
}
export class HealthClient {
  private readonly endpoint:URL;
  constructor(baseURL:string){this.endpoint=new URL('/health',baseURL);}
  async status():Promise<Health> {
    const response=await fetch(this.endpoint);
    if(!response.ok)throw new Error('Health request failed: '+response.status);
    return health(await response.json());
  }
}
export class Connectivity {
  constructor(private readonly query:()=>Promise<Health>) {}
  async ready():Promise<boolean>{return (await this.query()).ready;}
}
