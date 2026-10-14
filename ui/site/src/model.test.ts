import {test,expect} from 'bun:test';
import {evidenceStatus,ancestorIds,type Run,type SiteSnapshot} from './model';
const site={records:{snapshot:{id:'current'},check:{run_id:'9007199254740993'}}} as SiteSnapshot;
function run(status='PASS'):Run{return {id:'case',run:{result:{status},provenance:{snapshot:'current',check_occurrence:'9007199254740993',incomplete:[]},selected_subjects:['A'],required_subjects:['A'],required_cases:['case'],cases:[{id:'case'}],open_obligations:[]}} as Run}
test('historical PASS and partial provenance cannot color current requirements green',()=>{
 const r=run();expect(evidenceStatus(r,site)).toBe('PASS');r.run.provenance.check_occurrence='9007199254740992';expect(evidenceStatus(r,site)).toBe('NOT APPLICABLE');r.run.provenance.check_occurrence='9007199254740993';r.run.provenance.incomplete=['unavailable'];expect(evidenceStatus(r,site)).toBe('UNKNOWN');r.run.provenance.incomplete=[];r.run.required_subjects.push('B');expect(evidenceStatus(r,site)).toBe('UNKNOWN');expect(evidenceStatus(run('FAIL'),site)).toBe('FAIL');expect(evidenceStatus(run('UNKNOWN'),site)).toBe('UNKNOWN');
});
test('focus uses prerequisite closure and excludes unrelated earlier waves',()=>{
 const edges=[{task_id:'9',prerequisite_id:'4',condition:'delivered',reason:'type'},{task_id:'4',prerequisite_id:'1',condition:'delivered',reason:'type'},{task_id:'8',prerequisite_id:'2',condition:'delivered',reason:'other'}];expect([...ancestorIds('9',edges)].sort()).toEqual(['1','4','9']);
});
test('compressed observations decode losslessly and reject inconsistent inventories',async()=>{
 const {decodeCases}=await import('./model');const r=run('FAIL');const cases=[{id:'case',subject:'A',kind:'contract',result:{status:'FAIL'},bindings:{value:{kind:'integer',value:'9007199254740993'}},elapsed_us:'18446744073709551615'}];
 const compressed=Bun.gzipSync(new TextEncoder().encode(JSON.stringify(cases)));r.run.case_details=btoa(String.fromCharCode(...compressed));r.run.case_details_encoding='gzip-base64-json-decimal-integers-v1';
 expect(await decodeCases(r)).toEqual(cases);r.run.cases[0].id='other';await expect(decodeCases(r)).rejects.toThrow('inventory');r.run.case_details_encoding='unknown';await expect(decodeCases(r)).rejects.toThrow('encoding');
});

test('version-2 packed mandatory inventory is retained without loading cases into summaries',async()=>{
 const {decodeCases}=await import('./model');const r=run();const cases=[{id:'case',subject:'A',kind:'contract',result:{status:'PASS'}}];
 const corpus={cases,required_cases:['case']};r.run.cases=[];r.run.required_cases=[];r.run.case_count='1';r.run.required_case_count='1';r.run.case_inventory_complete=true;
 r.run.case_details_encoding='gzip-base64-json-decimal-integers-v2';r.run.case_details=btoa(String.fromCharCode(...Bun.gzipSync(new TextEncoder().encode(JSON.stringify(corpus)))));
 expect(evidenceStatus(r,site)).toBe('PASS');expect(await decodeCases(r)).toEqual(cases);r.run.case_count='2';await expect(decodeCases(r)).rejects.toThrow('inventory');
});

