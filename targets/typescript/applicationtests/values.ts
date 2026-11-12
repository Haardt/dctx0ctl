import {BindingError,type TypeRef,type Value} from './protocol.ts';
export type DataDeclaration={alias:TypeRef|null;fields:{name:string;ty:TypeRef}[];variants:{name:string;payload:TypeRef|null}[]};
export type ValueModel=Record<string,DataDeclaration>;
function resolved(model:ValueModel,ty:TypeRef,depth=0):TypeRef {
 if(depth>128)throw new BindingError('Type alias depth exceeded');
 const alias=ty.symbol_id?model[ty.symbol_id]?.alias:undefined;
 return alias?resolved(model,alias,depth+1):ty;
}
/** Project actual values after production runs; numeric wire text is exact. */
export function projectValue(value:unknown,ty:TypeRef,model:ValueModel,depth=0):Value {
 if(depth>128)throw new BindingError('Value projection depth exceeded');
 const ref=resolved(model,ty);
 if((ref.name==='Option'||ref.name==='Dctx0Option')&&ref.arguments[0])return {kind:'option',value:value===undefined?null:projectValue(value,ref.arguments[0],model,depth+1)};
 if(value===undefined)return {kind:'unit'};
 if(typeof value==='boolean')return {kind:'boolean',value};
 if(typeof value==='bigint')return {kind:'integer',value:String(value)};
 if(typeof value==='number'){
  if(!Number.isFinite(value))throw new BindingError('Nonfinite production value');
  if(ref.name==='Float'||ref.name==='Duration')return {kind:'float',value:String(value)};
  if(!Number.isSafeInteger(value))throw new BindingError('Native number cannot encode an exact integer');
  return {kind:'integer',value:String(value)};
 }
 const data=ref.symbol_id?model[ref.symbol_id]:undefined;
 if(typeof value==='string'){
  if(data?.variants.length){
   if(data.variants.some(v=>v.payload!==null))throw new BindingError('Payload variants need an explicit projector');
   return {kind:'variant',value:{declaration:ref.symbol_id!,name:value,payload:null}};
  }
  if(ref.name==='Decimal')return {kind:'decimal',value};
  return {kind:'string',value};
 }
 if(Array.isArray(value))return {kind:'list',value:value.map(item=>projectValue(item,ref.name==='List'&&ref.arguments[0]?ref.arguments[0]:{name:'Unknown',symbol_id:null,arguments:[]},model,depth+1))};
 if(value!==null&&typeof value==='object')return {kind:'record',value:Object.fromEntries(Object.entries(value).map(([key,item])=>[key,projectValue(item,data?.fields.find(field=>field.name===key)?.ty??{name:'Unknown',symbol_id:null,arguments:[]},model,depth+1)]))};
 throw new BindingError('Unsupported production representation');
}
/** Decode centrally typed input without validating its business contract.
 * Malformed shapes remain actual native values for production to reject.
 */
