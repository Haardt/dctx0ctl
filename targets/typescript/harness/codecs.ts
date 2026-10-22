// This template is embedded after the selected IR and effective configuration.
type Representation = 'number'|'bigint'|'null'|'undefined'|'string'|'tagged';
type Configuration = {schema_version:number;integer:Representation;option:Representation;enum:Representation;types:Record<string,Representation>};
type Ref = {name:string;symbol_id:string|null;arguments:Ref[]};
type Data = {id:string;stable_id:string|null;keys:string[];alias:Ref|null;fields:{name:string;ty:Ref}[];variants:{name:string;payload:Ref|null}[]};
const integerText = /^-?[0-9]+$/;
const decimalText = /^-?[0-9]+(?:\.[0-9]+)?$/;
const floatText = /^[+-]?(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)(?:[eE][+-]?[0-9]+)?$/;
function invalid(message:string):never { throw new TypeError(`Wire codec: ${message}`); }
function object(value:unknown):Record<string,unknown> {
 if(value===null||typeof value!=='object'||Array.isArray(value))return invalid('Expected a record');
 return value as Record<string,unknown>;
}
function text(value:unknown):string {
 if(typeof value!=='string')return invalid('Expected text');
 return value;
}
function item(ref:Ref):Ref {
 if(ref.arguments.length!==1)return invalid(`Expected one type argument for ${ref.name}`);
 return ref.arguments[0]!;
}
function integer(value:unknown, mode:Representation, decoding:boolean):string|number|bigint {
 if(decoding){
  const encoded=text(value);
  if(encoded.length>65536||!integerText.test(encoded))return invalid('Invalid integer text');
  const exact=BigInt(encoded);
  if(mode==='bigint')return exact;
  const number=Number(exact);
  if(!Number.isSafeInteger(number)||BigInt(number)!==exact)return invalid('Integer is not losslessly representable as number');
  return number;
 }
 if(mode==='bigint'){
  if(typeof value!=='bigint')return invalid('Expected bigint');
  const encoded=String(value);
  return encoded.length<=65536?encoded:invalid('Integer text exceeds 65536 bytes');
 }
 if(typeof value!=='number'||!Number.isSafeInteger(value))return invalid('Expected a safe integer number');
 return String(value);
}
function variantMode(data:Data, override?:Representation):Representation {
 return data.variants.some(v=>v.payload!==null)?'tagged':override??configuration.enum;
}
function primitive(ref:Ref, value:unknown, decoding:boolean, override:Representation|undefined, depth:number):unknown {
 const wire=decoding?object(value):null;
 const kind=wire?.kind;
 const payload=wire?.value;
 const expect=(expected:string)=>{if(decoding&&kind!==expected)invalid(`Expected ${expected} wire value`);};
 switch(ref.name){
  case 'Unit':
   expect('unit');if(!decoding&&value!==undefined)invalid('Expected undefined Unit');
   return decoding?undefined:{kind:'unit'};
  case 'Boolean': {
   expect('boolean');const native=decoding?payload:value;
   if(typeof native!=='boolean')return invalid('Expected boolean');
   return decoding?native:{kind:'boolean',value:native};
  }
  case 'String':case 'UUID':case 'Timestamp': {
   expect('string');const native=text(decoding?payload:value);
   return decoding?native:{kind:'string',value:native};
  }
  case 'Int': {
   expect('integer');const native=integer(decoding?payload:value,override??configuration.integer,decoding);
   return decoding?native:{kind:'integer',value:native};
  }
  case 'Decimal': {
   expect('decimal');const native=text(decoding?payload:value);
   if(native.length>65536||!decimalText.test(native))return invalid('Invalid exact decimal text');
   return decoding?native:{kind:'decimal',value:native};
  }
  case 'Float':case 'Duration': {
   expect('float');const native=decoding?Number(text(payload)):value;
   if(decoding&&!floatText.test(text(payload)))return invalid('Invalid float text');
   if(typeof native!=='number'||!Number.isFinite(native))return invalid('Expected finite Float');
   return decoding?native:{kind:'float',value:String(native)};
  }
  case 'List': {
   expect('list');const entries=decoding?payload:value;
   if(!Array.isArray(entries))return invalid('Expected list');
   const result=entries.map(v=>convert(item(ref),v,decoding,undefined,depth+1));
   return decoding?result:{kind:'list',value:result};
  }
  case 'Option':case 'Dctx0Option': {
   expect('option');const absent=(override??configuration.option)==='undefined'?undefined:null;
   if(decoding){
    if(payload===null)return absent;
    const present=convert(item(ref),payload,true,undefined,depth+1);
    if(present===absent)return invalid('Present option collides with absent representation; adapt this codec');
    return present;
   }
   return {kind:'option',value:value===absent?null:convert(item(ref),value,false,undefined,depth+1)};
  }
  case 'Bytes': {
   expect('list');const entries=decoding?payload:value;
   if(!Array.isArray(entries))return invalid('Expected byte array');
   const result=entries.map(entry=>{
    const native=decoding?primitive({name:'Int',symbol_id:null,arguments:[]},entry,true,'number',depth+1):entry;
    if(typeof native!=='number'||!Number.isInteger(native)||native<0||native>255)return invalid('Expected byte in 0..255');
    return decoding?native:{kind:'integer',value:String(native)};
   });
   return decoding?result:{kind:'list',value:result};
  }
  case 'Some': {
   expect('option');if(decoding&&payload===null)return invalid('Expected present option');
   const native=convert(item(ref),decoding?payload:value,decoding,undefined,depth+1);
   return decoding?native:{kind:'option',value:native};
  }
  case 'Dctx0Result': {
   expect('variant');const tagged=object(decoding?payload:value);
   if(decoding&&tagged.declaration!=='builtin:Dctx0Result')return invalid('Foreign result');
   const name=text(decoding?tagged.name:tagged.kind);
   const index=name==='Success'?0:name==='Failure'?1:invalid('Unknown result alternative');
   if(ref.arguments.length!==2)return invalid('Expected two result type arguments');
   const native=convert(ref.arguments[index]!,tagged.payload,decoding,undefined,depth+1);
   return decoding?{kind:name,payload:native}:{kind:'variant',value:{declaration:'builtin:Dctx0Result',name,payload:native}};
  }
  case 'None':
   expect('option');if(decoding&&payload!==null)invalid('Expected empty option');
   if(!decoding&&value!==null)invalid('Expected null None');
   return decoding?null:{kind:'option',value:null};
  default:return invalid(`Unsupported representation for builtin ${ref.name}; adapt this testing codec explicitly`);
 }
}
function convert(ref:Ref,value:unknown,decoding:boolean,override:Representation|undefined,depth:number):unknown {
 if(depth>128)return invalid('Conversion depth exceeds 128');
 const id=ref.symbol_id;
 if(id===null)return primitive(ref,value,decoding,override,depth);
 const data=declarations[id];
 if(!data)return invalid(`Unknown declaration ${id}`);
 const representation=override??configuration.types[id];
 if(data.alias)return convert(data.alias,value,decoding,representation,depth+1);
 if(data.variants.length){
  const mode=variantMode(data,representation);
  const wire=decoding?object(value):null;
  const tagged=decoding?object(wire?.value):mode==='tagged'?object(value):null;
  if(decoding&&(wire?.kind!=='variant'||tagged?.declaration!==id))return invalid(`Foreign variant for ${id}`);
  const name=decoding?text(tagged?.name):mode==='tagged'?text(tagged?.kind):text(value);
  const variant=data.variants.find(v=>v.name===name);
  if(!variant)return invalid(`Unknown alternative ${id}.${name}`);
  const payload=variant.payload?convert(variant.payload,tagged?.payload,decoding,undefined,depth+1):null;
  if(!variant.payload&&((decoding&&tagged?.payload!==null)||(!decoding&&mode==='tagged'&&tagged?.payload!==null)))return invalid('Payload on payloadless alternative');
  if(decoding)return mode==='string'?name:{kind:name,payload};
  return {kind:'variant',value:{declaration:id,name,payload}};
 }
 const wire=decoding?object(value):null;
 if(decoding&&wire?.kind!=='record')return invalid(`Expected record for ${id}`);
 const record=object(decoding?wire?.value:value);
 const keys=Object.keys(record);
 if(keys.length!==data.fields.length||data.fields.some(f=>!Object.hasOwn(record,f.name)))return invalid(`Record fields differ for ${id}`);
 const converted=Object.fromEntries(data.fields.map(f=>[f.name,convert(f.ty,record[f.name],decoding,undefined,depth+1)]));
 return decoding?converted:{kind:'record',value:converted};
}
function reference(typeName:string):Ref {
 if(!Object.hasOwn(declarations,typeName))return invalid(`Unknown selected type ${typeName}`);
 return {name:typeName,symbol_id:typeName,arguments:[]};
}
export function encode(typeName:string,value:unknown):Value {
 return convert(reference(typeName),value,false,undefined,0) as Value;
}
export function decode(typeName:string,value:Value):unknown {
 return convert(reference(typeName),value,true,undefined,0);
}