test('parallel semantic connections retain every original relationship and preserve cycles',async()=>{
 const {bundleRelations}=await import('./relations');
 const relations=[{source:'Catalog',target:'Planning',category:'semantic' as const,kind:'requires',from:'Catalog.Film',to:'Planning.Candidate'},{source:'Catalog',target:'Planning',category:'semantic' as const,kind:'calls',from:'Catalog.load',to:'Planning.prepare'},{source:'Planning',target:'Catalog',category:'semantic' as const,kind:'calls'}];
 const bundles=bundleRelations(relations);expect(bundles).toHaveLength(2);expect(bundles[0].relations).toEqual(relations.slice(0,2));expect(bundles[1].source).toBe('Planning');
});
test('bundled prerequisites retain conditions and never absorb historical links',async()=>{
 const {bundleRelations}=await import('./relations');
 const bundles=bundleRelations([{source:'9007199254740993',target:'9',category:'prerequisite',kind:'delivered',reason:'Required type'},{source:'9007199254740993',target:'9',category:'prerequisite',kind:'PASS',reason:'Required native check'},{source:'9007199254740993',target:'9',category:'history',kind:'history',reason:'Changed requirements'}]);
 expect(bundles).toHaveLength(2);expect(bundles[0].relations.map(r=>r.reason)).toEqual(['Required type','Required native check']);expect(bundles[1].category).toBe('history');
});

test('published selection inventory defines module scope rather than counting declarations',()=>{
 const r=run();r.run.selected_subjects=['W.Module'];r.run.required_subjects=['W','W.Module','W.Module.operation'];r.run.not_selected=[];
 expect(evidenceStatus(r,site)).toBe('PASS');r.run.not_selected=['W.Other'];expect(evidenceStatus(r,site)).toBe('UNKNOWN');
});
test('reviewed presentation requires exact native provenance, cases and inspection artifacts',async()=>{
 const {reviewedAssessment,evidenceDisplayStatus}=await import('./model');const native=run('UNKNOWN');native.run.origin='native';native.run.case_details='same recorded cases';native.run.open_obligations=[{code:'MANUAL_REQUIREMENT',message:'Review constraint'}];native.run.artifacts=[{hash:'native-artifact',media_type:'native'}];
 const effective=structuredClone(native);effective.id='assessment';effective.run.origin='imported';effective.run.result={status:'PASS'};effective.run.open_obligations=[];effective.run.artifacts.push({hash:'review',media_type:'application/vnd.dctx0.program-inspections+json'});
 const data={...site,records:{...site.records,runs:[native,effective]}};
 expect(evidenceDisplayStatus(native,data)).toBe('REVIEWED');expect(evidenceStatus(native,data)).toBe('UNKNOWN');expect(reviewedAssessment(native,data)?.id).toBe('assessment');
 effective.run.case_details='different cases';expect(evidenceDisplayStatus(native,data)).toBe('UNKNOWN');effective.run.case_details=native.run.case_details;
 effective.run.provenance.implementation={file:'changed'};expect(reviewedAssessment(native,data)).toBeUndefined();effective.run.provenance=native.run.provenance;
 native.run.open_obligations=[{code:'UNSUPPORTED_EXECUTION',message:'Not executed'}];expect(reviewedAssessment(native,data)).toBeUndefined();
});

test('architecture focus follows real parents and limits child depth independently of names',async()=>{
 const {descendantIds}=await import('./model');const symbols=Object.fromEntries([{id:'root',parent:null},{id:'child',parent:'root'},{id:'deep',parent:'child'},{id:'root.fake',parent:null}].map(s=>[s.id,s])) as SiteSnapshot['records']['snapshot']['ir']['symbols'];
 expect([...descendantIds('root',0,symbols)]).toEqual(['root']);expect([...descendantIds('root',1,symbols)]).toEqual(['root','child']);expect([...descendantIds('root',2,symbols)]).toEqual(['root','child','deep']);expect([...descendantIds('missing',Infinity,symbols)]).toEqual([]);
 symbols.root.parent='deep';expect([...descendantIds('root',Infinity,symbols)]).toEqual(['root','child','deep']);
});

