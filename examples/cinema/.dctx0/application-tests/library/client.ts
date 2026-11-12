import {connect,type Socket} from 'node:net';
import {AssessmentError,protocolVersion,type Reply,type Request,type Summary} from './protocol.ts';
export type ClientOptions={maxMessageBytes?:number;timeoutMs?:number};
/** Explicit handles allow concurrent native tests without ambient global state.
 * Exchanges serialize on the local channel; case contexts belong to the server.
 */
export class AssessmentClient {
 private sequence=0;
 private tail:Promise<unknown>=Promise.resolve();
 private pending:{id:number;resolve:(value:Reply)=>void;reject:(error:Error)=>void;timer:ReturnType<typeof setTimeout>}|undefined;
 private buffered=Buffer.alloc(0);
 private failure?:Error;
 private constructor(private socket:Socket,private limit:number,private timeout:number){
  socket.on('data',chunk=>this.receive(typeof chunk==='string'?Buffer.from(chunk):chunk));
  socket.on('error',error=>this.fail(error));
  socket.on('close',()=>this.fail(new Error('Assessment channel closed')));
 }
 static async connect(path:string,options:ClientOptions={}):Promise<AssessmentClient>{
  const limit=options.maxMessageBytes??1048576,timeout=options.timeoutMs??30000;
  if(!Number.isSafeInteger(limit)||limit<1||limit>64*1024*1024||!Number.isSafeInteger(timeout)||timeout<1)throw new RangeError('Invalid assessment channel limits');
  const socket=connect({path:path.startsWith('@')?'\0'+path.slice(1):path});
  await new Promise<void>((resolve,reject)=>{
   const timer=setTimeout(()=>{socket.destroy();reject(new Error('Assessment connection timed out'));},timeout);
   socket.once('connect',()=>{clearTimeout(timer);resolve();});
   socket.once('error',error=>{clearTimeout(timer);reject(error);});
  });
  return new AssessmentClient(socket,limit,timeout);
 }
 private fail(error:Error):void {
  this.failure??=error;
  if(this.pending){clearTimeout(this.pending.timer);this.pending.reject(this.failure);this.pending=undefined;}
  this.socket.destroy();
 }
 private receive(chunk:Buffer):void {
  if(this.failure)return;
  if(this.buffered.length+chunk.length>this.limit){this.fail(new Error('Assessment frame exceeds byte limit'));return;}
  this.buffered=Buffer.concat([this.buffered,chunk]);
  const newline=this.buffered.indexOf(10);
  if(newline<0)return;
  if(newline!==this.buffered.length-1||!this.pending){this.fail(new Error('Unexpected assessment frame'));return;}
  try{
   const frame=JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(this.buffered));
   if(frame===null||typeof frame!=='object'||Object.keys(frame).sort().join(',')!=='id,payload,protocol_version'||frame.protocol_version!==protocolVersion||!Number.isSafeInteger(frame.id)||frame.id!==this.pending.id||!frame.payload||typeof frame.payload.kind!=='string')throw new Error('Invalid assessment frame or correlation');
   const pending=this.pending;this.pending=undefined;this.buffered=Buffer.alloc(0);clearTimeout(pending.timer);
   pending.resolve(frame.payload as Reply);
  }catch(error){this.fail(error instanceof Error?error:new Error(String(error)));}
 }
 exchange(request:Request):Promise<Reply>{
  const next=this.tail.then(()=>this.send(request));this.tail=next.catch(()=>{});return next;
 }
 private send(request:Request):Promise<Reply>{
  if(this.failure)return Promise.reject(this.failure);
  const id=++this.sequence;
  if(!Number.isSafeInteger(id))return Promise.reject(new Error('Assessment sequence exhausted'));
  const bytes=Buffer.from(JSON.stringify({protocol_version:protocolVersion,id,payload:request})+'\n');
  if(bytes.length>this.limit){const error=new Error('Assessment request exceeds byte limit');this.fail(error);return Promise.reject(error);}
  return new Promise<Reply>((resolve,reject)=>{
   const timer=setTimeout(()=>this.fail(new Error('Assessment reply timed out')),this.timeout);
   this.pending={id,resolve,reject,timer};
   this.socket.write(bytes,error=>{if(error)this.fail(error);});
  }).then(reply=>{if(reply.kind==='rejected')throw new AssessmentError(`${reply.issue.code}: ${reply.issue.message}`);return reply;});
 }
 async finish():Promise<Summary>{
  const reply=await this.exchange({kind:'finish'});
  if(reply.kind!=='finished')throw new Error('Expected assessment finish');
  const s=reply.summary;
  if(s.assessment_version!=='dctx0-application-assessment-2'||s.finalized!==true||s.status!=='PASS'||!Number.isSafeInteger(s.required_cases)||s.required_cases<0||s.completed_cases!==s.required_cases||s.missing_cases!==0||s.missing_tests!==0||s.issues!==0||s.issue_code!==null)throw new AssessmentError(`Application group incomplete: ${JSON.stringify(s)}`);
  return s;
 }
 close():void {this.socket.destroy();}
}
