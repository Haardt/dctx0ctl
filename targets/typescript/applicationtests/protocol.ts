/** Application assessment is independent of SDK 2 / worker protocol 1. */
export const protocolVersion = 2;
export const libraryVersion = '0.2.0';
export type Value =
 | {kind:'unit'} | {kind:'boolean';value:boolean} | {kind:'string'|'integer'|'decimal'|'float';value:string}
 | {kind:'list';value:Value[]} | {kind:'record';value:Record<string,Value>}
 | {kind:'option';value:Value|null}
 | {kind:'variant';value:{declaration:string;name:string;payload:Value|null}}
 | {kind:'failure';value:{declaration:string;payload:Value|null}};
export type TypeRef = {name:string;symbol_id:string|null;arguments:TypeRef[]};
export type Issue = {code:string;message:string};
export type Verdict = {status:'PASS'|'FAIL'|'UNKNOWN';issue?:Issue};
export type DecodeResult<T> = {kind:'accepted';value:T}|{kind:'rejected';reason:string};
export type DecoderOutcome = {kind:'accepted';value:Value}|{kind:'rejected';reason:string}|{kind:'exception'|'binding-error';message:string};
export type Invocation={subject:string;arguments:Value[]};
export type Observation={kind:'returned';value:Value}|{kind:'threw';message:string}|{kind:'unavailable';reason:string};
export type InvocationObservation={invocation:Invocation;observation:Observation};
export type VerificationEvent={subject:string;payload:Value};
export type ResourceObservation={operation?:string|null;resource:string;access:'read'|'write'|'observe'};
export type InvocationEvidence={observation:Observation;calls:InvocationObservation[]|null;events:VerificationEvent[]|null;resources:ResourceObservation[]|null};
export type StateEvidence={entities:Record<string,Value[]>;complete:boolean};
export type Directive = {kind:'decode';subject:string;ty:TypeRef;raw:number[]}|{kind:'construct';subject:string;ty:TypeRef;value:Value}|{kind:'invoke';invocation:Invocation;phase:'setup'|'production'|'oracle'}|{kind:'read-state';entities:string[];phase:'before'|'after';invocation:Invocation}|{kind:'verdict';result:Verdict};
export type Summary = {assessment_version:string;group:string;finalized:boolean;required_cases:number;completed_cases:number;missing_cases:number;missing_tests:number;issues:number;status:'PASS'|'FAIL'|'UNKNOWN';issue_code:string|null};
export type Request =
 | {kind:'register'|'next'|'close-test';test:string}
 | {kind:'observe';context:number;observation:{kind:'decoded';raw:number[];outcome:DecoderOutcome}|{kind:'constructed';input:Value;outcome:DecoderOutcome}|{kind:'invoked';invocation:Invocation;observation:Observation;calls:InvocationObservation[]|null;events:VerificationEvent[]|null;resources:ResourceObservation[]|null}|{kind:'state';entities:Record<string,Value[]>;phase:'before'|'after';invocation:Invocation;complete:boolean}|{kind:'binding-error';message:string}}
 | {kind:'close-case';context:number;cleanup_error:string|null}
 | {kind:'collaborator';context:number;invocation:Invocation}
 | {kind:'finish'};
export type Reply =
 | {kind:'registered'|'no-more-cases'|'test-closed'}
 | {kind:'case';context:number;id:string;form:string;directive:Directive;mocks?:string[]}
 | {kind:'action';context:number;directive:Directive;mocks?:string[]}
 | {kind:'case-closed';context:number;result:Verdict}
 | {kind:'collaborator-result';context:number;observation:Observation}
 | {kind:'finished';summary:Summary}
 | {kind:'rejected';issue:Issue};
export class BindingError extends Error {}
export class AssessmentError extends Error {}
export function message(error:unknown):string {return error instanceof Error?error.message:String(error);}
export function assertPass(result:Verdict, label:string):void {
 if(result.status!=='PASS')throw new AssessmentError(`${label}: ${result.status} ${result.issue?.code??''} ${result.issue?.message??''}`);
}
