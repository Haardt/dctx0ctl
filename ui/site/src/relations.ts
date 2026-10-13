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
