import {expect,test} from 'bun:test';
import {mkdtempSync,rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {CinemaSession} from '../../../src/app/session.ts';
const id=(n:number)=>'00000000-0000-4000-8000-'+String(n).padStart(12,'0');
test('real SQLite production adapters retain data across owned session lifetimes',async()=>{
 const root=mkdtempSync(join(tmpdir(),'cinema-conformance-'));
 const database=join(root,'cinema.sqlite3');
 const sources={now:()=> '2026-10-11T12:00:00Z',id:()=>id(30)};
 try{
  const first=new CinemaSession(database,()=>{},sources);
  try{
   await first.service.saveFilm({id:id(1),title:'Durable',releaseYear:2026,genres:['Comedy'],runtimeMinutes:90,availability:'Available'});
   await first.service.addComment({filmId:id(1),authorId:id(10),text:'Persistent'});
   await first.service.rateFilm({filmId:id(1),memberId:id(10),stars:'FiveStars'});
   await first.service.rateFilm({filmId:id(1),memberId:id(10),stars:'FourStars'});
   await first.service.openVoting({pollId:id(20),scheduledFor:sources.now(),candidateFilmIds:[id(1)]});
   await first.service.castVote({pollId:id(20),memberId:id(10),filmId:id(1)});
   await first.service.closeVoting(id(20));
  }finally{await first.close();}
  const reopened=new CinemaSession(database);
  try{
   expect(await reopened.service.getFilm(id(1))).toMatchObject({film:{title:'Durable',ratingCount:1,averageStars:4}});
   expect(await reopened.service.listComments(id(1))).toMatchObject({total:1,items:[{id:id(30),text:'Persistent'}]});
   expect(await reopened.service.getCurrentVoting()).toMatchObject({status:'Closed',winningFilmId:id(1),scheduledFor:sources.now()});
   const before=reopened.store.observe();
   await expect(reopened.service.castVote({pollId:id(20),memberId:id(10),filmId:id(1)})).rejects.toMatchObject({code:'VotingClosed'});
   expect(reopened.store.observe()).toEqual(before);
   expect(reopened.store.observe().Rating).toHaveLength(1);
   expect(reopened.store.observe().FilmVote).toHaveLength(1);
  }finally{await reopened.close();}
 }finally{rmSync(root,{recursive:true,force:true});}
});
