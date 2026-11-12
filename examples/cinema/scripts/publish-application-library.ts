/** Reproducible local testing package. Production never imports this package. */
import {mkdir,readFile,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
const source=new URL('../../../targets/typescript/applicationtests/',import.meta.url);
const destination=new URL('../.dctx0/application-tests/library/',import.meta.url);
const files=['package.json','protocol.ts','client.ts','testing.ts','values.ts','operation.ts','context.ts','catalog.ts','index.ts','README.md'];
await mkdir(destination,{recursive:true});
const hashes:Record<string,string>={};
for(const name of files){
 const bytes=await readFile(new URL(name,source));
 hashes[name]=createHash('sha256').update(bytes).digest('hex');
 await writeFile(new URL(name,destination),bytes);
}
await writeFile(new URL('library-pin.json',new URL('../.dctx0/application-tests/',import.meta.url)),JSON.stringify({schema_version:1,name:'@dctx0/application-tests',version:'0.2.0',application_protocol:2,source:'targets/typescript/applicationtests',files:hashes},null,2)+'\n');
