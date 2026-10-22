import {afterAll,expect,test} from 'bun:test';
const started=performance.now();
afterAll(async()=>{
 await Bun.write('native-performance.json',JSON.stringify({elapsed_ms:performance.now()-started,peak_rss_bytes:process.platform==='linux'?process.resourceUsage().maxRSS*1024:null,memory_scope:process.platform==='linux'?'Bun self maxRSS; coverage-enabled conformance process, no children':'Peak RSS conversion unavailable on this platform',bun_version:Bun.version,platform:process.platform,architecture:process.arch}));
});
import {encode,decode,decodeInput} from './codecs.ts';
import * as configured from './configured.ts';
import type {Value} from './contract.ts';
const int=(value:string):Value=>({kind:'integer',value});
const string=(value:string):Value=>({kind:'string',value});
const option=(value:Value|null):Value=>({kind:'option',value});
const variant=(declaration:string,name:string,payload:Value|null):Value=>({kind:'variant',value:{declaration,name,payload}});
const pairs:[string,unknown,Value][]=[
 ['W.Id',42,int('42')],['W.Alias',-3,int('-3')],['W.Unit',undefined,{kind:'unit'}],
 ['W.Boolean',true,{kind:'boolean',value:true}],['W.String','hello',string('hello')],
 ['W.UUID','9a243dda-2662-4e65-8aae-b3a34c07603d',string('9a243dda-2662-4e65-8aae-b3a34c07603d')],
 ['W.Timestamp','2026-10-22T10:00:00Z',string('2026-10-22T10:00:00Z')],
 ['W.Decimal','12345678901234567890.123456789',{kind:'decimal',value:'12345678901234567890.123456789'}],
 ['W.Float',1.25,{kind:'float',value:'1.25'}],['W.Duration',2.5,{kind:'float',value:'2.5'}],
 ['W.Bytes',[0,255],{kind:'list',value:[int('0'),int('255')]}],
 ['W.Maybe',null,option(null)],['W.Maybe','present',option(string('present'))],
 ['W.List',[1,2],{kind:'list',value:[int('1'),int('2')]}],
 ['W.Status','Open',variant('W.Status','Open',null)],
 ['W.Choice',{kind:'Count',payload:2},variant('W.Choice','Count',int('2'))],
 ['W.Choice',{kind:'Empty',payload:null},variant('W.Choice','Empty',null)],
 ['W.Packet',{id:42,status:'Open',maybe:null},{kind:'record',value:{id:int('42'),status:variant('W.Status','Open',null),maybe:option(null)}}],
 ['W.Tree',{children:[{children:[]}]},{kind:'record',value:{children:{kind:'list',value:[{kind:'record',value:{children:{kind:'list',value:[]}}}]}}}],
 ['W.Some','x',option(string('x'))],['W.None',null,option(null)],
 ['W.Dctx0Option',null,option(null)],['W.Dctx0Option','x',option(string('x'))],
 ['W.Result',{kind:'Success',payload:'x'},variant('builtin:Dctx0Result','Success',string('x'))],
 ['W.Result',{kind:'Failure',payload:2},variant('builtin:Dctx0Result','Failure',int('2'))],
];
for(const [type,native,wire] of pairs)test(`independent wire pair ${type} ${JSON.stringify(wire)}`,()=>{
 expect(encode(type,native)).toEqual(wire);
 expect(decode(type,wire)).toEqual(native);
});
test('explicit defaults and type exceptions preserve native representations',()=>{
 expect(configured.decode('W.Sequence',int('9007199254740993'))).toBe(9007199254740993n);
 expect(configured.encode('W.Sequence',9007199254740993n)).toEqual(int('9007199254740993'));
 expect(configured.decode('W.Id',int('42'))).toBe(42);
 expect(configured.decode('W.Alias',int('42'))).toBe(42n);
 expect(configured.encode('W.Alias',42n)).toEqual(int('42'));
 expect(configured.decode('W.Maybe',option(null))).toBeNull();
 expect(configured.decode('W.Dctx0Option',option(null))).toBeUndefined();
 expect(configured.encode('W.Dctx0Option',undefined)).toEqual(option(null));
 expect(configured.decode('W.Status',variant('W.Status','Closed',null))).toBe('Closed');
 expect(configured.decode('W.Choice',variant('W.Choice','Count',int('4')))).toEqual({kind:'Count',payload:4n});
 expect(()=>configured.encode('W.Sequence',3)).toThrow('bigint');
});
const invalidEncodings:[string,unknown][]=[
 ['W.Id',9007199254740992],['W.Id',1.5],['W.Id',3n],['W.Float',Infinity],['W.Float','1'],
 ['W.Decimal','NaN'],['W.Decimal','1e2'],['W.Decimal','+1'],['W.Decimal','.5'],['W.Decimal','1.'],['W.Decimal','1'.repeat(65537)],['W.Decimal',1.1],['W.Unit',null],['W.Boolean',1],['W.String',4],
 ['W.List',{}],['W.Packet',{}],['W.Packet',{id:1,status:'Open',maybe:null,extra:1}],
 ['W.Status','Missing'],['W.Choice',{kind:'Empty',payload:'extra'}],['W.Choice',null],
 ['W.Bytes',[256]],['W.Bytes',[-1]],['W.Bytes',[1.5]],['W.None','x'],['Missing',3],
 ['W.Result',{kind:'Other',payload:null}],
];
for(const [type,value] of invalidEncodings)test(`reject invalid native ${type} ${String(value).slice(0,80)}`,()=>expect(()=>encode(type,value)).toThrow());
const invalidDecodings:[string,unknown][]=[
 ['W.Id',int('9007199254740993')],['W.Id',int('1.5')],['W.Id',int('x')],
 ['W.Id',string('1')],['W.Id',null],['W.Float',{kind:'float',value:''}],['W.Float',{kind:'float',value:'Infinity'}],
 ['W.Float',{kind:'float',value:'1e999'}],['W.Boolean',{kind:'boolean',value:1}],
 ['W.Packet',{kind:'record',value:[]}],['W.Packet',{kind:'record',value:{}}],
 ['W.Status',variant('Foreign.Status','Open',null)],['W.Status',variant('W.Status','Open',string('extra'))],
 ['W.Status',variant('W.Status','Missing',null)],['W.Choice',variant('W.Choice','Count',null)],
 ['W.Nested',option(option(null))],['W.Some',option(null)],['W.None',option(string('x'))],
 ['W.Bytes',{kind:'list',value:[int('256')]}],['W.List',{kind:'list',value:{}}],
 ['W.Result',variant('foreign','Success',string('x'))],
];
for(const [type,value] of invalidDecodings)test(`reject invalid wire ${type} ${JSON.stringify(value)}`,()=>expect(()=>decode(type,value as Value)).toThrow());
test('bounded recursion fails explicitly instead of exhausting the stack',()=>{
 let tree:unknown={children:[]};
 for(let i=0;i<130;i++)tree={children:[tree]};
 expect(()=>encode('W.Tree',tree)).toThrow('depth');
});

