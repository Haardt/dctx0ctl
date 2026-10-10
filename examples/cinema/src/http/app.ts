import {Hono} from 'hono';
import {ApplicationFailure,type MovieNight,type FilmSearch} from '../application/movie-night.ts';
/** Explicit public HTTP mapping, owned by this application. */
export function createHttpApp(service:MovieNight){
 const app=new Hono();
 app.onError((error,c)=>{
  if(error instanceof ApplicationFailure)return c.json({failure:error.code,payload:error.payload},error.code==='PersistenceUnavailable'?503:error.code==='VotingClosed'?409:404);
  if(error instanceof TypeError||error instanceof RangeError)return c.json({error:error.message},400);
  return c.json({error:'Internal error'},500);
 });
 app.get('/films',async c=>{
  const q=c.req.query();const filter:FilmSearch={query:q.query,genre:q.genre,minimumStars:q.minimumStars as FilmSearch['minimumStars'],availability:q.availability as FilmSearch['availability'],maximumRuntimeMinutes:q.maximumRuntimeMinutes===undefined?undefined:Number(q.maximumRuntimeMinutes),releasedFrom:q.releasedFrom===undefined?undefined:Number(q.releasedFrom),sort:q.sort??'TitleAscending',offset:Number(q.offset??0),limit:Number(q.limit??20)};
  return c.json(await service.searchFilms(filter));
 });
 app.get('/films/:id',async c=>c.json(await service.getFilm(c.req.param('id'))));
 app.post('/films',async c=>c.json(await service.saveFilm(await c.req.json())));
 app.patch('/films/:id/availability',async c=>c.json(await service.changeFilmAvailability({filmId:c.req.param('id'),availability:(await c.req.json()).availability})));
 app.get('/films/:id/comments',async c=>c.json(await service.listComments(c.req.param('id'))));
 app.post('/films/:id/comments',async c=>c.json(await service.addComment({...await c.req.json(),filmId:c.req.param('id')})));
 app.get('/films/:id/rating',async c=>c.json(await service.getFilmRating(c.req.param('id'),c.req.query('memberId')??'')));
 app.put('/films/:id/rating',async c=>c.json(await service.rateFilm({...await c.req.json(),filmId:c.req.param('id')})));
 app.post('/voting',async c=>c.json(await service.openVoting(await c.req.json())));
 app.get('/voting/current',async c=>c.json((await service.getCurrentVoting())??null));
 app.put('/voting/:id/votes',async c=>c.json(await service.castVote({...await c.req.json(),pollId:c.req.param('id')})));
 app.post('/voting/:id/close',async c=>c.json(await service.closeVoting(c.req.param('id'))));
 return app;
}
