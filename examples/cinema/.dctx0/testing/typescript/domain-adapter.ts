/** External access to existing Cinema validators; no framework/session imports. */
import {filmId} from '../../../src/domain/film-id.ts';
import {releaseYear} from '../../../src/domain/release-year.ts';
import type {ProjectAdapter,Value} from './contract.ts';
const root='HomeCinema.MovieNight.';
const ids=['FilmId','MemberId','CommentId','PollId'];
export const adapter:ProjectAdapter={
 async describe(){return {protocol_version:1,subjects:[...ids.map(name=>({subject:root+name,role:'contract',artifact:'src/domain/film-id.ts',entry_point:'filmId'})),{subject:root+'ReleaseYear',role:'contract',artifact:'src/domain/release-year.ts',entry_point:'releaseYear'}],artifacts:['src/domain/film-id.ts','src/domain/release-year.ts'],capabilities:['sessions','values','events'],representations:{UUID:'text validated by the existing filmId factory',Int:'lossless safe JavaScript integer'}};},
 async begin(){return {
  async invoke(){return {kind:'unavailable',reason:'The domain adapter offers no operations'};},
  async validate(subject,value){
   const name=subject.startsWith(root)?subject.slice(root.length):'';
   if(!ids.includes(name)&&name!=='ReleaseYear')return {kind:'unavailable',reason:'No existing domain validator for this subject'};
   let input:unknown;
   if(value.kind==='integer'){
    const number=Number(value.value);
    if(!Number.isSafeInteger(number)||BigInt(value.value)!==BigInt(number))return {kind:'unavailable',reason:'Integer has no lossless native representation'};
    input=number;
   }else if(value.kind==='float'){
    const number=Number(value.value);
    if(!Number.isFinite(number))return {kind:'unavailable',reason:'Nonfinite float has no supported native representation'};
    input=number;
   }else if(value.kind==='decimal')return {kind:'unavailable',reason:'No exact native Decimal representation'};
   else input=value.kind==='unit'?undefined:value.value;
   let actual:string|number;
   try{actual=ids.includes(name)?filmId(input):releaseYear(input);}
   catch(error){if(error instanceof TypeError||error instanceof RangeError)return {kind:'rejected',reason:error.message};throw error;}
   const translated:Value=typeof actual==='string'?{kind:'string',value:actual}:{kind:'integer',value:String(actual)};
   return {kind:'accepted',value:translated};
  },
  async close(){},
 };},
};
