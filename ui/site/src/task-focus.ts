import type {SiteSnapshot} from './model';
export type TaskDirection='prerequisites'|'dependents'|'both';
export function taskNeighborhood(root:string,depth:number,direction:TaskDirection,edges:SiteSnapshot['dependencies']):Set<string> {
 const ids=new Set<string>();if(!root)return ids;ids.add(root);const adjacent=new Map<string,string[]>();
 function add(from:string,to:string){const values=adjacent.get(from)??[];values.push(to);adjacent.set(from,values)}
 for(const edge of edges){if(direction!=='dependents')add(edge.task_id,edge.prerequisite_id);if(direction!=='prerequisites')add(edge.prerequisite_id,edge.task_id)}
 let frontier=[root];for(let level=0;level<depth&&frontier.length;level++){const next:string[]=[];for(const id of frontier)for(const other of adjacent.get(id)??[])if(!ids.has(other)){ids.add(other);next.push(other)}frontier=next}return ids;
}
