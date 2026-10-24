import {strict as assert} from 'node:assert';
import {createHash} from 'node:crypto';
import {resolve} from 'node:path';

const root=resolve(import.meta.dir,'../..');
const binary=process.env.DCTX0_BIN??resolve(root,'target/release/dctx0');
export const categories=['public_statement_keywords','context_generated_ast_kinds','builtin_types','operators','functions','formats','state_modes','expression_kinds','state_predicates'] as const;
type Category=typeof categories[number];
type Seen=Record<Category,Map<string,unknown>>;
export function emptyFeatures():Seen {return Object.fromEntries(categories.map(key=>[key,new Map])) as Seen;}
function walk(value:unknown,visit:(entry:any)=>void):void {
  if(!value||typeof value!=='object')return;
  visit(value);Object.values(value).forEach(entry=>walk(entry,visit));
}
export function collectFeatures(ast:any[],plan:any,source:Record<string,string>):Seen {
  const seen=emptyFeatures();
  const add=(category:Category,name:string,span:any)=>seen[category].set(name,span??null);
  function node(entry:any):void {
    add('public_statement_keywords',entry.kind,entry.span);add('context_generated_ast_kinds',entry.kind,entry.span);
    if(entry.kind==='assertion'&&Buffer.from(source[entry.span.file]).subarray(entry.span.start.offset,entry.span.end.offset).toString().startsWith('assert '))add('public_statement_keywords','assert',entry.span);
    if(entry.kind==='format')add('formats',entry.terms[0].text,entry.span);
    if(entry.kind==='state'&&['observes','lists','inserts','upserts','replaces','removes'].includes(entry.terms[0]?.text))add('state_modes',entry.terms[0].text,entry.span);
    if(entry.kind==='expect') {
      for(const term of entry.terms)if(term.kind==='identifier'&&['fails','emits'].includes(term.text))add('public_statement_keywords',term.text,term.span);
    }
    walk(entry.syntax,type=>{if(typeof type.name==='string'&&Array.isArray(type.arguments)&&type.span)add('builtin_types',type.name,type.span);});
    entry.children.forEach(node);
  }
  ast.forEach(file=>file.statements.forEach(node));
  walk(plan,expression=>{
    if(!expression.ty||!expression.span||!expression.value?.kind)return;
    const value=expression.value;
    add('expression_kinds',value.kind,expression.span);
    if(['binary','unary'].includes(value.kind))add('operators',value.operator,expression.span);
    if(value.kind==='collection')add('functions',value.operator,expression.span);
    if(value.kind==='call'&&!value.target)add('functions',value.name,expression.span);
    if(value.kind==='call'&&value.target&&/^(exists|preserves|creates|updates|removes):/.test(value.name))add('state_predicates',value.name.split(':')[0],expression.span);
  });
  // These keywords must also have executable, typed meanings in the plan.
  const expectations=plan.examples.flatMap((example:any)=>example.expectations);
  if(!expectations.some((expectation:any)=>expectation.kind==='failure'))seen.public_statement_keywords.delete('fails');
  if(!expectations.some((expectation:any)=>expectation.kind==='emitted'))seen.public_statement_keywords.delete('emits');
  return seen;
}
const strings=(text:string)=>[...text.matchAll(/"([^"\n]+)"/g)].map(match=>match[1]!);
const arms=(text:string)=>text.split('\n').filter(line=>line.includes('=>')).flatMap(line=>strings(line.split('=>')[0]!));
export async function languageCatalog():Promise<Record<Category,string[]>> {
  const core=await Bun.file(resolve(root,'crates/dctx0-core/src/lib.rs')).text();
  const semantic=await Bun.file(resolve(root,'crates/dctx0-core/src/semantic.rs')).text();
  const resolution=await Bun.file(resolve(root,'crates/dctx0-language/src/resolution.rs')).text();
  const expressions=await Bun.file(resolve(root,'crates/dctx0-language/src/expression.rs')).text();
  const calls=await Bun.file(resolve(root,'crates/dctx0-language/src/expression/calls.rs')).text();
  const relations=await Bun.file(resolve(root,'crates/dctx0-language/src/compiler/relations.rs')).text();
  const state=await Bun.file(resolve(root,'crates/dctx0-interpreter/src/observed.rs')).text();
  function section(text:string,pattern:RegExp):string {const match=text.match(pattern);assert.ok(match,'Language registry changed: update the audited extractor');return match[1]!;}
  const context=['field','own','dependency','artifact','criterion','narrative','assertion'];
  const keywords=strings(section(core,/statement_kinds! \{([\s\S]*?)\n\}/));
  const kinds=[...section(semantic,/pub enum ExpressionKind \{([\s\S]*?)\n\}/).matchAll(/^    ([A-Z][A-Za-z]+) \{/gm)].map(match=>match[1]!.replace(/[A-Z]/g,(letter,index)=>(index?'-':'')+letter.toLowerCase()));
  return {
    public_statement_keywords:keywords.filter(keyword=>!context.includes(keyword)),context_generated_ast_kinds:context,
    builtin_types:strings(section(resolution,/fn builtin\([^]*?\{([^]*?)\n\}/)),
    operators:[...arms(section(expressions,/fn precedence\([^]*?\{([^]*?)\n\}/)),...strings(section(expressions,/else if matches!\(token.text.as_str\(\), ([^]*?)\) \{\n\s*let operand/))],
    functions:[...strings(section(calls,/if matches!\([^]*?token.text.as_str\(\),([^]*?)\) \{\n\s*return self.collection/)),...arms(section(calls,/let \(arity, result\) = match token.text.as_str\(\) \{([^]*?)\n\s*_ =>/))],
    formats:strings(section(relations,/node.kind == StatementKind::Format[^]*?node.terms\[0\].text.as_str\(\),([^]*?)\)/)),
    state_modes:strings(section(relations,/node.kind == StatementKind::State[^]*?node.terms\[0\].text.as_str\(\),([^]*?)\)/)),
    expression_kinds:kinds,state_predicates:[section(state,/if predicate == "([^"]+)"/),...arms(section(state,/let result = match predicate \{([^]*?)\n\s*_ =>/))],
  };
}
export function assertCatalog(expected:Record<Category,string[]>,actual:Record<Category,string[]>):void {
  for(const category of categories)assert.deepEqual([...new Set(actual[category])].sort(),[...new Set(expected[category])].sort(),'Catalog drift: '+category);
}
async function command(args:string[]):Promise<string>{
  const child=Bun.spawn([binary,...args],{cwd:root,stdout:'pipe',stderr:'pipe'});
  const [output,error,code]=await Promise.all([new Response(child.stdout).text(),new Response(child.stderr).text(),child.exited]);
  assert.equal(code,0,args.join(' ')+'\n'+error);return output;
}
if(import.meta.main){
  const inventory=await Bun.file(resolve(import.meta.dir,'features.json')).json();
  const catalog=await languageCatalog();assertCatalog(catalog,inventory);
  const introductions=emptyFeatures(),steps=[];
  for(const step of inventory.steps){
    const spec='examples/'+step.directory+'/dctx0';
    // Parse each source separately: CLI JSON is formatted across multiple lines.
    const paths=await Array.fromAsync(new Bun.Glob('**/*.dctx0').scan({cwd:resolve(root,spec),onlyFiles:true}));
    const ast=[];
    for(const path of paths.sort())ast.push(JSON.parse(await command(['parse',spec+'/'+path,'--json'])));
    const plan=JSON.parse(await command(['verify','plan',spec,'--project','examples/'+step.directory,'--json']));
    const sources=Object.fromEntries(await Promise.all(ast.map(async file=>[file.file,await Bun.file(resolve(root,file.file)).text()])));
    const seen=collectFeatures(ast,plan,sources);
    for(const category of categories)for(const feature of catalog[category])if(seen[category].has(feature)&&!introductions[category].has(feature))introductions[category].set(feature,{step:step.number,span:seen[category].get(feature)});
    steps.push({number:step.number,snapshot:plan.snapshot,source_hashes:Object.fromEntries(Object.entries(sources).map(([file,text])=>[file,createHash('sha256').update(text as string).digest('hex')]))});
  }
  const missing=categories.flatMap(category=>catalog[category].filter(feature=>!introductions[category].has(feature)).map(feature=>({category,feature})));
  const report={schema_version:1,status:missing.length?'FAIL':'PASS',basis:'Audited language registries, actual parsed statements/type syntax and validated executable expression plans',introductions:Object.fromEntries(categories.map(category=>[category,Object.fromEntries(introductions[category])])),missing,steps};
  await Bun.write(resolve(import.meta.dir,'validation/features.json'),JSON.stringify(report,null,2)+'\n');
  console.log(JSON.stringify({status:report.status,features:categories.reduce((sum,category)=>sum+catalog[category].length,0),missing},null,2));
  assert.equal(missing.length,0,'Tutorial does not demonstrate every registered feature');
}