/** Decode test inputs without enforcing the selected contract's shape.
 * Transport/representation errors still throw; only the production call can
 * supply evidence that a contract-invalid input was rejected.
 */
export function decodeInput(typeName:string,value:Value):unknown {
 return input(reference(typeName),value,undefined,0);
}
function input(ref:Ref|undefined,value:Value,override:Representation|undefined,depth:number):unknown {
 if(depth>128)return invalid('Conversion depth exceeds 128');
 const data=ref?.symbol_id?declarations[ref.symbol_id]:undefined;
 const representation=override??(ref?.symbol_id?configuration.types[ref.symbol_id]:undefined);
 if(data?.alias)return input(data.alias,value,representation,depth+1);
 switch(value.kind){
  case 'failure':return invalid('Failure values require an explicit native exception mapping');
  case 'unit':return undefined;
  case 'boolean':return value.value;
  case 'string':return value.value;
  case 'integer':return integer(value.value,ref?.name==='Int'?representation??configuration.integer:configuration.integer,true);
  case 'decimal':return primitive({name:'Decimal',symbol_id:null,arguments:[]},value,true,undefined,depth);
  case 'float':return primitive({name:'Float',symbol_id:null,arguments:[]},value,true,undefined,depth);
  case 'list':return value.value.map(v=>input(ref?.name==='List'?ref.arguments[0]:ref?.name==='Bytes'?{name:'Int',symbol_id:null,arguments:[]}:undefined,v,ref?.name==='Bytes'?'number':undefined,depth+1));
  case 'record':return Object.fromEntries(Object.entries(value.value).map(([key,v])=>[key,input(data?.fields.find(f=>f.name===key)?.ty,v,undefined,depth+1)]));
  case 'option': {
   const absence=ref?.name==='None'?null:(representation??configuration.option)==='undefined'?undefined:null;
   if(value.value===null)return absence;
   const present=input(ref?.arguments[0],value.value,undefined,depth+1);
   if(present===absence)return invalid('Present option collapses into absence; adapt the native representation');
   return present;
  }
  case 'variant': {
   const {name,payload}=value.value;
   // A foreign declaration has no lossless counterpart in string/tagged native
   // enums. Reject the conversion instead of discarding transport identity.
   if(data&&value.value.declaration!==data.id)return invalid('Foreign variant declaration');
   if(ref?.name==='Dctx0Result'&&value.value.declaration!=='builtin:Dctx0Result')return invalid('Foreign result declaration');
   const mode=data?variantMode(data,representation):'tagged';
   if(mode==='string'){
    if(payload!==null)return invalid('Cannot preserve payload in string enum representation');
    return name;
   }
   const hint=data?.variants.find(v=>v.name===name)?.payload??(ref?.name==='Dctx0Result'?ref.arguments[name==='Success'?0:name==='Failure'?1:-1]:undefined);
   return {kind:name,payload:payload===null?null:input(hint??undefined,payload,undefined,depth+1)};
  }
 }
}
