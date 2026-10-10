import {lexDctx0} from '../../runatron/packages/plugin-dctx0/src/dctx0/language/lexer.ts';
import {parseDctx0} from '../../runatron/packages/plugin-dctx0/src/dctx0/language/parser.ts';
import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
const root = new URL('..', import.meta.url).pathname;
function normalize(s:any):any {return {kind:s.kind,name:s.name??null,terms:s.terms,children:s.children.map(normalize)}}
for(const [id,path] of [['cinema','examples/cinema/dctx0'],['runatron-next','examples/runatron-next/dctx0']]){
 for(const name of readdirSync(`${root}/${path}`).filter(s=>s.endsWith('.dctx0')).sort()){
 const file=`${path}/${name}`, source=readFileSync(`${root}/${file}`,'utf8');
 const l=lexDctx0(file,source),p=parseDctx0(file,l.tokens);
 if(l.diagnostics.length||p.diagnostics.length)throw Error(JSON.stringify([...l.diagnostics,...p.diagnostics]));
 writeFileSync(`${root}/fixtures/parser/${id}-${name}.json`,JSON.stringify(p.ast.statements.map(normalize))+'\n');
 }
}
