/** Typed route connections for the actual Hono application, independent of the
 * retained adapter. Wire omission/null represent native Option absence only.
 */
import type {Hono} from 'hono';
import {ApplicationFailure} from '../../src/application/movie-night.ts';
import type {SurfaceBindings} from './generated/facade.ts';
import {BindingError,type TypeRef,type ValueModel} from './library/index.ts';
import type {OperationModel} from './operation-fixtures.ts';
const root='HomeCinema.MovieNight.';
function optionType(ty:TypeRef,model:ValueModel):TypeRef {
 for(let depth=0;depth<128;depth++){
  const alias=ty.symbol_id?model[ty.symbol_id]?.alias:null;if(!alias)return ty;ty=alias;
 }
 throw new BindingError('HTTP type alias depth exceeded');
}
function nativeJson(value:unknown,ty:TypeRef,model:ValueModel,depth=0):unknown{
 if(depth>128)throw new BindingError('HTTP result depth exceeded');
 const ref=optionType(ty,model);
 if(ref.name==='Option'||ref.name==='Dctx0Option')return value===null||value===undefined?undefined:nativeJson(value,ref.arguments[0]!,model,depth+1);
 if(ref.name==='List'&&Array.isArray(value))return value.map(item=>nativeJson(item,ref.arguments[0]!,model,depth+1));
 if(value!==null&&typeof value==='object'&&!Array.isArray(value)){
  const fields=ref.symbol_id?model[ref.symbol_id]?.fields:undefined;
  const result=Object.fromEntries(Object.entries(value).map(([key,item])=>{const field=fields?.find(field=>field.name===key);return [key,field?nativeJson(item,field.ty,model,depth+1):item];}));
  for(const field of fields??[]){const type=optionType(field.ty,model);if(!Object.hasOwn(result,field.name)&&(type.name==='Option'||type.name==='Dctx0Option'))result[field.name]=undefined;}
  return result;
 }
 return value;
}
export function createHttpBindings(http:Pick<Hono,'request'>,model:OperationModel):SurfaceBindings{
 const id=(value:string)=>encodeURIComponent(value);
 async function request<T>(subject:string,path:string,method='GET',body?:unknown):Promise<T>{
  const response=await http.request(path,{method,headers:{'content-type':'application/json'},...(body===undefined?{}:{body:JSON.stringify(body)})});
  let value:unknown;try{value=await response.json();}catch(error){throw new BindingError('Production HTTP response is not JSON: '+String(error));}
  if(response.status===200){const operation=model.operations[root+subject];if(!operation)throw new BindingError('Missing HTTP result model');return nativeJson(value,operation.result,model.data) as T;}
  if(value!==null&&typeof value==='object'&&'failure' in value&&'payload' in value){
   const failure=value as {failure:string;payload:Record<string,unknown>};
   const expected=failure.failure==='PersistenceUnavailable'?503:failure.failure==='VotingClosed'?409:404;
   if(response.status!==expected)throw new BindingError('HTTP failure status differs from declared mapping');
   throw new ApplicationFailure(failure.failure,failure.payload);
  }
  throw new BindingError('Unexpected production HTTP response '+response.status+': '+JSON.stringify(value));
 }
 return {
  searchFilms(filter){const query=new URLSearchParams();for(const [key,value] of Object.entries(filter))if(value!==undefined)query.set(key,String(value));return request('Catalog.FilmCatalogApi.searchFilms','/films?'+query);},
  getFilm(filmId){return request('Catalog.FilmCatalogApi.getFilm','/films/'+id(filmId));},
  saveFilm(value){return request('Catalog.FilmCatalogApi.saveFilm','/films','POST',value);},
  changeFilmAvailability(value){return request('Catalog.FilmCatalogApi.changeFilmAvailability','/films/'+id(value.filmId)+'/availability','PATCH',{availability:value.availability});},
  listComments(filmId){return request('Community.FilmCommunityApi.listComments','/films/'+id(filmId)+'/comments');},
  addComment(value){return request('Community.FilmCommunityApi.addComment','/films/'+id(value.filmId)+'/comments','POST',{authorId:value.authorId,text:value.text});},
  getFilmRating(filmId,memberId){return request('Community.FilmCommunityApi.getFilmRating','/films/'+id(filmId)+'/rating?memberId='+id(memberId));},
  rateFilm(value){return request('Community.FilmCommunityApi.rateFilm','/films/'+id(value.filmId)+'/rating','PUT',{memberId:value.memberId,stars:value.stars});},
  openVoting(value){return request('ScreeningPlanning.ScreeningPlanningApi.openVoting','/voting','POST',value);},
  getCurrentVoting(){return request('ScreeningPlanning.ScreeningPlanningApi.getCurrentVoting','/voting/current');},
  castVote(value){return request('ScreeningPlanning.ScreeningPlanningApi.castVote','/voting/'+id(value.pollId)+'/votes','PUT',{memberId:value.memberId,filmId:value.filmId});},
  closeVoting(pollId){return request('ScreeningPlanning.ScreeningPlanningApi.closeVoting','/voting/'+id(pollId)+'/close','POST');},
 };
}