test('wire numeric grammar agrees with the central integer and decimal contracts',()=>{
 expect(decode('W.Id',int('0001'))).toBe(1);
 expect(decode('W.Id',int('-0'))).toBe(0);
 expect(decode('W.Decimal',{kind:'decimal',value:'001.50'})).toBe('001.50');
 expect(decode('W.Float',{kind:'float',value:'1e2'})).toBe(100);
 expect(()=>decode('W.Decimal',{kind:'decimal',value:'1e2'})).toThrow();
 expect(()=>decode('W.Id',int('1'.repeat(65537)))).toThrow();
 expect(()=>configured.encode('W.Sequence',10n**65536n)).toThrow('65536');
});

test('global tagged enums apply when no per-type exception is selected',()=>{
 const original=configured.configuration.types['W.Status']!;
 delete configured.configuration.types['W.Status'];
 try {
  expect(configured.encode('W.Status',{kind:'Open',payload:null})).toEqual(variant('W.Status','Open',null));
  expect(configured.decode('W.Status',variant('W.Status','Closed',null))).toEqual({kind:'Closed',payload:null});
 } finally { configured.configuration.types['W.Status']=original; }
});

 test('invalid contract inputs reach the real factory boundary',()=>{
  const received:unknown[]=[];
  const factory=(value:unknown)=>{received.push(value);if(typeof value!=='number')throw new Error('production rejection');return value;};
  const input=decodeInput('W.Id',string('wrong type'));
  expect(()=>factory(input)).toThrow('production rejection');
  expect(received).toEqual(['wrong type']);
  // A broken validator must remain observable as accepting the invalid input.
  expect(((value:unknown)=>value)(input)).toBe('wrong type');
  expect(()=>decode('W.Id',string('wrong type'))).toThrow();
 });
 test('input conversion preserves missing/extra fields and unknown enum names',()=>{
  expect(decodeInput('W.Packet',{kind:'record',value:{status:variant('W.Status','Missing',null),extra:string('kept')}})).toEqual({status:'Missing',extra:'kept'});
  expect(configured.decodeInput('W.Alias',int('9007199254740993'))).toBe(9007199254740993n);
  expect(configured.decodeInput('W.Dctx0Option',option(null))).toBeUndefined();
  expect(decodeInput('W.List',{kind:'list',value:[string('wrong item'),int('2')]})).toEqual(['wrong item',2]);
  expect(decodeInput('W.Choice',variant('W.Choice','Unknown',string('kept')))).toEqual({kind:'Unknown',payload:'kept'});
  expect(()=>decodeInput('W.Id',int('9007199254740993'))).toThrow('losslessly');
  expect(()=>decodeInput('W.Status',variant('Foreign','Open',null))).toThrow('Foreign');
  expect(()=>decodeInput('W.Status',variant('W.Status','Open',string('extra')))).toThrow('payload');
  expect(()=>decodeInput('W.Nested',option(option(null)))).toThrow('collapses');
  expect(()=>decodeInput('W.Id',{kind:'failure',value:{declaration:'F',payload:null}})).toThrow('exception');
 });
