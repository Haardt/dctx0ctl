import {expect,test} from 'bun:test';
import {assertCatalog,collectFeatures,emptyFeatures,languageCatalog} from './check-features.ts';

const span={file:'example.dctx0',start:{offset:0},end:{offset:18}};
const node=(kind:string,syntax:unknown=null)=>({kind,syntax,terms:[],children:[],span});
test('narrative words and generated values do not count as executable language coverage',()=>{
  const ast=[{statements:[node('intent',{tag:'narrative',text:'round implies Decimal calls emits'})]}];
  const seen=collectFeatures(ast,{examples:[],values:[{kind:'call',name:'round'}]},{'example.dctx0':'intent "round"'});
  expect(seen.functions.size).toBe(0);expect(seen.operators.size).toBe(0);expect(seen.builtin_types.size).toBe(0);
  expect(seen.expression_kinds.size).toBe(0);
});
test('typed expressions and actual type syntax provide independent feature witnesses',()=>{
  const ast=[{statements:[node('value',{tag:'data',alias:{name:'Decimal',arguments:[],span}})]}];
  const seen=collectFeatures(ast,{examples:[],expression:{ty:{name:'Decimal'},span,value:{kind:'call',name:'round',target:null,arguments:[]}}},{'example.dctx0':'value Amount'});
  expect(seen.functions.has('round')).toBe(true);expect(seen.builtin_types.has('Decimal')).toBe(true);
  expect(seen.expression_kinds.has('call')).toBe(true);
});
test('failure and emitted keywords require actual typed expectations',()=>{
  const entry={...node('expect'),terms:[{kind:'identifier',text:'fails',span},{kind:'identifier',text:'emits',span}]};
  expect(collectFeatures([{statements:[entry]}],{examples:[]},{'example.dctx0':'expect false'}).public_statement_keywords.has('fails')).toBe(false);
  const seen=collectFeatures([{statements:[entry]}],{examples:[{expectations:[{kind:'failure'},{kind:'emitted'}]}]},{'example.dctx0':'expect false'});
  expect(seen.public_statement_keywords.has('fails')).toBe(true);expect(seen.public_statement_keywords.has('emits')).toBe(true);
});
test('normalized assertion nodes still require the actual assert source spelling',()=>{
  const ast=[{statements:[node('assertion')]}];
  expect(collectFeatures(ast,{examples:[]},{'example.dctx0':'assert true'}).public_statement_keywords.has('assert')).toBe(true);
  expect(collectFeatures(ast,{examples:[]},{'example.dctx0':'"assert true"'}).public_statement_keywords.has('assert')).toBe(false);
});
test('new or removed registry features cannot silently pass the committed inventory',async()=>{
  const catalog=await languageCatalog();assertCatalog(catalog,catalog);
  const drift={...catalog,functions:[...catalog.functions,'inventedFunction']};
  expect(()=>assertCatalog(catalog,drift)).toThrow('Catalog drift');
  expect(emptyFeatures().operators.size).toBe(0);
});
