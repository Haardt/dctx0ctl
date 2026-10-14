import {useEffect,useMemo,useState,type ReactNode} from 'react';
import {ReactFlow,Background,Controls,Position,applyNodeChanges,type Node,type Edge,type Viewport} from '@xyflow/react';
import dagre from '@dagrejs/dagre';
import {Button} from '@heroui/react';
import {ancestorIds,descendantIds,type SiteSnapshot} from './model';
import {bundleRelations,type FocusDirection,type Relation} from './relations';
import NodePicker from './NodePicker';
import Inspector from './Inspector';
import {architectureSnapshot,focusedDependencies,type ArchitectureFilter} from './architecture';
export function readLocal<T>(key:string,fallback:T):T {try{return JSON.parse(localStorage.getItem(key)??'null')??fallback}catch{return fallback}}
export function saveLocal(key:string,value:unknown){try{localStorage.setItem(key,JSON.stringify(value))}catch{/* Private/file storage may be unavailable. */}}
export default function Graph({data,architecture,selected,onSelect,dark,focus='',childDepth='1',direction='children',nodeFilter='implementation',onNodeFilter,onFocus,renderNodeDetail}:{data:SiteSnapshot;architecture:boolean;selected:string;onSelect:(id:string)=>void;dark:boolean;focus?:string;childDepth?:string;direction?:FocusDirection;nodeFilter?:ArchitectureFilter;renderNodeDetail?:(id:string,snapshot:SiteSnapshot['records']['snapshot'])=>ReactNode;onNodeFilter?:(filter:ArchitectureFilter)=>void;onFocus?:(focus:string,depth:string,direction:FocusDirection)=>void}){
 const [mode,setMode]=useState(selected&&!architecture?'focus':'all'),[history,setHistory]=useState(false),[expanded,setExpanded]=useState<string[]>([]),[expandedWaves,setExpandedWaves]=useState<string[]>([]);
 const architectureData=useMemo(()=>architectureSnapshot(data.records.snapshot,nodeFilter),[data.records.snapshot,nodeFilter]);
 const scope=architecture?`${nodeFilter}:${focus?`${focus}:${childDepth}:${direction}`:'all'}`:'all';
 const key=`site:${data.project}:${architecture?'architecture':'tasks'}:horizontal-v2${architecture?`:scope:${scope}`:''}`;
 const viewport=readLocal<Viewport|undefined>(key+':viewport',undefined);
 const layout=useMemo(()=>{
  let nodes:Node[]=[],relations:Relation[]=[];
  if(architecture){
   const symbols=Object.values(architectureData.ir.symbols);
   const groups=symbols.filter(s=>['workspace','domain','module'].includes(s.kind));
   const neighborhood=focus&&direction!=='children'?focusedDependencies(architectureData,focus,childDepth==='all'?Infinity:Number(childDepth),direction):null;
   const visible=neighborhood?neighborhood.ids:focus?descendantIds(focus,childDepth==='all'?Infinity:Number(childDepth),architectureData.ir.symbols):new Set(groups.map(s=>s.id));
   if(!focus)for(const s of symbols)if(expanded.some(g=>s.id.startsWith(g+'.'))||s.id===selected)visible.add(s.id);
   const allowed=focus?descendantIds(focus,Infinity,architectureData.ir.symbols):null;
   const parents=new Map([...symbols.map(s=>[s.id,s.parent] as const),...architectureData.ir.nodes.map(n=>[n.id,n.parent] as const)]);
   function owner(id:string){if(allowed&&!allowed.has(id)&&!architectureData.ir.nodes.some(n=>n.id===id&&n.parent&&allowed.has(n.parent)))return '';let current:string|null=id;const seen=new Set<string>();while(current&&!visible.has(current)){if(seen.has(current))return '';seen.add(current);current=parents.get(current)??null}return current??''}
   nodes=symbols.filter(s=>visible.has(s.id)).map(s=>({id:s.id,data:{label:`${s.name} · ${s.kind}${!focus&&['domain','module'].includes(s.kind)?' (expand)':''}`},position:{x:0,y:0},className:s.id===selected?'selected':''}));
   if(neighborhood)relations=neighborhood.relations;else for(const e of architectureData.graph.edges){const from=owner(e.from),to=owner(e.to);if(from&&to&&from!==to){const label=typeof e.kind==='string'?e.kind:Object.values(e.kind??{}).join(' · ');relations.push({source:from,target:to,kind:label,category:'semantic',from:e.from,to:e.to})}}
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
 },[data,architecture,mode,selected,history,expanded,expandedWaves,key,focus,childDepth,direction,architectureData]);
 const [nodes,setNodes]=useState(layout.nodes),[edgeId,setEdgeId]=useState(''),[inspectorOpen,setInspectorOpen]=useState(!!(selected||focus)),[showConnections,setShowConnections]=useState(false);
 useEffect(()=>{if(architecture&&(selected||focus)){setEdgeId('');setInspectorOpen(true)}},[architecture,selected,focus]);
 useEffect(()=>setNodes(layout.nodes),[layout]);
 const edgeDetail=layout.bundles.find(bundle=>bundle.id===edgeId);
 const connectionDetail=edgeDetail&&<section className="relationship-detail" aria-label="Connection details"><h3>{edgeDetail.source} → {edgeDetail.target}</h3>{edgeDetail.relations.map((relation,i)=><p key={i}><strong>{relation.kind}</strong> · {relation.from??relation.source} → {relation.to??relation.target}{relation.reason&&<> · {relation.reason}</>}</p>)}</section>;
 const connections=<div className="connection-list" aria-label="Connections">{layout.bundles.map(bundle=><button className="row" key={bundle.id} aria-pressed={bundle.id===edgeId} onClick={()=>{setEdgeId(bundle.id);setShowConnections(false);setInspectorOpen(true)}}>{bundle.source} → {bundle.target}<small>{bundle.relations.length} {bundle.category} relationships</small></button>)}</div>;
 return <><div className="filters graph-filters">{!architecture&&<><label>Graph scope <select value={mode} onChange={e=>setMode(e.target.value)}><option value="all">Whole plan</option><option value="active">Active work</option><option value="focus">Selected task prerequisites</option></select></label><Button variant="secondary" onPress={()=>setExpandedWaves([])}>Collapse waves</Button><label><input type="checkbox" checked={history} onChange={e=>setHistory(e.target.checked)}/> Historical links</label></>}{architecture&&<><label>Nodes <select aria-label="Architecture node filter" value={nodeFilter} onChange={e=>onNodeFilter?.(e.target.value as ArchitectureFilter)}><option value="implementation">Implementation</option><option value="all">All spec nodes</option></select></label><NodePicker symbols={Object.values(architectureData.ir.symbols)} value={focus} onChange={value=>onFocus?.(value,childDepth,direction)}/><button className="tag graph-action" disabled={!architectureData.ir.symbols[selected]||selected===focus} onClick={()=>onFocus?.(selected,childDepth,direction)}>Focus selected</button><label>Direction <select aria-label="Focus direction" value={direction} onChange={e=>onFocus?.(focus,childDepth,e.target.value as FocusDirection)}><option value="children">Children</option><option value="outgoing">Uses / calls</option><option value="incoming">Used / called by</option><option value="both">Both directions</option></select></label><label className="depth-slider">Depth <input type="range" aria-label="Focus depth" min="0" max="6" step="1" value={childDepth==='all'?6:Number(childDepth)} disabled={!focus} aria-valuetext={childDepth==='all'?'All levels':`${childDepth} levels`} onChange={e=>onFocus?.(focus,e.target.value==='6'?'all':e.target.value,direction)}/><output>{childDepth==='all'?'All':childDepth}</output></label><button className="tag graph-action" disabled={!focus} onClick={()=>onFocus?.('',childDepth,direction)}>Show all</button>{!focus&&<Button variant="secondary" onPress={()=>setExpanded([])}>Collapse groups</Button>}</>}{architecture&&<button className="tag graph-action" onClick={()=>setInspectorOpen(true)}>Inspector</button>}<small>{nodes.length} nodes · {layout.relationCount} relationships{layout.edges.length!==layout.relationCount?` · ${layout.edges.length} connections`:""}</small></div>
 {architecture&&focus&&<p className="muted">Focus: {focus} · {direction==='children'?'Children':direction==='outgoing'?'Uses / calls':direction==='incoming'?'Used / called by':'Both directions'} · {childDepth==='all'?'all levels':`${childDepth} levels`}. {direction==='children'?'Only relationships within this subtree are shown.':'Depth counts dependency hops; node containment does not add hops.'}</p>}
 <div className="graph-stage"><div className="graph"><ReactFlow key={scope} colorMode={dark?'dark':'light'} nodes={nodes} edges={layout.edges.map(edge=>({...edge,selected:edge.id===edgeId}))} onNodesChange={changes=>setNodes(n=>applyNodeChanges(changes,n))} onNodeDragStop={(_,node)=>{const positions=readLocal<Record<string,{x:number;y:number}>>(key+':positions',{});positions[node.id]=node.position;saveLocal(key+':positions',positions)}} onMoveEnd={(_,v)=>{saveLocal(key+':viewport',v)}} defaultViewport={viewport} fitView={!viewport} onNodeClick={(_,node)=>{if(!architecture&&typeof node.data.wave==='string'){const wave=node.data.wave;setExpandedWaves(e=>e.includes(wave)?e.filter(x=>x!==wave):[...e,wave]);return}setEdgeId('');setInspectorOpen(true);onSelect(node.id);if(architecture&&!focus&&['domain','module'].includes(architectureData.ir.symbols[node.id]?.kind))setExpanded(e=>e.includes(node.id)?e.filter(x=>x!==node.id):[...e,node.id])}} onEdgeClick={(_,edge)=>{setEdgeId(edge.id);setShowConnections(false);setInspectorOpen(true)}} nodesConnectable={false} edgesReconnectable={false} deleteKeyCode={null} minZoom={0.05} maxZoom={2} onlyRenderVisibleElements><Background/><Controls showInteractive={false}/></ReactFlow></div>
 {architecture&&inspectorOpen&&<Inspector storageKey={`site:${data.project}:inspector-position`} onClose={()=>setInspectorOpen(false)}><div className="inspector-actions"><button className="tag" aria-pressed={!showConnections} onClick={()=>setShowConnections(false)}>Selection</button><button className="tag" aria-pressed={showConnections} onClick={()=>setShowConnections(true)}>Connections</button></div>{showConnections?connections:connectionDetail??renderNodeDetail?.(selected||focus,architectureData)}</Inspector>}</div>
 <p className="muted">Select a connection for its relationships. Numbers count bundled relations; dashed links show history. {!architecture&&mode!=='focus'&&' Select a wave to expand its tasks.'} Drag nodes to arrange.</p>
 {!architecture&&connectionDetail}
 {!architecture&&<details><summary>Connections</summary>{connections}</details>}</>;
}
