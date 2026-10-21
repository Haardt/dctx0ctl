import Markdown from 'react-markdown';
import {usePinnedSnapshot} from './usePinnedSnapshot';
import {useTaskState} from './useTaskState';
import {prunedEvidence,type Json,type SiteSnapshot,type Snapshot} from './model';
function Text({children}:{children:string}){return <div className="markdown"><Markdown skipHtml components={{img:({alt})=><span>{alt??'Image'}</span>}}>{children}</Markdown></div>}
function record(value:Json,label=''):import('react').ReactNode {
 if(value===null)return <span className="muted">Not recorded</span>;
 if(typeof value==='string')return <Text>{value}</Text>;
 if(typeof value!=='object')return <span>{String(value)}</span>;
 if(Array.isArray(value))return value.length?<ul>{value.map((v,i)=><li key={i}>{record(v,label)}</li>)}</ul>:<span className="muted">None</span>;
 return <dl className="record-fields">{Object.entries(value).map(([key,v])=><div key={key}><dt>{key.replaceAll('_',' ')}</dt><dd>{record(v,key)}</dd></div>)}</dl>;
}
function source(snapshot:Snapshot|undefined,id:string){const span=snapshot?.ir.symbols[id]?.span??snapshot?.ir.nodes.find(n=>n.id===id)?.span;return span?snapshot?.sources.find(s=>s.file===span.file)?.text.split('\n').slice(Number(span.start.line)-1,Number(span.end.line)).join('\n'):undefined}
export default function TaskDetail({data,id,onSelect}:{data:SiteSnapshot;id:string;onSelect:(id:string)=>void}){
 const session=data.records.tasks,task=session?.definitions[id],current=data.tasks.find(t=>t.task_id===id);
 const detail=useTaskState(data,id),state=detail.state;
 const {snapshot,loading,error}=usePinnedSnapshot(data,task&&session?(current?session.plan.snapshot_id:task.origin_snapshot):null);
 if(!session||!task||!state)return null;
 if(detail.loading||detail.error)return <article className="task-description"><h2>T{id} · {task.candidate.title}</h2><p role={detail.error?"alert":"status"}>{detail.error??"Loading recorded task evidence…"}</p></article>;
 const candidate=session.plan.members.find(m=>m.task_id===id)?.candidate??task.candidate,events=session.events.filter(e=>e.task_id===id),links=session.links.filter(l=>l.from===id||l.to===id);
 return <article className="task-description"><small>T{id} · {candidate.kind} · {current?'Active plan':'Historical task'}</small><h2>{candidate.title}</h2><div className="counts"><span className={'tag '+state.work}>{state.work.toUpperCase()}</span><span className={'tag '+state.check}>{state.check.toUpperCase()}</span>{state.reservation&&<span className="tag">RESERVED</span>}</div><Text>{task.reason}</Text>
 <dl><dt>Actor</dt><dd>{state.actor??'Unassigned'}</dd><dt>Readiness</dt><dd>{current?.ready?'Ready':current?.delivered?'Delivered':current?.blockers.join('; ')||'Not open'}</dd><dt>Requirements</dt><dd>Snapshot {snapshot?.id??task.origin_snapshot}</dd></dl>
 <h3>Assignment</h3><pre><code className="language-dctx0">{candidate.contract}</code></pre><h3>Acceptance</h3><ul>{candidate.acceptance.map((a,i)=><li key={i}><Text>{a}</Text></li>)}</ul>
 {loading&&<p role="status">Loading pinned snapshot…</p>}{error&&<p role="alert">{error}</p>}
 <h3>Subjects</h3>{candidate.subjects.map(s=><a className="row" key={s} href={'#'+new URLSearchParams({view:'Spec',id:s,snapshot:current?session.plan.snapshot_id:task.origin_snapshot})}>{s}</a>)}
 {candidate.types.filter(t=>!candidate.subjects.includes(t)).length>0&&<details><summary>Required type contracts</summary>{candidate.types.filter(t=>!candidate.subjects.includes(t)).map(type=><section key={type}><h3>{type}</h3>{source(snapshot,type)?<pre><code className="language-dctx0">{source(snapshot,type)}</code></pre>:record(snapshot?.ir.data[type]??{status:'Unavailable in pinned snapshot'})}</section>)}</details>}
 {candidate.facts.length>0&&<details><summary>Constraints, examples and decisions</summary>{candidate.facts.map(fact=><section key={fact}><h3>{fact}</h3>{source(snapshot,fact)?<pre><code className="language-dctx0">{source(snapshot,fact)}</code></pre>:<Text>{snapshot?.ir.nodes.find(n=>n.id===fact)?.terms.map(t=>t.text).join(' ')??'Unavailable in pinned snapshot'}</Text>}</section>)}</details>}
 {state.execution_bundle?.documents.length&&<details><summary>Implementation guidance (pinned)</summary>{state.execution_bundle.documents.map((doc,i)=><section key={i}><h3>{doc.path}</h3><Text>{doc.text}</Text></section>)}</details>}
 <h3>Prerequisites and blockers</h3>{data.dependencies.filter(e=>e.task_id===id).map(e=><p key={e.prerequisite_id}><button className="link" onClick={()=>onSelect(e.prerequisite_id)}>T{e.prerequisite_id}</button> · {e.condition}<Text>{e.reason}</Text></p>)}{!data.dependencies.some(e=>e.task_id===id)&&<p className="muted">No current-plan prerequisites.</p>}
 <h3>Delivered result</h3>{record(state.result)}<h3>Check evidence</h3>{prunedEvidence(state.evidence,data).map(run=><p className="muted" key={run.id}>Historical {run.result.status} · {run.id}. Details pruned; replay and new evidence publication are unavailable.</p>)}{state.evidence.map((e,i)=><details key={i}><summary>Check {i+1}</summary>{record(e)}</details>)}{!state.evidence.length&&<p className="muted">No recorded checks. Done does not imply PASS.</p>}
 <h3>Reviews</h3>{state.reviews.map((r,i)=><section key={i}>{record(r)}</section>)}{!state.reviews.length&&<p className="muted">No recorded reviews.</p>}
 {task.changes.length>0&&<details><summary>Before / after requirements</summary>{record(task.changes)}</details>}
 <details><summary>History</summary>{links.map((l,i)=><p key={i}><button className="link" onClick={()=>onSelect(l.from)}>T{l.from}</button> → <button className="link" onClick={()=>onSelect(l.to)}>T{l.to}</button> · {l.kind}<Text>{l.reason}</Text></p>)}{events.map(e=><p key={e.seq}>#{e.seq} · {e.kind}<Text>{e.message}</Text></p>)}{!links.length&&!events.length&&<p className="muted">No recorded history.</p>}<small>History links are not scheduling prerequisites.</small></details>
 <details><summary>Technical record (JSON)</summary><pre>{JSON.stringify({definition:task,state},null,2)}</pre></details>
 </article>;
}
