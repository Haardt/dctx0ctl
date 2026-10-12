// Contributor acceptance: run the shipped client against the real Rust server.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const {pathToFileURL} = require('node:url');
const Module = require('node:module');
const root = fs.mkdtempSync(path.join(os.tmpdir(),'dctx0-vscode-'));
const text = 'workspace W { domain D { scalar Id is UUID\n surface Api { operation echo(input: Id) -> Id { pure\n ensures result == input } } } }';
fs.writeFileSync(path.join(root,'spec.dctx0'),text);
const document = {uri:{scheme:'file',toString:()=>pathToFileURL(path.join(root,'spec.dctx0')).href},languageId:'dctx0',version:1,getText:()=>text};
const providers = {}; const events = {}; let colors = 0; const notices=[];
const disposable = () => ({dispose(){}});
class Shape {constructor(...args){this.args=args;}}
const vscode = {
  window:{createOutputChannel:()=>({append(){},appendLine(){},dispose(){}}),showErrorMessage:m=>notices.push(m)},
  workspace:{workspaceFolders:[{uri:{toString:()=>pathToFileURL(root).href},name:'fixture'}],textDocuments:[document],
    getConfiguration:()=>({get:()=>path.resolve(process.argv[2] || 'target/debug/dctx0')}),
    createFileSystemWatcher:()=>({onDidCreate:()=>disposable(),onDidChange:()=>disposable(),onDidDelete:()=>disposable(),dispose(){}})},
  languages:{createDiagnosticCollection:()=>({set(uri,diagnostics){assert.equal(diagnostics.length,0);colors++;},delete(){},dispose(){}})},
  EventEmitter:class {constructor(){this.event=()=>disposable();}fire(){}dispose(){}},
  Range:Shape,Location:Shape,Diagnostic:Shape,DiagnosticRelatedInformation:Shape,CompletionItem:Shape,SnippetString:Shape,Hover:Shape,MarkdownString:Shape,DocumentSymbol:Shape,SymbolInformation:Shape,SemanticTokens:Shape,SemanticTokensLegend:Shape,
  Uri:{parse:uri=>uri},commands:{registerCommand:()=>disposable()},CancellationError:class extends Error{}
};
for(const name of ['onDidOpenTextDocument','onDidChangeTextDocument','onDidCloseTextDocument','onDidSaveTextDocument'])vscode.workspace[name]=handler=>{events[name]=handler;return disposable();};
for(const [name,key] of [['registerCompletionItemProvider','completion'],['registerHoverProvider','hover'],['registerDefinitionProvider','definition'],['registerReferenceProvider','references'],['registerDocumentSymbolProvider','symbols'],['registerWorkspaceSymbolProvider','workspace'],['registerDocumentSemanticTokensProvider','tokens']])vscode.languages[name]=(...args)=>{providers[key]=args[key==='workspace'?0:1];return disposable();};
const original = Module._load;
Module._load = function(name,...args){return name==='vscode'?vscode:original.call(this,name,...args);};
const extension = require('./extension.js'); Module._load=original;
(async()=>{
  await extension.activate({asAbsolutePath:p=>path.join(root,p),subscriptions:[]});
  assert.deepEqual(notices,[]);
  assert.ok((await providers.symbols.provideDocumentSymbols(document)).length>0);
  const position={line:1,character:37};
  assert.ok((await providers.definition.provideDefinition(document,position)).args[0].endsWith('spec.dctx0'));
  assert.ok((await providers.references.provideReferences(document,position,{includeDeclaration:true})).length>=3);
  assert.ok((await providers.workspace.provideWorkspaceSymbols('echo')).length>0);
  assert.ok((await providers.tokens.provideDocumentSemanticTokens(document)).args[0].length>0);
  assert.ok((await providers.completion.provideCompletionItems(document,{line:0,character:0})).some(i=>i.args[0]==='intent'));
  assert.ok(await providers.hover.provideHover(document,{line:1,character:27}));
  document.version=2;
  events.onDidChangeTextDocument({document,contentChanges:[{text}]});
  await providers.tokens.provideDocumentSemanticTokens(document);
  assert.ok(colors>0);
  console.log(JSON.stringify({status:'PASS',checks:['client initializes native server','diagnostics','symbols','definitions','references','workspace symbols','semantic colors','completion','hover','document changes','shutdown']}));
})().catch(error=>{console.error(error);process.exitCode=1;}).finally(async()=>{await extension.deactivate();fs.rmSync(root,{recursive:true,force:true});});
