// Generated from the published model, not a second case generator.
// model-sha256: f71eaef76b7531349ddb940b8894632a73068e53032f1710a68357736b193f8d
import type {MovieNight,MovieNightPorts} from '../../../src/application/movie-night.ts';
import type {VotingRules} from '../../../src/domain/voting-rules.ts';
import {BindingError,type Invocation} from '../library/index.ts';
export type SurfaceBindings=Pick<MovieNight,"changeFilmAvailability"|"getFilm"|"saveFilm"|"searchFilms"|"addComment"|"getFilmRating"|"listComments"|"rateFilm"|"castVote"|"closeVoting"|"getCurrentVoting"|"openVoting">;
export type NativeBindings={service:SurfaceBindings;ports:MovieNightPorts;rules:VotingRules};
export type ArgumentReader={argument<T>(invocation:Invocation,index:number):T};
export function createCinemaFacade(bindings:NativeBindings){
 return async(invocation:Invocation,input:ArgumentReader):Promise<unknown>=>{
  switch(invocation.subject){
  case "HomeCinema.MovieNight.Catalog.FilmCatalogApi.changeFilmAvailability": return bindings.service.changeFilmAvailability(input.argument<Parameters<MovieNight['changeFilmAvailability']>[0]>(invocation,0));
  case "HomeCinema.MovieNight.Catalog.FilmCatalogApi.getFilm": return bindings.service.getFilm(input.argument<Parameters<MovieNight['getFilm']>[0]>(invocation,0));
  case "HomeCinema.MovieNight.Catalog.FilmCatalogApi.saveFilm": return bindings.service.saveFilm(input.argument<Parameters<MovieNight['saveFilm']>[0]>(invocation,0));
  case "HomeCinema.MovieNight.Catalog.FilmCatalogApi.searchFilms": return bindings.service.searchFilms(input.argument<Parameters<MovieNight['searchFilms']>[0]>(invocation,0));
  case "HomeCinema.MovieNight.Catalog.FilmCatalogStore.changeAvailability": return bindings.ports.catalog.changeAvailability(input.argument<Parameters<MovieNightPorts['catalog']['changeAvailability']>[0]>(invocation,0));
  case "HomeCinema.MovieNight.Catalog.FilmCatalogStore.find": return bindings.ports.catalog.find(input.argument<Parameters<MovieNightPorts['catalog']['find']>[0]>(invocation,0));
  case "HomeCinema.MovieNight.Catalog.FilmCatalogStore.save": return bindings.ports.catalog.save(input.argument<Parameters<MovieNightPorts['catalog']['save']>[0]>(invocation,0));
  case "HomeCinema.MovieNight.Catalog.FilmCatalogStore.search": return bindings.ports.catalog.search(input.argument<Parameters<MovieNightPorts['catalog']['search']>[0]>(invocation,0));
  case "HomeCinema.MovieNight.Community.CommentStore.append": return bindings.ports.comments.append(input.argument<Parameters<MovieNightPorts['comments']['append']>[0]>(invocation,0));
  case "HomeCinema.MovieNight.Community.CommentStore.listByFilm": return bindings.ports.comments.listByFilm(input.argument<Parameters<MovieNightPorts['comments']['listByFilm']>[0]>(invocation,0));
  case "HomeCinema.MovieNight.Community.FilmCommunityApi.addComment": return bindings.service.addComment(input.argument<Parameters<MovieNight['addComment']>[0]>(invocation,0));
  case "HomeCinema.MovieNight.Community.FilmCommunityApi.getFilmRating": return bindings.service.getFilmRating(input.argument<Parameters<MovieNight['getFilmRating']>[0]>(invocation,0),input.argument<Parameters<MovieNight['getFilmRating']>[1]>(invocation,1));
  case "HomeCinema.MovieNight.Community.FilmCommunityApi.listComments": return bindings.service.listComments(input.argument<Parameters<MovieNight['listComments']>[0]>(invocation,0));
  case "HomeCinema.MovieNight.Community.FilmCommunityApi.rateFilm": return bindings.service.rateFilm(input.argument<Parameters<MovieNight['rateFilm']>[0]>(invocation,0));
  case "HomeCinema.MovieNight.Community.FilmPresence.exists": return bindings.ports.presence.exists(input.argument<Parameters<MovieNightPorts['presence']['exists']>[0]>(invocation,0));
  case "HomeCinema.MovieNight.Community.RatingStore.summary": return bindings.ports.ratings.summary(input.argument<Parameters<MovieNightPorts['ratings']['summary']>[0]>(invocation,0),input.argument<Parameters<MovieNightPorts['ratings']['summary']>[1]>(invocation,1));
  case "HomeCinema.MovieNight.Community.RatingStore.upsert": return bindings.ports.ratings.upsert(input.argument<Parameters<MovieNightPorts['ratings']['upsert']>[0]>(invocation,0));
  case "HomeCinema.MovieNight.ScreeningPlanning.ScreeningPlanningApi.castVote": return bindings.service.castVote(input.argument<Parameters<MovieNight['castVote']>[0]>(invocation,0));
  case "HomeCinema.MovieNight.ScreeningPlanning.ScreeningPlanningApi.closeVoting": return bindings.service.closeVoting(input.argument<Parameters<MovieNight['closeVoting']>[0]>(invocation,0));
  case "HomeCinema.MovieNight.ScreeningPlanning.ScreeningPlanningApi.getCurrentVoting": return bindings.service.getCurrentVoting();
  case "HomeCinema.MovieNight.ScreeningPlanning.ScreeningPlanningApi.openVoting": return bindings.service.openVoting(input.argument<Parameters<MovieNight['openVoting']>[0]>(invocation,0));
  case "HomeCinema.MovieNight.ScreeningPlanning.VotingRules.candidateOrderWinner": return bindings.rules.candidateOrderWinner(input.argument<Parameters<VotingRules['candidateOrderWinner']>[0]>(invocation,0),input.argument<Parameters<VotingRules['candidateOrderWinner']>[1]>(invocation,1));
  case "HomeCinema.MovieNight.ScreeningPlanning.VotingStore.cast": return bindings.ports.voting.cast(input.argument<Parameters<MovieNightPorts['voting']['cast']>[0]>(invocation,0));
  case "HomeCinema.MovieNight.ScreeningPlanning.VotingStore.close": return bindings.ports.voting.close(input.argument<Parameters<MovieNightPorts['voting']['close']>[0]>(invocation,0));
  case "HomeCinema.MovieNight.ScreeningPlanning.VotingStore.current": return bindings.ports.voting.current();
  case "HomeCinema.MovieNight.ScreeningPlanning.VotingStore.open": return bindings.ports.voting.open(input.argument<Parameters<MovieNightPorts['voting']['open']>[0]>(invocation,0));
   default:throw new BindingError('No generated native connection for '+invocation.subject);
  }
 };
}
export function observedPorts(actual:MovieNightPorts,observe:<T>(subject:string,args:readonly unknown[],invoke:()=>Promise<T>)=>Promise<T>):MovieNightPorts {
 return {
  catalog:{
   changeAvailability:(...args:Parameters<MovieNightPorts['catalog']['changeAvailability']>)=>observe("HomeCinema.MovieNight.Catalog.FilmCatalogStore.changeAvailability",args,()=>actual.catalog.changeAvailability(...args)),
   find:(...args:Parameters<MovieNightPorts['catalog']['find']>)=>observe("HomeCinema.MovieNight.Catalog.FilmCatalogStore.find",args,()=>actual.catalog.find(...args)),
   save:(...args:Parameters<MovieNightPorts['catalog']['save']>)=>observe("HomeCinema.MovieNight.Catalog.FilmCatalogStore.save",args,()=>actual.catalog.save(...args)),
   search:(...args:Parameters<MovieNightPorts['catalog']['search']>)=>observe("HomeCinema.MovieNight.Catalog.FilmCatalogStore.search",args,()=>actual.catalog.search(...args)),
  },
  comments:{
   append:(...args:Parameters<MovieNightPorts['comments']['append']>)=>observe("HomeCinema.MovieNight.Community.CommentStore.append",args,()=>actual.comments.append(...args)),
   listByFilm:(...args:Parameters<MovieNightPorts['comments']['listByFilm']>)=>observe("HomeCinema.MovieNight.Community.CommentStore.listByFilm",args,()=>actual.comments.listByFilm(...args)),
  },
  presence:{
   exists:(...args:Parameters<MovieNightPorts['presence']['exists']>)=>observe("HomeCinema.MovieNight.Community.FilmPresence.exists",args,()=>actual.presence.exists(...args)),
  },
  ratings:{
   summary:(...args:Parameters<MovieNightPorts['ratings']['summary']>)=>observe("HomeCinema.MovieNight.Community.RatingStore.summary",args,()=>actual.ratings.summary(...args)),
   upsert:(...args:Parameters<MovieNightPorts['ratings']['upsert']>)=>observe("HomeCinema.MovieNight.Community.RatingStore.upsert",args,()=>actual.ratings.upsert(...args)),
  },
  voting:{
   cast:(...args:Parameters<MovieNightPorts['voting']['cast']>)=>observe("HomeCinema.MovieNight.ScreeningPlanning.VotingStore.cast",args,()=>actual.voting.cast(...args)),
   close:(...args:Parameters<MovieNightPorts['voting']['close']>)=>observe("HomeCinema.MovieNight.ScreeningPlanning.VotingStore.close",args,()=>actual.voting.close(...args)),
   current:(...args:Parameters<MovieNightPorts['voting']['current']>)=>observe("HomeCinema.MovieNight.ScreeningPlanning.VotingStore.current",args,()=>actual.voting.current(...args)),
   open:(...args:Parameters<MovieNightPorts['voting']['open']>)=>observe("HomeCinema.MovieNight.ScreeningPlanning.VotingStore.open",args,()=>actual.voting.open(...args)),
  },
 };
}
