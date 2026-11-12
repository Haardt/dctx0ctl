import {test,expect} from 'bun:test';
import {inputValue,projectValue,BindingError,type ValueModel} from './index.ts';
const int={name:'Int',symbol_id:null,arguments:[]};
const model:ValueModel={Record:{alias:null,variants:[],fields:[{name:'count',ty:int},{name:'label',ty:{name:'Option',symbol_id:null,arguments:[{name:'String',symbol_id:null,arguments:[]}]}}]}};
const record={name:'Record',symbol_id:'Record',arguments:[]};
test('projector preserves actual record fields and rejects unsafe native integers',()=>{
 expect(projectValue({extra:true},record,model)).toEqual({kind:'record',value:{extra:{kind:'boolean',value:true}}});
 expect(()=>projectValue(9007199254740992,int,model)).toThrow('exact integer');
 expect(projectValue(1.5,{name:'Float',symbol_id:null,arguments:[]},model)).toEqual({kind:'float',value:'1.5'});
});
test('typed constructor conversion preserves invalid shapes and exact oversized integers',async()=>{
 const {inputValue}=await import('./values.ts');
 expect(inputValue({kind:'integer',value:'9007199254740993'},int,model)).toBe(9007199254740993n);
 expect(inputValue({kind:'record',value:{extra:{kind:'boolean',value:false}}},record,model)).toEqual({extra:false});
 expect(inputValue({kind:'option',value:null},{name:'Option',symbol_id:null,arguments:[int]},model)).toBeUndefined();
 expect(()=>inputValue({kind:'integer',value:'1.5'},int,model)).toThrow('integer wire');
 expect(()=>inputValue({kind:'decimal',value:'1.25'},int,model)).toThrow('Decimal');
});

