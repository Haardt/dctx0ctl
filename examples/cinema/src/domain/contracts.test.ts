import {expect,test} from 'bun:test';
import {validateRuntimeContract} from './contracts.ts';
import {CinemaStore} from '../persistence/sqlite.ts';
import {Database} from 'bun:sqlite';

const id='00000000-0000-4000-8000-000000000001';
test('application output validation preserves values and rejects invalid nested summaries',()=>{
 const film={id,title:'Cinema',releaseYear:2026,genres:['Drama'],runtimeMinutes:90,availability:'Available',averageStars:4.5,ratingCount:2,currentVoteCount:1};
 const details={film};
 expect(validateRuntimeContract('FilmDetails',details)).toBe(details);
 for(const invalid of [{film:{...film,id:'broken'}},{film:{...film,averageStars:NaN}},{film:{...film,ratingCount:1.5}},{film:{...film,genres:['Unknown']}},{film:{...film,extra:true}}]){
  expect(()=>validateRuntimeContract('FilmDetails',invalid)).toThrow(TypeError);
 }
});
test('events and failure payloads validate real fields without replacing values',()=>{
 const event={pollId:id,winningFilmId:undefined};
 expect(validateRuntimeContract('VotingCompleted',event)).toBe(event);
 expect(()=>validateRuntimeContract('VotingCompleted',{pollId:id})).toThrow(TypeError);
 expect(()=>validateRuntimeContract('FilmNotFound',{filmId:'broken'})).toThrow(TypeError);
 expect(()=>validateRuntimeContract('PersistenceUnavailable',{reason:42})).toThrow(TypeError);
 expect(()=>validateRuntimeContract('Unknown',{})).toThrow(TypeError);
});
test('persisted malformed data cannot escape through an application port',async()=>{
 const db=new Database(':memory:');
 try{
  const store=new CinemaStore(db);
  db.query('INSERT INTO films VALUES (?,?)').run(id,JSON.stringify({id,title:'Broken',releaseYear:2026,genres:[],runtimeMinutes:0,availability:'Available'}));
  await expect(store.ports.catalog.find(id)).rejects.toThrow(TypeError);
 }finally{db.close();}
});
test('comments require visible text and preserve the original content',()=>{
 for(const text of ['', ' ', '\n\t'])expect(()=>validateRuntimeContract('AddComment',{filmId:id,authorId:id,text})).toThrow(TypeError);
 expect(validateRuntimeContract('AddComment',{filmId:id,authorId:id,text:' Cinema '}).text).toBe(' Cinema ');
});
test('unbounded DTO integers preserve values beyond native number precision',()=>{
 const large=9007199254740993n;
 const overview={items:[],total:large};
 expect(validateRuntimeContract('CommentOverview',overview)).toBe(overview);
 expect(overview.total).toBe(large);
 expect(()=>validateRuntimeContract('CommentOverview',{items:[],total:Number(large)})).toThrow(TypeError);
 expect(()=>validateRuntimeContract('CommentOverview',{items:[],total:0.5})).toThrow(TypeError);
});
