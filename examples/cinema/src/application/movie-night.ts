/** Application-owned boundary messages and services. No testing/runtime imports. */
import {validateRuntimeContract,type ExactInteger} from '../domain/contracts.ts';
export type Availability = 'Available' | 'Watched' | 'Wishlist' | 'Unavailable';
export type Stars = 'OneStar' | 'TwoStars' | 'ThreeStars' | 'FourStars' | 'FiveStars';
export type Genre = string;
export interface FilmSearch { query: string | undefined; genre: Genre | undefined; minimumStars: Stars | undefined; availability: Availability | undefined; maximumRuntimeMinutes: number | undefined; releasedFrom: number | undefined; sort: string; offset: number; limit: number }
export interface FilmSummary { id: string; title: string; releaseYear: number; genres: Genre[]; runtimeMinutes: number; availability: Availability; averageStars: number; ratingCount: ExactInteger; currentVoteCount: ExactInteger }
export interface FilmOverview { items: FilmSummary[]; total: ExactInteger; offset: ExactInteger; limit: ExactInteger }
export interface FilmDetails { film: FilmSummary }
export interface SaveFilm { id: string; title: string; releaseYear: number; genres: Genre[]; runtimeMinutes: number; availability: Availability }
export interface ChangeAvailability { filmId: string; availability: Availability }
export interface AddComment { filmId: string; authorId: string; text: string }
export interface CommentView { id: string; filmId: string; authorId: string; text: string; createdAt: string }
export interface CommentOverview { items: CommentView[]; total: ExactInteger }
export interface RateFilm { filmId: string; memberId: string; stars: Stars }
export interface RatingSummary { filmId: string; averageStars: number; ratingCount: ExactInteger; memberRating: Stars | undefined }
export interface OpenVoting { pollId: string; scheduledFor: string; candidateFilmIds: string[] }
export interface CastVote { pollId: string; memberId: string; filmId: string }
export interface VotingOverview { pollId: string; scheduledFor: string; status: 'Open' | 'Closed'; results: {filmId: string; votes: ExactInteger}[]; winningFilmId: string | undefined }

export class ApplicationFailure extends Error {
 constructor(public readonly code: string, public readonly payload: Record<string,unknown>) { super(code);validateRuntimeContract(code,payload); }
}
export interface MovieNightPorts {
 catalog: {
  search(filter: FilmSearch): Promise<FilmOverview>;
  find(id: string): Promise<FilmDetails | undefined>;
  save(request: SaveFilm): Promise<FilmDetails>;
  changeAvailability(request: ChangeAvailability): Promise<FilmDetails>;
 };
 presence: { exists(filmId: string): Promise<string | undefined> };
 comments: { listByFilm(filmId: string): Promise<CommentOverview>; append(request: AddComment): Promise<CommentView> };
 ratings: { summary(filmId: string, memberId: string): Promise<RatingSummary>; upsert(request: RateFilm): Promise<RatingSummary> };
 voting: { open(request: OpenVoting): Promise<VotingOverview>; current(): Promise<VotingOverview | undefined>; cast(request: CastVote): Promise<VotingOverview>; close(pollId: string): Promise<VotingOverview> };
}
export type ApplicationEvent =
 | {name: 'FilmSaved'; payload: {filmId: string}}
 | {name: 'FilmAvailabilityChanged'; payload: ChangeAvailability}
 | {name: 'CommentAdded'; payload: {commentId: string; filmId: string}}
 | {name: 'FilmRated'; payload: RateFilm}
 | {name: 'VotingOpened'; payload: {pollId: string; scheduledFor: string}}
 | {name: 'VoteCast'; payload: CastVote}
 | {name: 'VotingCompleted'; payload: {pollId: string; winningFilmId: string | undefined}};

/** Existing dependency boundaries may supply real persistence or test doubles. */
export class MovieNight {
 private readonly emit:(event:ApplicationEvent)=>void;
 constructor(private readonly ports: MovieNightPorts, publish: (event: ApplicationEvent)=>void) {this.emit=publish;}
 private publish(event:ApplicationEvent){validateRuntimeContract(event.name,event.payload);this.emit(event);}
 searchFilms(filter: FilmSearch) { return this.ports.catalog.search(filter); }
 async getFilm(id: string) {
  const details = await this.ports.catalog.find(id);
  if (!details) throw new ApplicationFailure('FilmNotFound',{filmId:id});
  return details;
 }
 async saveFilm(request: SaveFilm) {
  const details = await this.ports.catalog.save(request);
  this.publish({name:'FilmSaved',payload:{filmId:request.id}});
  return details;
 }
 async changeFilmAvailability(request: ChangeAvailability) {
  const details = await this.ports.catalog.changeAvailability(request);
  this.publish({name:'FilmAvailabilityChanged',payload:{filmId:request.filmId,availability:request.availability}});
  return details;
 }
 private async requireFilm(id: string) {
  if (await this.ports.presence.exists(id) === undefined) throw new ApplicationFailure('FilmNotFound',{filmId:id});
 }
 async listComments(id: string) { await this.requireFilm(id); return this.ports.comments.listByFilm(id); }
 async addComment(request: AddComment) {
  await this.requireFilm(request.filmId);
  const comment = await this.ports.comments.append(request);
  this.publish({name:'CommentAdded',payload:{commentId:comment.id,filmId:request.filmId}});
  return comment;
 }
 async getFilmRating(filmId: string, memberId: string) { await this.requireFilm(filmId); return this.ports.ratings.summary(filmId,memberId); }
 async rateFilm(request: RateFilm) {
  await this.requireFilm(request.filmId);
  const summary = await this.ports.ratings.upsert(request);
  this.publish({name:'FilmRated',payload:{filmId:request.filmId,memberId:request.memberId,stars:request.stars}});
  return summary;
 }
 async openVoting(request: OpenVoting) {
  const voting = await this.ports.voting.open(request);
  this.publish({name:'VotingOpened',payload:{pollId:request.pollId,scheduledFor:request.scheduledFor}});
  return voting;
 }
 getCurrentVoting() { return this.ports.voting.current(); }
 async castVote(request: CastVote) {
  const voting = await this.ports.voting.cast(request);
  this.publish({name:'VoteCast',payload:{pollId:request.pollId,memberId:request.memberId,filmId:request.filmId}});
  return voting;
 }
 async closeVoting(pollId: string) {
  const voting = await this.ports.voting.close(pollId);
  this.publish({name:'VotingCompleted',payload:{pollId,winningFilmId:voting.winningFilmId}});
  return voting;
 }
}
