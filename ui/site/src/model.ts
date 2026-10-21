export type Json = null | boolean | string | number | Json[] | { [key: string]: Json };
export interface Span {file: string; start: {line: string; column: string}; end: {line: string; column: string}}
export interface Symbol {id: string; name: string; kind: string; parent: string | null; span: Span}
export interface SemanticNode {id: string; kind: string; parent: string|null; span: Span; terms: {text:string}[]; [key:string]: unknown}
export interface Snapshot {id:string; source_hash:string; sources:{file:string;text:string}[]; ir:{workspace:string;symbols:Record<string,Symbol>;nodes:SemanticNode[];data:Record<string,Json>;operations:Record<string,Json>;references:Json[]}; graph:{nodes:Record<string,Symbol>;edges:{from:string;to:string;kind:Json;origins:Span[]}[]}}
export interface Task {task_id:string;title:string;kind:string;subjects:string[];wave:string;work:string;check:string;ready:boolean;delivered:boolean;blockers:string[];actor:string|null;reservation:string|null;claim:string|null;result_summary:string|null}
export interface Candidate {key:string;title:string;subjects:string[];acceptance:string[];contract:string;wave:string;kind:string;facts:string[];types:string[]}
export interface Definition {id:string;candidate:Candidate;reason:string;origin_snapshot:string;before_snapshot:string|null;changes:Json[]}
export interface TaskState {details?:PackedJson;version?:string;execution_bundle?:{documents:{path:string;text:string;subjects:string[]}[]}|null;work:string;check:string;result:Json;evidence:Json[];reviews:Json[];actor:string|null;reservation:string|null;claim:string|null}
export interface Session {plan:{id:string;snapshot_id:string;check_run_id:string;parent:string|null;members:{task_id:string;candidate:Candidate}[]};event_seq:string;definitions:Record<string,Definition>;states:Record<string,TaskState>;links:{from:string;to:string;kind:string;reason:string}[];events:{seq:string;task_id:string|null;kind:string;message:string}[]}
export interface Verdict {status:'PASS'|'FAIL'|'UNKNOWN';issue?:{code:string;message:string}}
export interface Case {id:string;subject:string;kind:string;result:Verdict;[key:string]:unknown}
export interface Run {id:string;created_at:string;run:{origin:string;result:Verdict;provenance:{snapshot:string;check_occurrence:string;implementation:Record<string,string>;incomplete:Json[]};selected_subjects:string[];required_subjects:string[];not_selected?:string[];cases:Case[];case_count?:string;required_case_count?:string;case_inventory_complete?:boolean;case_subjects?:string[];case_details?:string;case_details_encoding?:string;required_cases:string[];open_obligations:Json[];artifacts:Json[];[key:string]:unknown}}
export interface RunSummary {id:string;created_at:string;result:Verdict;selected_subjects:string[];details_available:boolean;pruned_at:string|null;case_count:string;outcomes:string[]}
export interface PackedJson {encoding:"gzip-base64-json-decimal-integers-v1";data:string}
export interface SiteSnapshot {schema_version:1;project:string;notice:string;tasks:Task[];dependencies:{task_id:string;prerequisite_id:string;condition:string;reason:string}[];records:{snapshot:Snapshot;check:{run_id:string;timestamp:string};tasks:Session|null;task_snapshot:Snapshot|null;requirement_snapshots?:Record<string,Snapshot|PackedJson>;history_scope?:"previous"|"all";omitted_requirement_snapshots?:string[];runs:Run[];run_summaries?:RunSummary[];history:Json[];renames:Json[];changes:Json;before:Snapshot|null;plan_diff:Json}}
export function applicable(run:Run,data:SiteSnapshot) {return run.run.provenance.snapshot===data.records.snapshot.id && run.run.provenance.check_occurrence===data.records.check.run_id}
export function evidenceStatus(run:Run,data:SiteSnapshot) {if(!applicable(run,data))return 'NOT APPLICABLE';if(run.run.result.status==='PASS'&&(run.run.provenance.incomplete.length||run.run.open_obligations.length||(run.run.not_selected?run.run.not_selected.length>0:run.run.selected_subjects.length<run.run.required_subjects.length)||!(run.run.case_inventory_complete??(run.run.cases.length===run.run.required_cases.length))||(run.run.required_case_count??String(run.run.required_cases.length))==='0'))return 'UNKNOWN';return run.run.result.status}
export function ancestorIds(id:string,edges:SiteSnapshot['dependencies']) {const ids=new Set([id]);let size=0;while(size!==ids.size){size=ids.size;for(const e of edges)if(ids.has(e.task_id))ids.add(e.prerequisite_id)}return ids}