test('dependency focus traverses outgoing, incoming and both directions without containment hops',async()=>{
 const {dependencyNeighborhood}=await import('./relations');const symbols=Object.fromEntries([{id:'Module',parent:null},{id:'operation',parent:'Module'},{id:'Used',parent:null},{id:'Caller',parent:null},{id:'Next',parent:null}].map(s=>[s.id,s]));
 const snapshot={ir:{symbols,nodes:[]},graph:{edges:[{from:'operation',to:'Used',kind:{dependency:'calls'}},{from:'Caller',to:'operation',kind:{dependency:'uses-type'}},{from:'Used',to:'Next',kind:{dependency:'reads'}},{from:'Next',to:'Used',kind:{dependency:'calls'}},{from:'Module',to:'Caller',kind:'contains'}]}} as unknown as SiteSnapshot['records']['snapshot'];
 expect([...dependencyNeighborhood('Module',0,'both',snapshot).ids]).toEqual(['Module']);expect([...dependencyNeighborhood('Module',1,'outgoing',snapshot).ids]).toEqual(['Module','Used']);expect([...dependencyNeighborhood('Module',1,'incoming',snapshot).ids]).toEqual(['Module','Caller']);expect([...dependencyNeighborhood('Module',1,'both',snapshot).ids]).toEqual(['Module','Used','Caller']);expect([...dependencyNeighborhood('Module',Infinity,'outgoing',snapshot).ids]).toEqual(['Module','Used','Next']);
 expect(dependencyNeighborhood('Module',1,'outgoing',snapshot).relations[0].from).toBe('operation');expect(dependencyNeighborhood('missing',2,'both',snapshot).ids.size).toBe(0);
});

test('implementation architecture excludes test nodes and their edges before dependency traversal',async()=>{
 const {architectureSnapshot}=await import('./architecture');const {dependencyNeighborhood}=await import('./relations');
 const symbols=Object.fromEntries([{id:'M',kind:'module',parent:null},{id:'M.api',kind:'surface',parent:'M'},{id:'M.op',kind:'operation',parent:'M.api'},{id:'M.test',kind:'test',parent:'M'},{id:'M.review',kind:'review',parent:'M'},{id:'Type',kind:'type',parent:null}].map(s=>[s.id,s]));
 const snapshot={ir:{symbols,nodes:[{id:'M.test#assert',parent:'M.test'}]},graph:{nodes:symbols,edges:[{from:'M.op',to:'Type',kind:{dependency:'uses-type'}},{from:'M.test#assert',to:'M.op',kind:{dependency:'calls'}},{from:'M.review',to:'M.api',kind:'governs'}]}} as unknown as SiteSnapshot['records']['snapshot'];
 const filtered=architectureSnapshot(snapshot,'implementation');expect(Object.keys(filtered.ir.symbols)).toEqual(['M','M.api','M.op','Type']);expect(filtered.ir.nodes).toEqual([]);expect(filtered.graph.edges).toHaveLength(1);expect([...dependencyNeighborhood('M.op',1,'incoming',filtered).ids]).toEqual(['M.op']);expect(architectureSnapshot(snapshot,'all')).toBe(snapshot);expect(snapshot.graph.edges).toHaveLength(3);
});

test('surface dependency focus retains its operations and operation focus shows the declaring surface',async()=>{
 const {focusedDependencies}=await import('./architecture');
 const symbols=Object.fromEntries([{id:'Surface',kind:'surface',parent:null},{id:'Surface.getFilmRating',kind:'operation',parent:'Surface'},{id:'RatingStore',kind:'port',parent:null},{id:'RatingStore.summary',kind:'operation',parent:'RatingStore'}].map(s=>[s.id,s]));
 const snapshot={ir:{symbols,nodes:[]},graph:{nodes:symbols,edges:[{from:'Surface',to:'Surface.getFilmRating',kind:'contains'},{from:'RatingStore',to:'RatingStore.summary',kind:'contains'},{from:'Surface.getFilmRating',to:'RatingStore.summary',kind:{dependency:'calls'}}]}} as unknown as SiteSnapshot['records']['snapshot'];
 const surface=focusedDependencies(snapshot,'Surface',1,'outgoing');expect(surface.ids.has('Surface.getFilmRating')).toBe(true);expect(surface.relations.some(r=>r.source==='Surface.getFilmRating'&&r.target==='RatingStore.summary')).toBe(true);expect(surface.relations.some(r=>r.source==='Surface'&&r.target==='Surface.getFilmRating'&&r.kind==='contains')).toBe(true);
 const operation=focusedDependencies(snapshot,'Surface.getFilmRating',1,'both');expect(operation.ids.has('Surface')).toBe(true);expect(operation.ids.has('RatingStore')).toBe(true);expect([...focusedDependencies(snapshot,'Surface.getFilmRating',0,'outgoing').ids]).toEqual(['Surface.getFilmRating']);
});
