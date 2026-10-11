/** Application-owned runtime contracts for persisted data and public messages. */
import {filmId} from './film-id.ts';
import {releaseYear} from './release-year.ts';
import {timestamp as checkedTimestamp} from './timestamp.ts';

type Validator = (value: unknown) => void;
const text:Validator=value=>{if(typeof value!=='string')throw new TypeError('Expected text');};
const commentText:Validator=value=>{text(value);if(!(value as string).trim())throw new TypeError('Comment text must be visible');};
/** Unbounded counts use bigint when a native number cannot retain the value. */
export type ExactInteger = number | bigint;
const integer:Validator=value=>{if(typeof value!=='bigint'&&(typeof value!=='number'||!Number.isSafeInteger(value)))throw new TypeError('Expected an exact integer');};
const float:Validator=value=>{if(typeof value!=='number'||!Number.isFinite(value))throw new TypeError('Expected a finite number');};
const uuid:Validator=value=>{filmId(value);};
const year:Validator=value=>{releaseYear(value);};
const range=(minimum:number,maximum:number):Validator=>value=>{integer(value);if((value as number)<minimum||(value as number)>maximum)throw new TypeError('Number outside range');};
const alternatives=(...values:string[]):Validator=>value=>{if(typeof value!=='string'||!values.includes(value))throw new TypeError('Unknown alternative');};
const option=(check:Validator):Validator=>value=>{if(value!==undefined)check(value);};
const list=(check:Validator):Validator=>value=>{if(!Array.isArray(value))throw new TypeError('Expected list');for(const item of value)check(item);};
const record=(fields:Record<string,Validator>):Validator=>value=>{
 if(value===null||typeof value!=='object'||Array.isArray(value))throw new TypeError('Expected record');
 const actual=value as Record<string,unknown>;
 if(Object.keys(actual).length!==Object.keys(fields).length||Object.keys(actual).some(key=>!Object.hasOwn(fields,key)))throw new TypeError('Unexpected or missing record field');
 for(const [key,check] of Object.entries(fields)){if(!Object.hasOwn(actual,key))throw new TypeError('Missing record field');check(actual[key]);}
};
const timestamp:Validator=value=>{checkedTimestamp(value);};
const genre=alternatives('Action','Adventure','Animation','Comedy','Crime','Documentary','Drama','Fantasy','Horror','Mystery','Romance','ScienceFiction','Thriller','Western');
const availability=alternatives('Available','Watched','Wishlist','Unavailable');
const stars=alternatives('OneStar','TwoStars','ThreeStars','FourStars','FiveStars');
const status=alternatives('Open','Closed');
const sort=alternatives('TitleAscending','NewestFirst','HighestRated','MostVoted');
const runtime=range(1,600),offset=range(0,100000),limit=range(1,100);
const filmFields={id:uuid,title:text,releaseYear:year,genres:list(genre),runtimeMinutes:runtime,availability};
const summary=record({...filmFields,averageStars:float,ratingCount:integer,currentVoteCount:integer});
const comment=record({id:uuid,filmId:uuid,authorId:uuid,text:commentText,createdAt:timestamp});
const rating=record({filmId:uuid,memberId:uuid,stars});
const vote=record({pollId:uuid,memberId:uuid,filmId:uuid});
const open=record({pollId:uuid,scheduledFor:timestamp,candidateFilmIds:list(uuid)});
const contracts:Record<string,Validator>={
 FilmId:uuid,MemberId:uuid,CommentId:uuid,PollId:uuid,ReleaseYear:year,
 CommentText:commentText,
 RuntimeMinutes:runtime,PageOffset:offset,PageLimit:limit,
 Genre:genre,FilmAvailability:availability,FilmSort:sort,StarRating:stars,PollStatus:status,
 Film:record(filmFields),SaveFilm:record(filmFields),FilmSummary:summary,
 FilmOverview:record({items:list(summary),total:integer,offset:integer,limit:integer}),
 FilmDetails:record({film:summary}),
 FilmSearch:record({query:option(text),genre:option(genre),minimumStars:option(stars),availability:option(availability),maximumRuntimeMinutes:option(runtime),releasedFrom:option(year),sort,offset,limit}),
 ChangeAvailability:record({filmId:uuid,availability}),
 AddComment:record({filmId:uuid,authorId:uuid,text:commentText}),Comment:comment,CommentView:comment,
 CommentOverview:record({items:list(comment),total:integer}),Rating:rating,RateFilm:rating,
 RatingSummary:record({filmId:uuid,averageStars:float,ratingCount:integer,memberRating:option(stars)}),
 OpenVoting:open,CastVote:vote,FilmVote:vote,
 ScreeningPoll:record({id:uuid,scheduledFor:timestamp,candidateFilmIds:list(uuid),status}),
 FilmVoteResult:record({filmId:uuid,votes:integer}),
 VotingOverview:record({pollId:uuid,scheduledFor:timestamp,status,results:list(record({filmId:uuid,votes:integer})),winningFilmId:option(uuid)}),
 FilmSaved:record({filmId:uuid}),FilmAvailabilityChanged:record({filmId:uuid,availability}),
 CommentAdded:record({commentId:uuid,filmId:uuid}),FilmRated:rating,
 VotingOpened:record({pollId:uuid,scheduledFor:timestamp}),VoteCast:vote,
 VotingCompleted:record({pollId:uuid,winningFilmId:option(uuid)}),
 FilmNotFound:record({filmId:uuid}),CommentNotFound:record({commentId:uuid}),
 PollNotFound:record({pollId:uuid}),VotingClosed:record({pollId:uuid}),
 PersistenceUnavailable:record({reason:text}),
};
export const runtimeContractNames=Object.freeze(Object.keys(contracts));
/** Validate an actual application value without translating or fabricating it. */
export function validateRuntimeContract<T>(name:string,value:T):T {
 const validate=contracts[name];if(!validate)throw new TypeError('Unknown application contract: '+name);
 validate(value);return value;
}
