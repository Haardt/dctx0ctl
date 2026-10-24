/** Testing-only wire contract. Production imports none of these declarations. */
export type Value =
 | {kind:'unit'} | {kind:'boolean';value:boolean}
 | {kind:'integer'|'decimal'|'float'|'string';value:string}
 | {kind:'record';value:Record<string,Value>} | {kind:'list';value:Value[]}
 | {kind:'option';value:Value|null}
 | {kind:'variant';value:{declaration:string;name:string;payload:Value|null}}
 | {kind:'failure';value:{declaration:string;payload:Value|null}};
export type Invocation = {subject:string;arguments:Value[]};
export type Observation = {kind:'returned';value:Value}|{kind:'threw';message:string}|{kind:'unavailable';reason:string};
export type ValueCheck = {kind:'accepted';value:Value}|{kind:'rejected'|'unavailable';reason:string};
export type AdapterDescription = {
 protocol_version:1;
 subjects:{subject:string;role:string;artifact:string;entry_point:string}[];
 artifacts:string[];capabilities:string[];representations:Record<string,string>;
};
export type ResourceObservation = {operation?:string;resource:string;access:'read'|'write'|'observe'};
export type TestContext = {
 collaborator(invocation:Invocation):Promise<Observation>;
 emit(subject:string,payload:Value):void;
 recordCall(invocation:Invocation,observation:Observation):void;
 recordResource(observation:ResourceObservation):void;
};
export type TestSession = {
 invoke(invocation:Invocation):Promise<Observation>;
 validate?(subject:string,value:Value):Promise<ValueCheck>;
 observe?(entities:string[]):Promise<Record<string,Value[]>>;
 close():Promise<void>;
};
export type ProjectAdapter = {
 describe():Promise<AdapterDescription>;
 begin(caseId:string,context:TestContext):Promise<TestSession>;
};

/** Capture only the actual application call; translation errors stay technical. */
export async function invokeApplication<T>(operation:()=>T|Promise<T>,encode:(value:T)=>Value):Promise<Observation>{
 let value:T;
 try{value=await operation();}catch(error){return {kind:'threw',message:error instanceof Error?error.message:String(error)};}
 try{return {kind:'returned',value:encode(value)};}catch(error){throw new AdapterError(error instanceof Error?error.message:String(error));}
}

/** Testing adapters may explicitly identify a technical cause. Production stays independent. */
export class AdapterError extends Error {}
export class EnvironmentError extends Error {}
