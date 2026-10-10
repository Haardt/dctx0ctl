import { compileDctx0 } from '../../runatron/packages/plugin-dctx0/src/dctx0/language/compiler';
import {readdirSync,readFileSync,writeFileSync} from 'node:fs';
const root=new URL('..',import.meta.url).pathname;
function ty(t:any):any{return {name:t.name,symbol_id:t.symbolId??null,arguments:t.arguments.map(ty)}}
for(const [name,directory]of [['cinema','examples/cinema/dctx0'],['runatron-next','examples/runatron-next/dctx0']]){
 const c=compileDctx0(readdirSync(root+directory).filter(f=>f.endsWith('.dctx0')).sort().map(file=>({file,source:readFileSync(`${root}${directory}/${file}`,'utf8')})));
 if(c.diagnostics.length||!c.ir)throw Error(JSON.stringify(c.diagnostics));
 const data=Object.fromEntries(c.ir.domain.declarations.map(d=>[d.id,{id:d.id,stable_id:d.stableId??null,alias:d.alias?ty(d.alias):null,fields:d.fields.map(f=>({name:f.name,ty:ty(f.type)})),variants:d.variants.map(v=>({name:v.name,payload:v.payload?ty(v.payload):null})),keys:d.keyFields}]));
 const operations=Object.fromEntries(c.ir.control.operations.map(o=>[o.id,{id:o.id,parameters:o.parameters.map(p=>({name:p.name,ty:ty(p.type)})),result:ty(o.result),failures:o.failures.map(ty),pure:o.purity==='pure'}]));
 writeFileSync(`${root}fixtures/semantic/${name}.json`,JSON.stringify({workspace:c.ir.workspace,symbol_count:c.ir.symbols.length,data,operations})+'\n');
}