test('model codec converts aliases, enum alternatives and nested collections',()=>{
 const schema:ValueModel={...model,Alias:{alias:int,fields:[],variants:[]},Genre:{alias:null,fields:[],variants:[{name:'Drama',payload:null}]}};
 const alias={name:'Alias',symbol_id:'Alias',arguments:[]},genre={name:'Genre',symbol_id:'Genre',arguments:[]};
 expect(inputValue({kind:'list',value:[{kind:'integer',value:'2'}]},{name:'List',symbol_id:null,arguments:[alias]},schema)).toEqual([2]);
 expect(projectValue([2],{name:'List',symbol_id:null,arguments:[alias]},schema)).toEqual({kind:'list',value:[{kind:'integer',value:'2'}]});
 expect(inputValue({kind:'variant',value:{declaration:'Genre',name:'Drama',payload:null}},genre,schema)).toBe('Drama');
 expect(projectValue('Drama',genre,schema)).toEqual({kind:'variant',value:{declaration:'Genre',name:'Drama',payload:null}});
 expect(inputValue({kind:'record',value:{count:{kind:'integer',value:'5'},label:{kind:'option',value:null}}},record,schema)).toEqual({count:5,label:undefined});
 expect(projectValue({count:5,label:undefined},record,schema)).toEqual({kind:'record',value:{count:{kind:'integer',value:'5'},label:{kind:'option',value:null}}});
});
test('unsupported exact representations and foreign enum identities remain visible',()=>{
 const schema:ValueModel={Genre:{alias:null,fields:[],variants:[{name:'Drama',payload:null}]}},genre={name:'Genre',symbol_id:'Genre',arguments:[]};
 expect(()=>inputValue({kind:'variant',value:{declaration:'Other',name:'Drama',payload:null}},genre,schema)).toThrow('Foreign variant');
 expect(()=>inputValue({kind:'variant',value:{declaration:'Genre',name:'Drama',payload:{kind:'unit'}}},genre,schema)).toThrow('Payload variants');
 expect(()=>inputValue({kind:'failure',value:{declaration:'Error',payload:null}},int,model)).toThrow('exception');
 expect(()=>projectValue(null,int,model)).toThrow(BindingError);
 expect(()=>projectValue(NaN,int,model)).toThrow('Nonfinite');
 expect(projectValue('1.25',{name:'Decimal',symbol_id:null,arguments:[]},model)).toEqual({kind:'decimal',value:'1.25'});
});
test('primitive conversion distinguishes numeric and textual production values',()=>{
 expect(inputValue({kind:'unit'},int,model)).toBeUndefined();
 expect(inputValue({kind:'string',value:'text'},int,model)).toBe('text');
 expect(inputValue({kind:'float',value:'1.5'},int,model)).toBe(1.5);
 expect(()=>inputValue({kind:'float',value:'NaN'},int,model)).toThrow('float wire');
 expect(()=>inputValue({kind:'float',value:'1e9999'},int,model)).toThrow('Nonfinite');
 expect(projectValue(Number.MAX_VALUE,{name:'Float',symbol_id:null,arguments:[]},model)).toEqual({kind:'float',value:'1.7976931348623157e+308'});
 expect(projectValue(undefined,int,model)).toEqual({kind:'unit'});
 expect(projectValue('actual',int,model)).toEqual({kind:'string',value:'actual'});
});
test('input witnesses reconstruct changed primitives and nested actual values',async()=>{
 const {projectInput}=await import('./values.ts');
 expect(projectInput(1800,{kind:'integer',value:'1887'},int,model)).toEqual({kind:'integer',value:'1800'});
 const expected={kind:'record',value:{count:{kind:'integer',value:'7'},label:{kind:'option',value:{kind:'string',value:'original'}}}} as import('./protocol.ts').Value;
 expect(projectInput({count:8,label:'changed',extra:false},expected,record,model)).toEqual({kind:'record',value:{count:{kind:'integer',value:'8'},label:{kind:'option',value:{kind:'string',value:'changed'}},extra:{kind:'boolean',value:false}}});
 expect(projectInput([9],{kind:'list',value:[{kind:'integer',value:'7'},{kind:'integer',value:'8'}]},{name:'List',symbol_id:null,arguments:[int]},model)).toEqual({kind:'list',value:[{kind:'integer',value:'9'}]});
});
test('input witnesses preserve only option and enum metadata, not their values',async()=>{
 const {projectInput}=await import('./values.ts');
 expect(projectInput('Changed',{kind:'variant',value:{declaration:'Enum',name:'Original',payload:null}},int,model)).toEqual({kind:'variant',value:{declaration:'Enum',name:'Changed',payload:null}});
 expect(()=>projectInput(undefined,{kind:'option',value:{kind:'integer',value:'5'}},{name:'Option',symbol_id:null,arguments:[int]},model)).toThrow('tagged options required');
 expect(projectInput(6,{kind:'option',value:null},{name:'Option',symbol_id:null,arguments:[int]},model)).toEqual({kind:'option',value:{kind:'integer',value:'6'}});
});
test('float witness retains original text only for the same finite native f64',async()=>{
 const {projectInput}=await import('./values.ts'),float={name:'Float',symbol_id:null,arguments:[]};
 expect(projectInput(Number.MAX_VALUE,{kind:'float',value:'1.7976931348623157e308'},float,model)).toEqual({kind:'float',value:'1.7976931348623157e308'});
 expect(projectInput(1.5,{kind:'float',value:'1.25'},float,model)).toEqual({kind:'float',value:'1.5'});
 expect(()=>projectInput(Infinity,{kind:'float',value:'1'},float,model)).toThrow('Nonfinite');
});
test('unwitnessable classes and accessors cannot become production rejection',async()=>{
 const {projectInput}=await import('./values.ts');
 expect(()=>projectInput(new Date(),{kind:'unit'},int,model)).toThrow('class');
 expect(()=>projectInput({get count(){return 1;}},{kind:'record',value:{}},record,model)).toThrow('property');
 const native=Object.fromEntries([['__proto__',false]]);
 expect(projectInput(native,{kind:'record',value:{}},record,model)).toEqual({kind:'record',value:Object.fromEntries([['__proto__',{kind:'boolean',value:false}]])});
});
test('witness rejects cycles and array accessors without invoking getters',async()=>{
 const {projectInput}=await import('./values.ts');
 const cyclic:Record<string,unknown>={};cyclic.self=cyclic;
 expect(()=>projectInput(cyclic,{kind:'record',value:{}},record,model)).toThrow('Cyclic');
 let invoked=false;const array:unknown[]=[];
 Object.defineProperty(array,'0',{get(){invoked=true;return 7;},enumerable:true});
 expect(()=>projectInput(array,{kind:'list',value:[]},{name:'List',symbol_id:null,arguments:[int]},model)).toThrow('array property');
 expect(invoked).toBe(false);
 const shared={count:5};
 expect(projectInput({left:shared,right:shared},{kind:'record',value:{}},record,model)).toEqual({kind:'record',value:{left:{kind:'record',value:{count:{kind:'integer',value:'5'}}},right:{kind:'record',value:{count:{kind:'integer',value:'5'}}}}});
});
test('Some(Unit) malformed-input annotation is retained only for non-Option targets',async()=>{
 const {projectInput}=await import('./values.ts');
 expect(projectInput(undefined,{kind:'option',value:{kind:'unit'}},int,model)).toEqual({kind:'option',value:{kind:'unit'}});
 expect(projectInput(false,{kind:'option',value:{kind:'unit'}},int,model)).toEqual({kind:'option',value:{kind:'boolean',value:false}});
 expect(()=>projectInput(undefined,{kind:'option',value:{kind:'unit'}},{name:'Option',symbol_id:null,arguments:[{name:'Unit',symbol_id:null,arguments:[]}]},model)).toThrow('tagged options required');
});
test('witness preserves safe native integers and BigInt digits and rejects rounded integer numbers',async()=>{
 const {projectInput}=await import('./values.ts');
 expect(projectInput(1887,{kind:'integer',value:'1887'},int,model)).toEqual({kind:'integer',value:'1887'});
 expect(projectInput(9007199254740993n,{kind:'integer',value:'9007199254740993'},int,model)).toEqual({kind:'integer',value:'9007199254740993'});
 expect(()=>projectInput(Number(9007199254740993n),{kind:'integer',value:'9007199254740993'},int,model)).toThrow('Unsafe native integer');
});
