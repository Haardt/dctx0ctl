/** Additional actual persistence/HTTP checks. They do not fabricate central
 * case observations or satisfy a manifest duty by themselves.
 */
import {expect} from 'bun:test';
import {cinemaFixture} from './cinema-fixture.ts';
import {BindingError,projectValue,type CaseContext,type Invocation,type Value} from './library/index.ts';
import type {OperationModel} from './operation-fixtures.ts';
const root='HomeCinema.MovieNight.';
export async function verifyNativeBoundaries(model:OperationModel):Promise<void>{
 const request={id:'00000000-0000-4000-8000-000000000099',title:'Native persistence boundary',releaseYear:2026,genres:['Drama'],runtimeMinutes:90,availability:'Available'};
 function invocation(subject:string,value:unknown):Invocation{const operation=model.operations[root+subject];if(!operation)throw new BindingError('Missing native boundary operation');return {subject:root+subject,arguments:[projectValue(value,operation.parameters[0]!.ty,model.data)]};}
 function context(id:string):CaseContext{return {id,context:0,initialSubject:null,mocks:[],async collaborate(){throw new BindingError('Independent real boundary check must not use central mocks');}};}
 const persisted=await cinemaFixture(model,context('technical:persistence-reopen'));
 try{
  expect((await persisted.readState!([root+'Film'],'before',invocation('Catalog.FilmCatalogStore.save',request))).entities[root+'Film']).toEqual([]);
  const saved=await persisted.invokeProduction(invocation('Catalog.FilmCatalogStore.save',request));
  expect(saved.observation.kind).toBe('returned');
  expect(saved.resources?.some(resource=>resource.access==='write'&&resource.resource===root+'Film')).toBe(true);
  const before=(await persisted.readState!([root+'Film'],'after',invocation('Catalog.FilmCatalogStore.save',request))).entities;
  await persisted.reopen();
  expect((await persisted.readState!([root+'Film'],'before',invocation('Catalog.FilmCatalogStore.find',request.id))).entities).toEqual(before);
  const found=await persisted.invokeProduction(invocation('Catalog.FilmCatalogStore.find',request.id));
  expect(found.observation).toEqual({kind:'returned',value:{kind:'option',value:saved.observation.kind==='returned'?saved.observation.value:null}});
 }finally{await persisted.cleanup();}
 const http=await cinemaFixture(model,context('technical:http'),{transport:'http'});
 try{
  const saved=await http.invokeProduction(invocation('Catalog.FilmCatalogApi.saveFilm',request));
  expect(saved.observation.kind).toBe('returned');expect(saved.events?.map(event=>event.subject)).toEqual([root+'FilmSaved']);
  expect(saved.calls?.map(call=>call.invocation.subject)).toEqual([root+'Catalog.FilmCatalogStore.save']);
  await http.reopen();
  const found=await http.invokeProduction(invocation('Catalog.FilmCatalogApi.getFilm',request.id));
  expect(found.observation).toEqual(saved.observation);
 }finally{await http.cleanup();}
}
