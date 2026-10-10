import {Database} from 'bun:sqlite';
import {asClass,asValue,createContainer,InjectionMode} from 'awilix';
import {MovieNight,type MovieNightPorts,type ApplicationEvent} from '../application/movie-night.ts';
import {CinemaStore,type Sources} from '../persistence/sqlite.ts';
import {createHttpApp} from '../http/app.ts';

/** One owned database/container lifetime. No global application singleton. */
export class CinemaSession {
 readonly store:CinemaStore;
 readonly container;
 readonly scope;
 readonly service:MovieNight;
 readonly http;
 constructor(database:string, publish:(event:ApplicationEvent)=>void=()=>{}, sources?:Sources, ports?:MovieNightPorts|((ports:MovieNightPorts)=>MovieNightPorts)){
  const db=new Database(database);
  try{
   this.store=new CinemaStore(db,sources);
   this.container=createContainer<{ports:MovieNightPorts;publish:(event:ApplicationEvent)=>void;movieNight:MovieNight}>({injectionMode:InjectionMode.CLASSIC,strict:true});
   const selected=typeof ports==='function'?ports(this.store.ports):ports??this.store.ports;
   this.container.register({ports:asValue(selected),publish:asValue(publish),movieNight:asClass(MovieNight).scoped()});
   this.scope=this.container.createScope();
   this.service=this.scope.resolve('movieNight');
   this.http=createHttpApp(this.service);
  }catch(error){db.close();throw error;}
 }
 async close(){try{await this.scope.dispose();await this.container.dispose();}finally{this.store.db.close();}}
}
