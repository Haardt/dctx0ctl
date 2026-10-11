import {expect,test} from 'bun:test';
import {CinemaSession} from '../../../src/app/session.ts';
import {invokeHttp} from './http-invocation.ts';
const id=(n:number)=>'00000000-0000-4000-8000-'+String(n).padStart(12,'0');
const film={id:id(1),title:'Cinema HTTP',releaseYear:2026,genres:['Comedy'],runtimeMinutes:90,availability:'Available'};
test('reviewed HTTP transport exercises all twelve real routes and preserves failures',async()=>{
 const events:string[]=[];
 const session=new CinemaSession(':memory:',event=>events.push(event.name),{now:()=> '2026-10-11T12:00:00Z',id:()=>id(30)});
 try{
  const call=(name:string,...args:unknown[])=>invokeHttp(session.http,name,args);
  expect(await call('getCurrentVoting')).toBeUndefined();
  expect(await call('saveFilm',film)).toMatchObject({film:{id:id(1)}});
  expect(await call('searchFilms',{query:'cinema',sort:'TitleAscending',offset:0,limit:20})).toMatchObject({total:1});
  expect(await call('getFilm',id(1))).toMatchObject({film:{title:'Cinema HTTP'}});
  expect(await call('changeFilmAvailability',{filmId:id(1),availability:'Watched'})).toMatchObject({film:{availability:'Watched'}});
  expect(await call('addComment',{filmId:id(1),authorId:id(10),text:'Great'})).toMatchObject({id:id(30),text:'Great'});
  expect(await call('listComments',id(1))).toMatchObject({total:1});
  expect(await call('rateFilm',{filmId:id(1),memberId:id(10),stars:'FiveStars'})).toMatchObject({ratingCount:1});
  expect(await call('getFilmRating',id(1),id(10))).toMatchObject({memberRating:'FiveStars'});
  expect(await call('openVoting',{pollId:id(20),scheduledFor:'2026-10-11T12:00:00Z',candidateFilmIds:[id(1)]})).toMatchObject({status:'Open'});
  expect(await call('castVote',{pollId:id(20),memberId:id(10),filmId:id(1)})).toMatchObject({winningFilmId:id(1)});
  expect(await call('closeVoting',id(20))).toMatchObject({status:'Closed'});
  await expect(call('castVote',{pollId:id(20),memberId:id(10),filmId:id(1)})).rejects.toMatchObject({code:'VotingClosed'});
  await expect(call('getFilm',id(99))).rejects.toMatchObject({code:'FilmNotFound'});
  expect(events).toContain('VotingCompleted');
  expect(session.scope.resolve('movieNight')).toBe(session.service);
  session.store.db.close();
  await expect(call('searchFilms',{sort:'TitleAscending',offset:0,limit:20})).rejects.toMatchObject({code:'PersistenceUnavailable'});
 }finally{await session.close();}
});
test('HTTP translation reports unavailable routes and invalid status mappings',async()=>{
 const session=new CinemaSession(':memory:');
 try{
  await expect(invokeHttp(session.http,'unknown',[])).rejects.toThrow('No reviewed HTTP mapping');
  await expect(invokeHttp(session.http,'searchFilms',[{sort:'TitleAscending',offset:0,limit:0}])).rejects.toThrow('Unexpected HTTP response 400');
  const bad={request:async()=>new Response(JSON.stringify({failure:'FilmNotFound',payload:{filmId:id(1)}}),{status:503})};
  await expect(invokeHttp(bad as never,'getFilm',[id(1)])).rejects.toThrow('HTTP failure status mismatch');
 }finally{await session.close();}
});
