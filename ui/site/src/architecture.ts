import type {Snapshot} from './model';
export type ArchitectureFilter='implementation'|'all';
const kinds=new Set(['workspace','domain','module','surface','port','operation','entity','type','value','message','event','failure','external']);
export function architectureSnapshot(snapshot:Snapshot,filter:ArchitectureFilter):Snapshot {
 if(filter==='all')return snapshot;
 const symbols=Object.fromEntries(Object.entries(snapshot.ir.symbols).filter(([,s])=>kinds.has(s.kind)));
 const parents=new Map([...Object.values(snapshot.ir.symbols).map(s=>[s.id,s.parent] as const),...snapshot.ir.nodes.map(n=>[n.id,n.parent] as const)]);
 function included(id:string){const visited=new Set<string>();let current:string|null=id;while(current&&!visited.has(current)){visited.add(current);if(snapshot.ir.symbols[current])return !!symbols[current];current=parents.get(current)??null}return false}
 return {...snapshot,ir:{...snapshot.ir,symbols,nodes:snapshot.ir.nodes.filter(n=>included(n.id))},graph:{...snapshot.graph,nodes:Object.fromEntries(Object.entries(snapshot.graph.nodes).filter(([id])=>included(id))),edges:snapshot.graph.edges.filter(e=>e.kind!=='governs'&&included(e.from)&&included(e.to))}};
}

import {descendantIds} from './model';
import {dependencyNeighborhood,type FocusDirection,type Relation} from './relations';
export function focusedDependencies(snapshot:Snapshot,root:string,depth:number,direction:Exclude<FocusDirection,'children'>) {
 const neighborhood=dependencyNeighborhood(root,depth,direction,snapshot),ids=new Set(neighborhood.ids);
 if(depth>0)for(const id of descendantIds(root,depth,snapshot.ir.symbols))ids.add(id);
 const parents=new Map([...Object.values(snapshot.ir.symbols).map(s=>[s.id,s.parent] as const),...snapshot.ir.nodes.map(n=>[n.id,n.parent] as const)]);
 if(depth>0)for(const id of [...ids]){const symbol=snapshot.ir.symbols[id],parent=symbol?.parent? snapshot.ir.symbols[symbol.parent]:undefined;if(symbol?.kind==='operation'&&parent&&['surface','port'].includes(parent.kind))ids.add(parent.id)}
 function owner(id:string,fallback:string){const seen=new Set<string>();let current:string|null=id;while(current&&!seen.has(current)){if(ids.has(current))return current;seen.add(current);current=parents.get(current)??null}return fallback}
 const relations:Relation[]=neighborhood.relations.map(r=>({...r,source:owner(r.from??r.source,r.source),target:owner(r.to??r.target,r.target)}));
 for(const edge of snapshot.graph.edges)if(edge.kind==='contains'&&ids.has(edge.from)&&ids.has(edge.to))relations.push({source:edge.from,target:edge.to,from:edge.from,to:edge.to,kind:'contains',category:'semantic'});
 return {ids,relations};
}
