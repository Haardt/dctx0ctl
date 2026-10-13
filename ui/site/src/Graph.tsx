import {useEffect,useMemo,useState} from 'react';
import {ReactFlow,Background,Controls,Position,applyNodeChanges,type Node,type Edge,type Viewport} from '@xyflow/react';
import dagre from '@dagrejs/dagre';
import {Button} from '@heroui/react';
import {ancestorIds,type SiteSnapshot} from './model';
import {bundleRelations,type Relation} from './relations';
export function readLocal<T>(key:string,fallback:T):T {try{return JSON.parse(localStorage.getItem(key)??'null')??fallback}catch{return fallback}}
export function saveLocal(key:string,value:unknown){try{localStorage.setItem(key,JSON.stringify(value))}catch{/* Private/file storage may be unavailable. */}}
export default function Graph({data,architecture,selected,onSelect,dark}:{data:SiteSnapshot;architecture:boolean;selected:string;onSelect:(id:string)=>void;dark:boolean}){
 const [mode,setMode]=useState(selected&&!architecture?'focus':'all'),[history,setHistory]=useState(false),[expanded,setExpanded]=useState<string[]>([]),[expandedWaves,setExpandedWaves]=useState<string[]>([]);
 const key=`site:${data.project}:${architecture?'architecture':'tasks'}:horizontal-v2`;
 const [viewport,setViewport]=useState<Viewport|undefined>(()=>readLocal(key+':viewport',undefined));
 const layout=useMemo(()=>{
  let nodes:Node[]=[],relations:Relation[]=[];
  if(architecture){
   const symbols=Object.values(data.records.snapshot.ir.symbols);
   const groups=symbols.filter(s=>['workspace','domain','module'].includes(s.kind));
   const visible=new Set(groups.map(s=>s.id));
   for(const s of symbols)if(expanded.some(g=>s.id.startsWith(g+'.'))||s.id===selected)visible.add(s.id);
   const parents=new Map([...symbols.map(s=>[s.id,s.parent] as const),...data.records.snapshot.ir.nodes.map(n=>[n.id,n.parent] as const)]);
   function owner(id:string){let current:string|null=id;const seen=new Set<string>();while(current&&!visible.has(current)){if(seen.has(current))return '';seen.add(current);current=parents.get(current)??null}return current??''}
   nodes=symbols.filter(s=>visible.has(s.id)).map(s=>({id:s.id,data:{label:`${s.name} · ${s.kind}${['domain','module'].includes(s.kind)?' (expand)':''}`},position:{x:0,y:0}}));
   for(const e of data.records.snapshot.graph.edges){const from=owner(e.from),to=owner(e.to);if(from&&to&&from!==to){const label=typeof e.kind==='string'?e.kind:Object.values(e.kind??{}).join(' · ');relations.push({source:from,target:to,kind:label,category:'semantic',from:e.from,to:e.to})}}
  }else{
   const focus=ancestorIds(selected,data.dependencies);
   let tasks=data.tasks.filter(t=>mode==='focus'?focus.has(t.task_id):mode==='active'?t.work!=='done':true);
   const ids=new Set(tasks.map(t=>t.task_id));
   if(history){for(const l of data.records.tasks?.links??[])if(ids.has(l.to)||ids.has(l.from)){ids.add(l.from);ids.add(l.to)}
    const added=[...ids].filter(id=>!tasks.some(t=>t.task_id===id));
    for(const id of added){const d=data.records.tasks?.definitions[id],s=data.records.tasks?.states[id];if(d&&s)tasks.push({task_id:id,title:d.candidate.title,kind:d.candidate.kind,subjects:d.candidate.subjects,wave:d.candidate.wave,work:s.work,check:s.check,ready:false,delivered:false,blockers:[],actor:s.actor,reservation:s.reservation,claim:s.claim,result_summary:null})}
   }
   const owners=new Map<string,string>(),waves=new Map<string,typeof tasks>();
   for(const task of tasks){const wave=waves.get(task.wave)??[];wave.push(task);waves.set(task.wave,wave);owners.set(task.task_id,mode==='focus'||expandedWaves.includes(task.wave)?task.task_id:`wave:${task.wave}`)}
   for(const [wave,members]of waves){
    if(mode==='focus'||expandedWaves.includes(wave))nodes.push(...members.map(t=>({id:t.task_id,data:{label:`T${t.task_id} · ${t.title}\n${t.work} / ${t.check}${t.reservation?' · reserved':''}`},position:{x:0,y:0},className:t.task_id===selected?'selected':''})));
    else nodes.push({id:`wave:${wave}`,data:{label:`Wave ${wave}\n${members.length} tasks · expand`,wave},position:{x:0,y:0},className:'wave-group'});
   }
   relations=data.dependencies.filter(e=>ids.has(e.task_id)&&ids.has(e.prerequisite_id)).map(e=>({source:owners.get(e.prerequisite_id)!,target:owners.get(e.task_id)!,from:e.prerequisite_id,to:e.task_id,kind:e.condition,category:'prerequisite',reason:e.reason}));
   if(history)for(const l of data.records.tasks?.links??[])if(ids.has(l.from)&&ids.has(l.to))relations.push({source:owners.get(l.from)!,target:owners.get(l.to)!,from:l.from,to:l.to,kind:'history',category:'history',reason:l.reason});
  }
  const bundles=bundleRelations(relations);
  const edges:Edge[]=bundles.map(bundle=>({id:bundle.id,source:bundle.source,target:bundle.target,label:bundle.relations.length>1?String(bundle.relations.length):undefined,data:{bundle},className:bundle.category==='history'?'history-edge':'relation-edge',style:{strokeDasharray:bundle.category==='history'?'5 5':undefined}}));
  const graph=new dagre.graphlib.Graph().setGraph({rankdir:'LR',ranksep:90,nodesep:35}).setDefaultEdgeLabel(()=>({}));
  for(const n of nodes)graph.setNode(n.id,{width:180,height:68});for(const e of edges)graph.setEdge(e.source,e.target);dagre.layout(graph);
  const saved=readLocal<Record<string,{x:number;y:number}>>(key+':positions',{});
  return {nodes:nodes.map<Node>(n=>({...n,data:{...n.data,label:<span className="graph-node-label" title={String(n.data.label)}>{String(n.data.label)}</span>},sourcePosition:Position.Right,targetPosition:Position.Left,position:saved[n.id]??{x:graph.node(n.id).x-90,y:graph.node(n.id).y-34}})),edges,bundles,relationCount:relations.length};
 },[data,architecture,mode,selected,history,expanded,expandedWaves,key]);
 const [nodes,setNodes]=useState(layout.nodes),[edgeId,setEdgeId]=useState('');
 useEffect(()=>setNodes(layout.nodes),[layout]);
 const edgeDetail=layout.bundles.find(bundle=>bundle.id===edgeId);
 return <><div className="filters graph-filters">{!architecture&&<><label>Graph scope <select value={mode} onChange={e=>setMode(e.target.value)}><option value="all">Whole plan</option><option value="active">Active work</option><option value="focus">Selected task prerequisites</option></select></label><Button variant="secondary" onPress={()=>setExpandedWaves([])}>Collapse waves</Button><label><input type="checkbox" checked={history} onChange={e=>setHistory(e.target.checked)}/> Historical links</label></>}{architecture&&<Button variant="secondary" onPress={()=>setExpanded([])}>Collapse groups</Button>}<small>{nodes.length} nodes · {layout.relationCount} relationships{layout.edges.length!==layout.relationCount?` · ${layout.edges.length} connections`:""}</small></div>
 <div className="graph"><ReactFlow colorMode={dark?'dark':'light'} nodes={nodes} edges={layout.edges.map(edge=>({...edge,selected:edge.id===edgeId}))} onNodesChange={changes=>setNodes(n=>applyNodeChanges(changes,n))} onNodeDragStop={(_,node)=>{const positions=readLocal<Record<string,{x:number;y:number}>>(key+':positions',{});positions[node.id]=node.position;saveLocal(key+':positions',positions)}} onMoveEnd={(_,v)=>{setViewport(v);saveLocal(key+':viewport',v)}} defaultViewport={viewport} fitView={!viewport} onNodeClick={(_,node)=>{if(!architecture&&typeof node.data.wave==='string'){const wave=node.data.wave;setExpandedWaves(e=>e.includes(wave)?e.filter(x=>x!==wave):[...e,wave]);return}onSelect(node.id);if(architecture&&['domain','module'].includes(data.records.snapshot.ir.symbols[node.id]?.kind))setExpanded(e=>e.includes(node.id)?e.filter(x=>x!==node.id):[...e,node.id])}} onEdgeClick={(_,edge)=>setEdgeId(edge.id)} nodesConnectable={false} edgesReconnectable={false} deleteKeyCode={null} minZoom={0.05} maxZoom={2} onlyRenderVisibleElements><Background/><Controls showInteractive={false}/></ReactFlow></div>
 <p className="muted">Select a connection for its relationships. Numbers count bundled relations; dashed links show history. {!architecture&&mode!=='focus'&&' Select a wave to expand its tasks.'} Drag nodes to arrange.</p>
 {edgeDetail&&<section className="panel relationship-detail" aria-label="Connection details"><h3>{edgeDetail.source} → {edgeDetail.target}</h3>{edgeDetail.relations.map((relation,i)=><p key={i}><strong>{relation.kind}</strong> · {relation.from??relation.source} → {relation.to??relation.target}{relation.reason&&<> · {relation.reason}</>}</p>)}</section>}
 <details><summary>Keyboard-accessible relationships</summary>{layout.bundles.map(bundle=><p key={bundle.id}><button aria-pressed={bundle.id===edgeId} onClick={()=>setEdgeId(bundle.id)}>{bundle.source} → {bundle.target} · {bundle.relations.length} {bundle.category} relationships</button></p>)}</details></>;
}
