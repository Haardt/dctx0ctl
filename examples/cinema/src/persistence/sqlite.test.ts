import {afterAll,expect,test} from 'bun:test';
import {Database} from 'bun:sqlite';
import {CinemaStore} from './sqlite.ts';
import {CinemaSession} from '../app/session.ts';
import {ApplicationFailure,type ApplicationEvent,type FilmSearch,type SaveFilm} from '../application/movie-night.ts';
import {readFileSync,writeFileSync} from 'node:fs';
const id=(n:number)=>`00000000-0000-4000-8000-${String(n).padStart(12,'0')}`;
const now='2026-10-10T18:00:00Z';
const film=(n:number,title='Movie Night'):SaveFilm=>({id:id(n),title,releaseYear:2020,genres:['Drama'],runtimeMinutes:120,availability:'Available'});
const filter:FilmSearch={query:undefined,genre:undefined,minimumStars:undefined,availability:undefined,maximumRuntimeMinutes:undefined,releasedFrom:undefined,sort:'TitleAscending',offset:0,limit:20};
const samples:Record<string,{elapsed_ms:number}>={};
async function measured(name:string,body:()=>Promise<void>){const start=performance.now();await body();samples[name]={elapsed_ms:performance.now()-start};}
afterAll(()=>{if(process.env.DCTX0_CINEMA_NATIVE_REPORT){const status=readFileSync('/proc/self/status','utf8');const peak_rss_bytes=Number(status.match(/^VmHWM:\s+(\d+)\s+kB$/m)![1])*1024;writeFileSync(process.env.DCTX0_CINEMA_NATIVE_REPORT,JSON.stringify({scope:'Independent real Cinema SQLite/Hono/Awilix tests; not complete spec-driven verification',samples,peak_rss_bytes,memory_scope:'Linux live Bun VmHWM including suite initialization',runtime_version:Bun.version}));}});
function fixture(){const db=new Database(':memory:');let next=30;const store=new CinemaStore(db,{now:()=>now,id:()=>id(next++)});return {db,store,p:store.ports};}
test('catalog persistence, combined filters, stable ordering and pagination',async()=>measured('catalog',async()=>{
 const {db,store,p}=fixture();try{
  expect(await p.catalog.find(id(1))).toBeUndefined();await p.catalog.save(film(1,'alpha'));await p.catalog.save({...film(2,'Beta'),releaseYear:2021,genres:['Action'],runtimeMinutes:90});
  expect((await p.catalog.search(filter)).items.map(f=>f.title)).toEqual(['Beta','alpha']);
  expect((await p.catalog.search({...filter,query:'ALPHA',genre:'Drama',availability:'Available',releasedFrom:2000,maximumRuntimeMinutes:120})).total).toBe(1);
  expect((await p.catalog.search({...filter,query:'alpha',genre:'Action'})).total).toBe(0);
  expect((await p.catalog.search({...filter,sort:'NewestFirst',offset:1,limit:1})).items[0].id).toBe(id(1));
  expect((await p.catalog.changeAvailability({filmId:id(1),availability:'Watched'})).film.availability).toBe('Watched');
  expect((await p.catalog.search({...filter,availability:'Available'})).total).toBe(1);
  await expect(p.catalog.changeAvailability({filmId:id(99),availability:'Watched'})).rejects.toBeInstanceOf(ApplicationFailure);
  await p.catalog.save({...film(1),title:'changed'});expect(store.observe().Film.length).toBe(2);
  for(const patch of [{offset:-1},{offset:100001},{offset:0.5},{limit:0},{limit:101},{maximumRuntimeMinutes:0},{maximumRuntimeMinutes:601},{sort:'bad'},{minimumStars:'bad'},{availability:'bad'},{releasedFrom:1800}])await expect(p.catalog.search({...filter,...patch} as FilmSearch)).rejects.toThrow();
  await expect(p.catalog.save({...film(3),runtimeMinutes:0})).rejects.toThrow();
  await expect(p.catalog.save({...film(3),genres:['bad']})).rejects.toThrow();
 }finally{db.close();}
}));
test('comments, rating replacement and film-specific member identity',async()=>measured('community',async()=>{
 const {db,store,p}=fixture();try{
  await p.catalog.save(film(1));await p.catalog.save(film(2));expect(await p.presence.exists(id(1))).toBe(id(1));expect(await p.presence.exists(id(99))).toBeUndefined();
  expect((await p.ratings.summary(id(1),id(10))).memberRating).toBeUndefined();
  const comment=await p.comments.append({filmId:id(1),authorId:id(10),text:'Great film'});expect(comment.createdAt).toBe(now);expect(comment.id).toBe(id(30));expect((await p.comments.listByFilm(id(2))).total).toBe(0);expect((await p.comments.listByFilm(id(1))).items).toEqual([comment]);
  await expect(p.comments.append({filmId:id(1),authorId:id(10),text:'   '})).rejects.toThrow(TypeError);
  await p.ratings.upsert({filmId:id(1),memberId:id(10),stars:'ThreeStars'});await p.ratings.upsert({filmId:id(1),memberId:id(10),stars:'FiveStars'});
  expect(await p.ratings.summary(id(1),id(10))).toEqual({filmId:id(1),averageStars:5,ratingCount:1,memberRating:'FiveStars'});
  await p.ratings.upsert({filmId:id(1),memberId:id(11),stars:'FourStars'});expect((await p.ratings.summary(id(1),id(12))).averageStars).toBe(4.5);
  await p.ratings.upsert({filmId:id(2),memberId:id(10),stars:'OneStar'});expect(store.observe().Rating.length).toBe(3);
  expect((await p.catalog.search({...filter,minimumStars:'FourStars',sort:'HighestRated'})).items.map(f=>f.id)).toEqual([id(1)]);
  await expect(p.ratings.upsert({filmId:id(1),memberId:id(10),stars:'bad' as never})).rejects.toThrow();
 }finally{db.close();}
}));
test('voting replacement, candidate-order tie, closure and failure preservation',async()=>measured('voting',async()=>{
 const {db,store,p}=fixture();try{
  expect(await p.voting.current()).toBeUndefined();await p.catalog.save(film(1));await p.catalog.save(film(2));await p.catalog.save(film(3));
  const open={pollId:id(20),scheduledFor:now,candidateFilmIds:[id(2),id(1)]};expect((await p.voting.open(open)).winningFilmId).toBeUndefined();
  await p.voting.cast({pollId:id(20),memberId:id(10),filmId:id(1)});await p.voting.cast({pollId:id(20),memberId:id(10),filmId:id(2)});
  expect(store.observe().FilmVote.length).toBe(1);
  await p.voting.cast({pollId:id(20),memberId:id(11),filmId:id(1)});expect((await p.voting.current())!.winningFilmId).toBe(id(2));
  expect((await p.catalog.search({...filter,sort:'MostVoted'})).items[0].currentVoteCount).toBe(1);
  for(const invalid of [id(3),id(99)])await expect(p.voting.cast({pollId:id(20),memberId:id(12),filmId:invalid})).rejects.toMatchObject({code:'FilmNotFound'});
  expect((await p.voting.close(id(20))).status).toBe('Closed');const before=store.observe();
  await expect(p.voting.cast({pollId:id(20),memberId:id(10),filmId:id(1)})).rejects.toMatchObject({code:'VotingClosed'});
  await expect(p.voting.close(id(20))).rejects.toMatchObject({code:'VotingClosed'});expect(store.observe()).toEqual(before);
  await expect(p.voting.close(id(99))).rejects.toMatchObject({code:'PollNotFound'});
  await p.voting.open(open);expect(store.observe().FilmVote).toEqual([]);expect((await p.voting.current())!.status).toBe('Open');
  await p.voting.open({pollId:id(21),scheduledFor:now,candidateFilmIds:[]});expect((await p.voting.current())!.pollId).toBe(id(21));
  await expect(p.voting.open({...open,scheduledFor:'bad'})).rejects.toThrow(TypeError);
 }finally{db.close();}
}));
test('Hono routes use actual scoped Awilix services, declared failures and real storage',async()=>measured('http-di',async()=>{
 const events:ApplicationEvent[]=[];const session=new CinemaSession(':memory:',event=>events.push(event));try{
  expect(session.scope.resolve('movieNight')).toBe(session.service);const another=session.container.createScope();expect(another.resolve('movieNight')).not.toBe(session.service);await another.dispose();
  const app=session.http;const request=async(path:string,method='GET',body?:unknown)=>app.request(path,{method,headers:{'content-type':'application/json'},...(body===undefined?{}:{body:JSON.stringify(body)})});
  expect((await request('/films','POST',film(1))).status).toBe(200);expect((await request('/films?query=MOVIE')).status).toBe(200);
  expect((await(await request('/films/'+id(1))).json()).film.id).toBe(id(1));expect((await request('/films/'+id(99))).status).toBe(404);
  expect((await request('/films/'+id(1)+'/availability','PATCH',{availability:'Watched'})).status).toBe(200);
  expect((await request('/films/'+id(1)+'/comments','POST',{authorId:id(10),text:'Good'})).status).toBe(200);expect((await request('/films/'+id(1)+'/comments')).status).toBe(200);
  expect((await request('/films/'+id(1)+'/rating','PUT',{memberId:id(10),stars:'FiveStars'})).status).toBe(200);expect((await request('/films/'+id(1)+'/rating?memberId='+id(10))).status).toBe(200);
  expect((await request('/voting','POST',{pollId:id(20),scheduledFor:now,candidateFilmIds:[id(1)]})).status).toBe(200);
  expect((await request('/voting/'+id(20)+'/votes','PUT',{memberId:id(10),filmId:id(1)})).status).toBe(200);expect((await request('/voting/current')).status).toBe(200);
  expect((await request('/voting/'+id(20)+'/close','POST')).status).toBe(200);expect((await request('/voting/'+id(20)+'/votes','PUT',{memberId:id(10),filmId:id(1)})).status).toBe(409);
  expect((await request('/films?limit=0')).status).toBe(400);expect(events.map(e=>e.name)).toContain('VotingCompleted');
 }finally{await session.close();}
}));
test('closed native database reports the declared persistence failure',async()=>measured('unavailable-storage',async()=>{
 const {db,p}=fixture();db.close();await expect(p.catalog.search(filter)).rejects.toMatchObject({code:'PersistenceUnavailable'});
}));
test('HTTP error mapping, default events and failed composition cleanup are real',async()=>measured('http-failure-cleanup',async()=>{
 const normal=new CinemaSession(':memory:');try{
  await normal.service.saveFilm(film(1));normal.store.db.close();expect((await normal.http.request('/films')).status).toBe(503);
 }finally{await normal.close();}
 const db=new Database(':memory:');const store=new CinemaStore(db);const ports={...store.ports,catalog:{...store.ports.catalog,search:async()=>{throw new Error('private diagnostic');}}};
 const broken=new CinemaSession(':memory:',()=>{},undefined,ports);try{
  const reply=await broken.http.request('/films');expect(reply.status).toBe(500);expect(await reply.json()).toEqual({error:'Internal error'});
 }finally{await broken.close();db.close();}
 let retained:import('../application/movie-night.ts').MovieNightPorts|undefined;
 expect(()=>new CinemaSession(':memory:',()=>{},undefined,ports=>{retained=ports;throw new Error('failed registration');})).toThrow('failed registration');
 await expect(retained!.catalog.search(filter)).rejects.toMatchObject({code:'PersistenceUnavailable'});
}));
