import {Database} from 'bun:sqlite';
import {ApplicationFailure,type MovieNightPorts,type FilmSummary,type FilmDetails,type SaveFilm,type FilmSearch,type CommentView,type RatingSummary,type Stars,type OpenVoting,type VotingOverview,type CastVote} from '../application/movie-night.ts';
import {candidateOrderWinner} from '../domain/candidate-order-winner.ts';
import {filmId} from '../domain/film-id.ts';
import {releaseYear} from '../domain/release-year.ts';

export interface Sources { now():string; id():string }
const defaults:Sources={now:()=>new Date().toISOString(),id:()=>crypto.randomUUID()};
const stars:Stars[]=['OneStar','TwoStars','ThreeStars','FourStars','FiveStars'];
type Film=SaveFilm;
type Poll={id:string;scheduledFor:string;candidateFilmIds:string[];status:'Open'|'Closed'};
type Rating={filmId:string;memberId:string;stars:Stars};
type Vote={pollId:string;memberId:string;filmId:string};

/** Real SQLite-backed ports. A database and its lifetime belong to the caller. */
export class CinemaStore {
 readonly ports:MovieNightPorts;
 constructor(readonly db:Database,private readonly sources:Sources=defaults){
  db.exec(`PRAGMA foreign_keys=ON;
   CREATE TABLE IF NOT EXISTS films(id TEXT PRIMARY KEY,document TEXT NOT NULL);
   CREATE TABLE IF NOT EXISTS comments(id TEXT PRIMARY KEY,film_id TEXT NOT NULL REFERENCES films(id),document TEXT NOT NULL);
   CREATE TABLE IF NOT EXISTS ratings(film_id TEXT NOT NULL REFERENCES films(id),member_id TEXT NOT NULL,document TEXT NOT NULL,PRIMARY KEY(film_id,member_id));
   CREATE TABLE IF NOT EXISTS polls(id TEXT PRIMARY KEY,document TEXT NOT NULL);
   CREATE TABLE IF NOT EXISTS votes(poll_id TEXT NOT NULL REFERENCES polls(id),member_id TEXT NOT NULL,film_id TEXT NOT NULL REFERENCES films(id),document TEXT NOT NULL,PRIMARY KEY(poll_id,member_id));`);
  this.ports={
   catalog:{search:async filter=>this.guard(()=>this.search(filter)),find:async id=>this.guard(()=>this.find(id)),save:async request=>this.guard(()=>this.save(request)),changeAvailability:async request=>this.guard(()=>{
    const film=this.requireFilm(request.filmId);this.availability(request.availability);const changed={...film,availability:request.availability};this.putFilm(changed);return this.details(changed);
   })},
   presence:{exists:async id=>this.guard(()=>this.getFilm(id)?.id)},
   comments:{listByFilm:async id=>this.guard(()=>{filmId(id);const items=this.rows<CommentView>('comments').filter(comment=>comment.filmId===id);return {items,total:items.length};}),append:async request=>this.guard(()=>{
    filmId(request.filmId);filmId(request.authorId);if(typeof request.text!=='string'||!request.text.trim())throw new TypeError('Comment text must be visible');
    const comment={id:filmId(this.sources.id()),...request,createdAt:this.timestamp(this.sources.now())};
    this.db.query('INSERT INTO comments VALUES(?,?,?)').run(comment.id,comment.filmId,JSON.stringify(comment));return comment;
   })},
   ratings:{summary:async(film,member)=>this.guard(()=>this.ratingSummary(film,member)),upsert:async request=>this.guard(()=>{
    filmId(request.filmId);filmId(request.memberId);this.star(request.stars);
    this.db.query('INSERT INTO ratings VALUES(?,?,?) ON CONFLICT(film_id,member_id) DO UPDATE SET document=excluded.document').run(request.filmId,request.memberId,JSON.stringify(request));
    return this.ratingSummary(request.filmId,request.memberId);
   })},
   voting:{open:async request=>this.guard(()=>this.open(request)),current:async()=>this.guard(()=>{const polls=this.rows<Poll>('polls');const poll=[...polls].reverse().find(poll=>poll.status==='Open')??polls.at(-1);return poll?this.overview(poll):undefined;}),cast:async request=>this.guard(()=>this.cast(request)),close:async id=>this.guard(()=>this.db.transaction(()=>{
    const poll=this.requireOpenPoll(id);const closed:Poll={...poll,status:'Closed'};this.putPoll(closed);return this.overview(closed);
   })())},
  };
 }
 private guard<T>(operation:()=>T):T {
  try{return operation();}catch(error){if(error instanceof ApplicationFailure||error instanceof TypeError)throw error;throw new ApplicationFailure('PersistenceUnavailable',{reason:error instanceof Error?error.message:String(error)});}
 }
 private rows<T>(table:'films'|'comments'|'ratings'|'polls'|'votes'):T[]{return (this.db.query(`SELECT document FROM ${table} ORDER BY rowid`).all() as {document:string}[]).map(row=>JSON.parse(row.document) as T);}
 private getFilm(id:string):Film|undefined {filmId(id);const row=this.db.query('SELECT document FROM films WHERE id=?').get(id) as {document:string}|null;return row?JSON.parse(row.document) as Film:undefined;}
 private requireFilm(id:string):Film {const film=this.getFilm(id);if(!film)throw new ApplicationFailure('FilmNotFound',{filmId:id});return film;}
 private putFilm(film:Film){this.db.query('INSERT INTO films VALUES(?,?) ON CONFLICT(id) DO UPDATE SET document=excluded.document').run(film.id,JSON.stringify(film));}
 private putPoll(poll:Poll){this.db.query('INSERT INTO polls VALUES(?,?) ON CONFLICT(id) DO UPDATE SET document=excluded.document').run(poll.id,JSON.stringify(poll));}
 private year(value:number){try{return releaseYear(value);}catch(error){throw new TypeError(error instanceof Error?error.message:String(error));}}
 private star(value:Stars){if(!stars.includes(value))throw new TypeError('Unknown star rating');}
 private availability(value:string){if(!['Available','Watched','Wishlist','Unavailable'].includes(value))throw new TypeError('Unknown availability');}
 private timestamp(value:string){if(typeof value!=='string'||!/^\d{4}-\d{2}-\d{2}T.*(?:Z|[+-]\d{2}:\d{2})$/.test(value)||!Number.isFinite(Date.parse(value)))throw new TypeError('Expected RFC3339 timestamp');return value;}
 private save(request:SaveFilm):FilmDetails {
  filmId(request.id);this.year(request.releaseYear);this.availability(request.availability);
  if(typeof request.title!=='string'||!Array.isArray(request.genres)||request.genres.some(genre=>!['Action','Adventure','Animation','Comedy','Crime','Documentary','Drama','Fantasy','Horror','Mystery','Romance','ScienceFiction','Thriller','Western'].includes(genre))||!Number.isInteger(request.runtimeMinutes)||request.runtimeMinutes<1||request.runtimeMinutes>600)throw new TypeError('Invalid film details');
  const film={...request,genres:[...request.genres]};this.putFilm(film);return this.details(film);
 }
 private summary(film:Film):FilmSummary {
  const ratings=this.rows<Rating>('ratings').filter(rating=>rating.filmId===film.id);const current=this.rows<Poll>('polls').reverse().find(poll=>poll.status==='Open');
  return {...film,genres:[...film.genres],averageStars:ratings.length?ratings.reduce((sum,rating)=>sum+stars.indexOf(rating.stars)+1,0)/ratings.length:0,ratingCount:ratings.length,currentVoteCount:current?this.rows<Vote>('votes').filter(vote=>vote.pollId===current.id&&vote.filmId===film.id).length:0};
 }
 private details(film:Film):FilmDetails{return {film:this.summary(film)};}
 private find(id:string):FilmDetails|undefined{const film=this.getFilm(id);return film?this.details(film):undefined;}
 private search(filter:FilmSearch){
  if(!Number.isInteger(filter.offset)||filter.offset<0||filter.offset>100000||!Number.isInteger(filter.limit)||filter.limit<1||filter.limit>100)throw new TypeError('Invalid pagination');
  if(filter.minimumStars!==undefined)this.star(filter.minimumStars);
  if(filter.availability!==undefined)this.availability(filter.availability);
  if(filter.releasedFrom!==undefined)this.year(filter.releasedFrom);
  if(filter.maximumRuntimeMinutes!==undefined&&(!Number.isInteger(filter.maximumRuntimeMinutes)||filter.maximumRuntimeMinutes<1||filter.maximumRuntimeMinutes>600))throw new TypeError('Invalid runtime filter');
  if(!['TitleAscending','NewestFirst','HighestRated','MostVoted'].includes(filter.sort))throw new TypeError('Unknown film sort');
  const items=this.rows<Film>('films').map(film=>this.summary(film)).filter(film=>
   (filter.query===undefined||film.title.toLowerCase().includes(filter.query.toLowerCase()))&&
   (filter.genre===undefined||film.genres.includes(filter.genre))&&
   (filter.minimumStars===undefined||film.averageStars>=stars.indexOf(filter.minimumStars)+1)&&
   (filter.availability===undefined||film.availability===filter.availability)&&
   (filter.maximumRuntimeMinutes===undefined||film.runtimeMinutes<=filter.maximumRuntimeMinutes)&&
   (filter.releasedFrom===undefined||film.releaseYear>=filter.releasedFrom));
  items.sort((a,b)=>{
   const primary=filter.sort==='NewestFirst'?b.releaseYear-a.releaseYear:filter.sort==='HighestRated'?b.averageStars-a.averageStars:filter.sort==='MostVoted'?b.currentVoteCount-a.currentVoteCount:(a.title<b.title?-1:a.title>b.title?1:0);
   return primary||(a.id<b.id?-1:a.id>b.id?1:0);
  });
  return {items:items.slice(filter.offset,filter.offset+filter.limit),total:items.length,offset:filter.offset,limit:filter.limit};
 }
 private ratingSummary(filmIdValue:string,memberId:string):RatingSummary {
  filmId(filmIdValue);filmId(memberId);const ratings=this.rows<Rating>('ratings').filter(rating=>rating.filmId===filmIdValue);
  return {filmId:filmIdValue,averageStars:ratings.length?ratings.reduce((sum,rating)=>sum+stars.indexOf(rating.stars)+1,0)/ratings.length:0,ratingCount:ratings.length,memberRating:ratings.find(rating=>rating.memberId===memberId)?.stars};
 }
 private requireOpenPoll(id:string):Poll {
  filmId(id);const row=this.db.query('SELECT document FROM polls WHERE id=?').get(id) as {document:string}|null;
  if(!row)throw new ApplicationFailure('PollNotFound',{pollId:id});const poll=JSON.parse(row.document) as Poll;
  if(poll.status!=='Open')throw new ApplicationFailure('VotingClosed',{pollId:id});return poll;
 }
 private open(request:OpenVoting):VotingOverview {
  filmId(request.pollId);this.timestamp(request.scheduledFor);if(!Array.isArray(request.candidateFilmIds))throw new TypeError('Expected candidate list');
  request.candidateFilmIds.forEach(id=>filmId(id));
  const poll:Poll={id:request.pollId,scheduledFor:request.scheduledFor,candidateFilmIds:[...request.candidateFilmIds],status:'Open'};
  return this.db.transaction(()=>{this.db.query('DELETE FROM votes WHERE poll_id=?').run(poll.id);this.putPoll(poll);return this.overview(poll);})();
 }
 private cast(request:CastVote):VotingOverview {
  return this.db.transaction(()=>{
   const poll=this.requireOpenPoll(request.pollId);filmId(request.memberId);this.requireFilm(request.filmId);
   if(!poll.candidateFilmIds.includes(request.filmId))throw new ApplicationFailure('FilmNotFound',{filmId:request.filmId});
   this.db.query('INSERT INTO votes VALUES(?,?,?,?) ON CONFLICT(poll_id,member_id) DO UPDATE SET film_id=excluded.film_id,document=excluded.document').run(request.pollId,request.memberId,request.filmId,JSON.stringify(request));
   return this.overview(poll);
  })();
 }
 private overview(poll:Poll):VotingOverview {
  const votes=this.rows<Vote>('votes').filter(vote=>vote.pollId===poll.id).map(vote=>vote.filmId);
  return {pollId:poll.id,scheduledFor:poll.scheduledFor,status:poll.status,results:poll.candidateFilmIds.map(filmId=>({filmId,votes:votes.filter(vote=>vote===filmId).length})),winningFilmId:candidateOrderWinner(votes,poll.candidateFilmIds)};
 }
 /** Actual persisted entities, independent of any spec or harness representation. */
 observe(){return {Film:this.rows<Film>('films'),Comment:this.rows<CommentView>('comments'),Rating:this.rows<Rating>('ratings'),ScreeningPoll:this.rows<Poll>('polls'),FilmVote:this.rows<Vote>('votes')};}
}
