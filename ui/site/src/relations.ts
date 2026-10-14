export interface Relation {
 source:string;
 target:string;
 kind:string;
 category:'semantic'|'prerequisite'|'history';
 reason?:string;
 from?:string;
 to?:string;
}
export interface RelationBundle {
 id:string;
 source:string;
 target:string;
 category:Relation['category'];
 relations:Relation[];
}
// Preserve direction, individual requirements and the history/scheduling boundary.
export function bundleRelations(relations:Relation[]):RelationBundle[] {
 const bundles=new Map<string,RelationBundle>();
 for(const relation of relations){
  const id=JSON.stringify([relation.category,relation.source,relation.target]);
  const bundle=bundles.get(id)??{id,source:relation.source,target:relation.target,category:relation.category,relations:[]};
  bundle.relations.push(relation);bundles.set(id,bundle);
 }
 return [...bundles.values()];
}

import {descendantIds,type Snapshot} from './model';
export type FocusDirection='children'|'outgoing'|'incoming'|'both';
export function dependencyNeighborhood(root:string,depth:number,direction:Exclude<FocusDirection,'children'>,snapshot:Snapshot) {
 const ids=new Set<string>();if(!snapshot.ir.symbols[root])return {ids,relations:[] as Relation[]};ids.add(root);
 const subtree=descendantIds(root,Infinity,snapshot.ir.symbols),parents=new Map([...Object.values(snapshot.ir.symbols).map(s=>[s.id,s.parent] as const),...snapshot.ir.nodes.map(n=>[n.id,n.parent] as const)]);
 function owner(id:string){const seen=new Set<string>();let current:string|null=id;while(current){if(subtree.has(current))return root;if(snapshot.ir.symbols[current])return current;if(seen.has(current))break;seen.add(current);current=parents.get(current)??null}return ''}
 const all:Relation[]=[];
 for(const edge of snapshot.graph.edges){if(!edge.kind||typeof edge.kind!=='object'||Array.isArray(edge.kind)||!('dependency' in edge.kind))continue;const source=owner(edge.from),target=owner(edge.to);if(source&&target&&source!==target)all.push({source,target,from:edge.from,to:edge.to,kind:String(edge.kind.dependency),category:'semantic'})}
 const adjacent=new Map<string,string[]>();function add(a:string,b:string){const values=adjacent.get(a)??[];values.push(b);adjacent.set(a,values)}
 for(const edge of all){if(direction!=='incoming')add(edge.source,edge.target);if(direction!=='outgoing')add(edge.target,edge.source)}
 let frontier=[root];for(let level=0;level<depth&&frontier.length;level++){const next:string[]=[];for(const id of frontier)for(const other of adjacent.get(id)??[])if(!ids.has(other)){ids.add(other);next.push(other)}frontier=next}
 return {ids,relations:all.filter(edge=>ids.has(edge.source)&&ids.has(edge.target))};
}
