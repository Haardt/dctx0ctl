import {readdir,readFile,writeFile,mkdir,realpath} from 'node:fs/promises';
import {join,relative} from 'node:path';
import {createHash} from 'node:crypto';
const root=import.meta.dir;
process.chdir(root);
const check=process.argv.includes('--check');
const digest=(s:string|Uint8Array)=>createHash('sha256').update(s).digest('hex');
const result=await Bun.build({entrypoints:['src/main.tsx'],target:'browser',format:'iife',minify:true,define:{'process.env.NODE_ENV':'"production"','import.meta.env':'{"MODE":"production"}'}});
if(!result.success)throw new Error(result.logs.join('\n'));
let js=await result.outputs[0].text();
// HTML raw-text elements need protection even for bundled string literals.
js=js.replace(/<\/script/gi,'<\\/script');
const cssProcess=Bun.spawn(['bun','node_modules/@tailwindcss/cli/dist/index.mjs','-i','src/styles.css','--minify'],{stdout:'pipe',stderr:'inherit'});
let css=await new Response(cssProcess.stdout).text();
if(await cssProcess.exited)throw new Error('CSS build failed');
css=css.replace(/<\/style/gi,'<\\/style');
const packages=new Map<string,{name:string;version:string;license:string;text:string}>();
const visited=new Set<string>();
async function inventory(dir:string){
 let canonical;try{canonical=await realpath(dir)}catch{return}if(visited.has(canonical))return;visited.add(canonical);
 let entries;try{entries=await readdir(dir,{withFileTypes:true})}catch{return}
 for(const entry of entries){if(entry.name.startsWith('.')&&entry.name!=='.bun')continue;const path=join(dir,entry.name);
 if(entry.name.startsWith('@')||entry.name==='.bun'){await inventory(path);continue}
 let pkg;try{pkg=JSON.parse(await readFile(join(path,'package.json'),'utf8'))}catch{if(entry.isDirectory())await inventory(path);continue}
 const texts=[];for(const file of (await readdir(path)).sort())if(/^(license|copying|notice)(\.|$)/i.test(file)) {try{texts.push(file+'\n'+await readFile(join(path,file),'utf8'))}catch{}}
 const license=typeof pkg.license==='string'?pkg.license:JSON.stringify(pkg.license??pkg.licenses??'MISSING');
 if(license==='MISSING')throw new Error('Missing license: '+pkg.name);
 packages.set(pkg.name+'@'+pkg.version,{name:pkg.name,version:pkg.version,license,text:texts.join('\n')});
 await inventory(join(path,'node_modules'));
 }
}
await inventory(join(root,'node_modules'));
const licenses=[...packages.values()].sort((a,b)=>(a.name+'@'+a.version).localeCompare(b.name+'@'+b.version));
let inputs='';for(const path of ['package.json','bun.lock','tsconfig.json','build.ts',...(await readdir('src')).sort().map(x=>'src/'+x)])inputs+=path+'\0'+await readFile(path,'utf8')+'\0';
const files:Record<string,string>={'site.js':js,'site.css':css,'LICENSES.txt':'Installed UI dependencies (runtime and build/test tools)\n'+licenses.map(p=>`\n${p.name}@${p.version} — ${p.license}\n${p.text}`).join('\n'),'provenance.json':JSON.stringify({schema_version:1,input_sha256:digest(inputs),bun:Bun.version,assets:{'site.js':digest(js),'site.css':digest(css)},packages:licenses.map(({text,...p})=>p)},null,2)+'\n'};
await mkdir('dist',{recursive:true});
for(const [name,content]of Object.entries(files)){if(check){if(await readFile('dist/'+name,'utf8')!==content)throw new Error('Stale asset: '+name)}else await writeFile('dist/'+name,content)}
console.log(JSON.stringify({assets:Object.fromEntries(Object.entries(files).map(([k,v])=>[k,new TextEncoder().encode(v).length])),packages:packages.size,check}));