export async function decodeCases(run:Run):Promise<Case[]> {
 if(!run.run.case_details)return run.run.cases;
 const encoding=run.run.case_details_encoding;
 if(!['gzip-base64-json-decimal-integers-v1','gzip-base64-json-decimal-integers-v2'].includes(encoding??''))throw new Error('Unsupported case detail encoding');
 const bytes=Uint8Array.from(atob(run.run.case_details),char=>char.charCodeAt(0));
 const stream=new Blob([bytes]).stream().pipeThrough(new DecompressionStream('gzip'));
 const decoded=JSON.parse(await new Response(stream).text());
 const cases=(encoding?.endsWith('-v2')?decoded.cases:decoded) as Case[];
 if(encoding?.endsWith('-v2')) {
  if(String(cases.length)!==run.run.case_count||String(decoded.required_cases.length)!==run.run.required_case_count)throw new Error('Case inventory mismatch');
 }else if(cases.length!==run.run.cases.length||cases.some((c,i)=>c.id!==run.run.cases[i].id))throw new Error('Case inventory mismatch');
 return cases;
}

export function pinnedSnapshot(data:SiteSnapshot,id:string|null):Snapshot|undefined {
 const snapshots=[data.records.snapshot,data.records.task_snapshot,data.records.before,...Object.values(data.records.requirement_snapshots??{})];
 return snapshots.find((s):s is Snapshot=>!!s&&'id' in s&&s.id===id)??undefined;
}

// Link an effective inspection only to the exact captured native execution.
export function reviewedAssessment(run:Run,data:SiteSnapshot):Run|undefined {
 if(evidenceStatus(run,data)!=='UNKNOWN'||run.run.origin!=='native'||!run.run.case_details||!run.run.open_obligations.length||run.run.provenance.incomplete.length)return;
 if(run.run.open_obligations.some(o=>!o||typeof o!=='object'||Array.isArray(o)||!['MANUAL_REQUIREMENT','SUITE_INSPECTION_REQUIRED'].includes(String(o.code))))return;
 return data.records.runs.find(other=>other.run.origin==='imported'&&evidenceStatus(other,data)==='PASS'&&other.run.case_details===run.run.case_details&&JSON.stringify(other.run.provenance)===JSON.stringify(run.run.provenance)&&other.run.artifacts.some(a=>a&&typeof a==='object'&&!Array.isArray(a)&&a.media_type==='application/vnd.dctx0.program-inspections+json')&&containsArtifacts(other.run.artifacts,run.run.artifacts));
}
export function evidenceDisplayStatus(run:Run,data:SiteSnapshot) {return reviewedAssessment(run,data)?'REVIEWED':evidenceStatus(run,data)}

function containsArtifacts(actual:Json[],required:Json[]) {const hashes=new Set(actual.map(a=>JSON.stringify(a)));return required.every(a=>hashes.has(JSON.stringify(a)))}

// Containment follows declared parents, independently of semantic dependency cycles.
export function descendantIds(root:string,depth:number,symbols:Record<string,Symbol>):Set<string> {
 const visible=new Set<string>();if(!symbols[root])return visible;
 const children=new Map<string,string[]>();for(const symbol of Object.values(symbols)){if(symbol.parent){const ids=children.get(symbol.parent)??[];ids.push(symbol.id);children.set(symbol.parent,ids)}}
 visible.add(root);let frontier=[root];for(let level=0;level<depth&&frontier.length;level++){const next:string[]=[];for(const parent of frontier)for(const id of children.get(parent)??[])if(!visible.has(id)){visible.add(id);next.push(id)}frontier=next}return visible;
}

export function prunedEvidence(evidence:Json[],data:SiteSnapshot):RunSummary[] {
 const ids=new Set<string>();
 for(const item of evidence) {
  const report=item&&typeof item==='object'&&!Array.isArray(item)?item.report:undefined;
  if(typeof report!=='string')continue;
  if(/^[a-fA-F0-9]{64}$/.test(report)){ids.add(report);continue}
  try {const parsed=JSON.parse(report);if(typeof parsed?.native_run==='string')ids.add(parsed.native_run)}catch{}
 }
 return (data.records.run_summaries??[]).filter(run=>ids.has(run.id)&&!run.details_available);
}
