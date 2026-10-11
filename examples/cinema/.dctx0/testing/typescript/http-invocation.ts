/** Explicit testing access to application-owned HTTP routes. No result logic. */
import {ApplicationFailure} from '../../../src/application/movie-night.ts';
import type {Hono} from 'hono';

export async function invokeHttp(app:Pick<Hono,'request'>, operation:string, args:unknown[]):Promise<unknown> {
 const first=args[0] as Record<string,unknown>;
 let path:string,method='GET',body:unknown;
 const id=(value:unknown)=>encodeURIComponent(String(value));
 switch(operation){
  case 'searchFilms':{
   const query=new URLSearchParams();
   for(const [name,value] of Object.entries(first))if(value!==undefined)query.set(name,String(value));
   path='/films?'+query;break;
  }
  case 'getFilm':path='/films/'+id(args[0]);break;
  case 'saveFilm':path='/films';method='POST';body=first;break;
  case 'changeFilmAvailability':path='/films/'+id(first.filmId)+'/availability';method='PATCH';body={availability:first.availability};break;
  case 'listComments':path='/films/'+id(args[0])+'/comments';break;
  case 'addComment':path='/films/'+id(first.filmId)+'/comments';method='POST';body={authorId:first.authorId,text:first.text};break;
  case 'getFilmRating':path='/films/'+id(args[0])+'/rating?memberId='+id(args[1]);break;
  case 'rateFilm':path='/films/'+id(first.filmId)+'/rating';method='PUT';body={memberId:first.memberId,stars:first.stars};break;
  case 'openVoting':path='/voting';method='POST';body=first;break;
  case 'getCurrentVoting':path='/voting/current';break;
  case 'castVote':path='/voting/'+id(first.pollId)+'/votes';method='PUT';body={memberId:first.memberId,filmId:first.filmId};break;
  case 'closeVoting':path='/voting/'+id(args[0])+'/close';method='POST';break;
  default:throw new Error('No reviewed HTTP mapping for '+operation);
 }
 const response=await app.request(path,{method,headers:{'content-type':'application/json'},...(body===undefined?{}:{body:JSON.stringify(body)})});
 let value:unknown;
 try{value=await response.json();}catch{throw new Error('HTTP response '+response.status+' has a non-JSON body');}
 if(response.status===200)return value===null?undefined:value;
 if(value&&typeof value==='object'&&'failure' in value&&'payload' in value){
  const failure=value as {failure:string;payload:Record<string,unknown>};
  const expected=failure.failure==='PersistenceUnavailable'?503:failure.failure==='VotingClosed'?409:404;
  if(response.status!==expected)throw new Error('HTTP failure status mismatch: '+response.status);
  throw new ApplicationFailure(failure.failure,failure.payload);
 }
 throw new Error('Unexpected HTTP response '+response.status+': '+JSON.stringify(value));
}