export function inputValue(value:Value,ty:TypeRef,model:ValueModel,depth=0):unknown {
 if(depth>128)throw new BindingError('Input conversion depth exceeded');
 const ref=resolved(model,ty);
 const unknown={name:'Unknown',symbol_id:null,arguments:[]};
 const data=ref.symbol_id?model[ref.symbol_id]:undefined;
 switch(value.kind){
  case 'unit':return undefined;
  case 'boolean':case 'string':return value.value;
  case 'integer':{
   if(!/^-?\d+$/.test(value.value))throw new BindingError('Invalid exact integer wire text');
   const exact=BigInt(value.value),number=Number(exact);
   return Number.isSafeInteger(number)?number:exact;
  }
  case 'float':{
   if(!/^[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?$/.test(value.value))throw new BindingError('Invalid float wire text');
   const number=Number(value.value);if(!Number.isFinite(number))throw new BindingError('Nonfinite float wire text');return number;
  }
  case 'decimal':throw new BindingError('Cinema has no exact Decimal application representation');
  case 'list':return value.value.map(item=>inputValue(item,ref.name==='List'?ref.arguments[0]??unknown:unknown,model,depth+1));
  case 'record':return Object.fromEntries(Object.entries(value.value).map(([key,item])=>[key,inputValue(item,data?.fields.find(field=>field.name===key)?.ty??unknown,model,depth+1)]));
  case 'option':return value.value===null?undefined:inputValue(value.value,ref.arguments[0]??unknown,model,depth+1);
  case 'variant':{
   if(data&&value.value.declaration!==ref.symbol_id)throw new BindingError('Foreign variant has no lossless native enum representation');
   if(value.value.payload!==null)throw new BindingError('Payload variants need an explicit native representation');
   return value.value.name;
  }
  case 'failure':throw new BindingError('Failure input needs an explicit native exception representation');
 }
}
/** Witness the actual native input before production is invoked. Expected wire
 * shape supplies only representation metadata (option/enum/numeric category),
 * never a replacement native value. Unsupported native classes are incomplete.
 */
export function projectInput(native:unknown,expected:Value|undefined,ty:TypeRef,model:ValueModel,depth=0,ancestors=new WeakSet<object>()):Value {
 if(depth>128)throw new BindingError('Input witness depth exceeded');
 const ref=resolved(model,ty),unknown={name:'Unknown',symbol_id:null,arguments:[]};
 const data=ref.symbol_id?model[ref.symbol_id]:undefined;
 if(expected?.kind==='option'){
  const optionTarget=ref.name==='Option'||ref.name==='Dctx0Option';
  if(expected.value!==null&&native===undefined){
   if(optionTarget)throw new BindingError('Present option collapses into absent native representation; tagged options required');
   // Existing malformed-input boundary for non-Option constructors: Some(Unit)
   // and None both map to undefined. Retain only the requested form annotation;
   // this is not evidence that the native constructor distinguishes these forms.
   return {kind:'option',value:{kind:'unit'}};
  }
  return {kind:'option',value:native===undefined?null:projectInput(native,expected.value??undefined,ref.arguments[0]??unknown,model,depth+1,ancestors)};
 }
 if(expected?.kind==='variant'&&typeof native==='string'){
  if(expected.value.payload!==null)throw new BindingError('Payload variant input needs an explicit witness');
  return {kind:'variant',value:{declaration:expected.value.declaration,name:native,payload:null}};
 }
 if(native===undefined)return {kind:'unit'};
 if(typeof native==='boolean')return {kind:'boolean',value:native};
 if(typeof native==='string')return {kind:'string',value:native};
 if(typeof native==='bigint')return {kind:'integer',value:String(native)};
 if(typeof native==='number'){
  if(!Number.isFinite(native))throw new BindingError('Nonfinite native input cannot be witnessed');
  if(expected?.kind==='float'){
   // Preserve the exact original spelling only when it denotes this same f64.
   // Changed values must be reconstructed from the actual native number.
   const same=Object.is(native,Number(expected.value));
   return {kind:'float',value:same?expected.value:String(native)};
  }
  if(Number.isSafeInteger(native))return {kind:'integer',value:String(native)};
  if(Number.isInteger(native))throw new BindingError('Unsafe native integer cannot be witnessed exactly');
  return {kind:'float',value:String(native)};
 }
 if(native!==null&&typeof native==='object'){
  if(ancestors.has(native))throw new BindingError('Cyclic native input cannot be witnessed');
  ancestors.add(native);
  try{
   if(Array.isArray(native)){
    if(Object.getPrototypeOf(native)!==Array.prototype||Reflect.ownKeys(native).length!==native.length+1)throw new BindingError('Native array needs an explicit witness');
    const values:Value[]=[];
    for(let index=0;index<native.length;index++){
     const descriptor=Object.getOwnPropertyDescriptor(native,String(index));
     if(!descriptor?.enumerable||!Object.hasOwn(descriptor,'value'))throw new BindingError('Native array property needs an explicit witness');
     values.push(projectInput(descriptor.value,expected?.kind==='list'?expected.value[index]:undefined,ref.name==='List'?ref.arguments[0]??unknown:unknown,model,depth+1,ancestors));
    }
    return {kind:'list',value:values};
   }
   const prototype=Object.getPrototypeOf(native);
   if(prototype!==Object.prototype&&prototype!==null)throw new BindingError('Native input class needs an explicit witness');
   const values:Record<string,Value>=Object.create(null);
   for(const key of Reflect.ownKeys(native)){
    if(typeof key!=='string')throw new BindingError('Symbol input field cannot be witnessed');
    const descriptor=Object.getOwnPropertyDescriptor(native,key)!;
    if(!descriptor.enumerable||!Object.hasOwn(descriptor,'value'))throw new BindingError('Native input property needs an explicit witness');
    values[key]=projectInput(descriptor.value,expected?.kind==='record'?expected.value[key]:undefined,data?.fields.find(field=>field.name===key)?.ty??unknown,model,depth+1,ancestors);
   }
   return {kind:'record',value:values};
  }finally{ancestors.delete(native);}
 }
 throw new BindingError('Native input representation needs an explicit witness');
}
