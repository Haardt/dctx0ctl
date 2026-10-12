// Small dependency-free LSP client. VS Code owns its Node runtime; the server is Rust.
const vscode = require('vscode');
const { spawn } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');
const tokenTypes = ['namespace','type','class','interface','method','event','keyword','comment','string','number','operator','variable'];
const modifiers = ['declaration','defaultLibrary'];
let active;
class Client {
  constructor(context) {
    this.context = context; this.pending = new Map(); this.nextId = 1; this.buffer = Buffer.alloc(0); this.disposables = []; this.stopped = false;
    this.output = vscode.window.createOutputChannel('DCTX0');
    this.diagnostics = vscode.languages.createDiagnosticCollection('dctx0');
    this.semanticChanged = new vscode.EventEmitter();
    this.disposables.push(this.output, this.diagnostics, this.semanticChanged);
  }
  async start() {
    const configured = vscode.workspace.getConfiguration('dctx0').get('serverPath', '');
    const binary = configured || this.context.asAbsolutePath(`server/dctx0${process.platform === 'win32' ? '.exe' : ''}`);
    if (!configured && process.platform !== 'win32') fs.chmodSync(binary, 0o755);
    this.child = spawn(binary, ['lsp','--stdio'], {stdio:['pipe','pipe','pipe']});
    this.child.stdin.on('error', error => this.fail(error));
    this.child.stdout.on('error', error => this.fail(error));
    this.child.stdout.on('data', data => this.consume(data));
    this.child.stderr.on('data', data => this.output.append(data.toString()));
    this.child.on('error', error => { this.fail(error); vscode.window.showErrorMessage(`DCTX0: ${error.message}`); });
    this.child.on('exit', code => { this.fail(new Error(`DCTX0 server exited (${code})`)); });
    const folders = (vscode.workspace.workspaceFolders || []).map(f => ({uri:f.uri.toString(),name:f.name}));
    await this.request('initialize', {processId:process.pid, rootUri:folders[0]?.uri || null, workspaceFolders:folders,
      capabilities:{general:{positionEncodings:['utf-16']},textDocument:{completion:{completionItem:{snippetSupport:true}},semanticTokens:{requests:{full:true},tokenTypes,tokenModifiers:modifiers,formats:['relative']}}}});
    this.notify('initialized', {});
    this.ready = true;
    const selector = {language:'dctx0',scheme:'file'};
    const uri = d => ({uri:d.uri.toString()});
    const at = (d,p) => ({textDocument:uri(d),position:p});
    const request = (method,params,token) => this.request(method,params,token);
    this.disposables.push(
      vscode.workspace.onDidOpenTextDocument(d => this.open(d)),
      vscode.workspace.onDidChangeTextDocument(e => {
        if (e.document.languageId !== 'dctx0' || e.document.uri.scheme !== 'file') return;
        this.notify('textDocument/didChange',{textDocument:{...uri(e.document),version:e.document.version},contentChanges:e.contentChanges.map(c => ({range:c.range,text:c.text}))});
      }),
      vscode.workspace.onDidCloseTextDocument(d => { if(d.languageId==='dctx0') {this.notify('textDocument/didClose',{textDocument:uri(d)});this.diagnostics.delete(d.uri);} }),
      vscode.workspace.onDidSaveTextDocument(d => { if(d.languageId==='dctx0') this.notify('textDocument/didSave',{textDocument:uri(d)}); }),
      vscode.languages.registerCompletionItemProvider(selector,{provideCompletionItems:async(d,p,t) => {
        const result = await request('textDocument/completion',at(d,p),t);
        return (result || []).map(i => {const item = new vscode.CompletionItem(i.label, i.kind === undefined ? undefined : i.kind-1);item.detail=i.detail;item.documentation=i.documentation;item.insertText=i.insertTextFormat===2?new vscode.SnippetString(i.insertText):i.insertText;return item;});
      }},'.'),
      vscode.languages.registerHoverProvider(selector,{provideHover:async(d,p,t) => {
        const result = await request('textDocument/hover',at(d,p),t);
        return result ? new vscode.Hover(new vscode.MarkdownString(result.contents.value),range(result.range)) : undefined;
      }}),
      vscode.languages.registerDefinitionProvider(selector,{provideDefinition:async(d,p,t) => {
        const result = await request('textDocument/definition',at(d,p),t); return result ? location(result) : undefined;
      }}),
      vscode.languages.registerReferenceProvider(selector,{provideReferences:async(d,p,c,t) => (await request('textDocument/references',{...at(d,p),context:c},t) || []).map(location)}),
      vscode.languages.registerDocumentSymbolProvider(selector,{provideDocumentSymbols:async(d,t) => (await request('textDocument/documentSymbol',{textDocument:uri(d)},t) || []).map(symbol)}),
      vscode.languages.registerWorkspaceSymbolProvider({provideWorkspaceSymbols:async(query,t) => (await request('workspace/symbol',{query},t) || []).map(s => new vscode.SymbolInformation(s.name,s.kind-1,s.containerName || '',location(s.location)))}),
      vscode.languages.registerDocumentSemanticTokensProvider(selector,{
        onDidChangeSemanticTokens:this.semanticChanged.event,
        provideDocumentSemanticTokens:async(d,t) => new vscode.SemanticTokens(new Uint32Array((await request('textDocument/semanticTokens/full',{textDocument:uri(d)},t)).data))
      },new vscode.SemanticTokensLegend(tokenTypes,modifiers))
    );
    const watcher = vscode.workspace.createFileSystemWatcher('**/*.dctx0');
    for (const [event,type] of [['onDidCreate',1],['onDidChange',2],['onDidDelete',3]]) this.disposables.push(watcher[event](u => this.notify('workspace/didChangeWatchedFiles',{changes:[{uri:u.toString(),type}]})));
    this.disposables.push(watcher);
    for (const document of vscode.workspace.textDocuments) this.open(document);
  }
  open(d) {if(d.languageId==='dctx0' && d.uri.scheme==='file') this.notify('textDocument/didOpen',{textDocument:{uri:d.uri.toString(),languageId:'dctx0',version:d.version,text:d.getText()}});}
  write(message) {
    if (!this.child || this.stopped || this.child.stdin.destroyed) throw new Error('DCTX0 server is unavailable');
    const body = Buffer.from(JSON.stringify({jsonrpc:'2.0',...message}));
    this.child.stdin.write(Buffer.concat([Buffer.from(`Content-Length: ${body.length}\r\n\r\n`),body]));
  }
  notify(method,params) {try {this.write({method,params});} catch(error) {this.output.appendLine(error.message);} }
  request(method,params,token) {
    if(token?.isCancellationRequested) return Promise.reject(new vscode.CancellationError());
    if(this.pending.size>=128) return Promise.reject(new Error("Too many pending DCTX0 requests"));
    const id=this.nextId++;
    return new Promise((resolve,reject) => {
      const timer=setTimeout(() => {this.pending.delete(id);cancel?.dispose();this.notify('$/cancelRequest',{id});reject(new Error(`DCTX0 request timed out: ${method}`));},15000);
      let cancel;
      const finish=(error,value) => {clearTimeout(timer);cancel?.dispose();error?reject(error):resolve(value);};
      this.pending.set(id,{finish});
      cancel=token?.onCancellationRequested(() => {this.pending.delete(id);this.notify('$/cancelRequest',{id});finish(new vscode.CancellationError());});
      try {this.write({id,method,params});} catch(error) {this.pending.delete(id);finish(error);}
    });
  }
  consume(data) {
    this.buffer=Buffer.concat([this.buffer,data]);
    if(this.buffer.length>4*1024*1024+8192) {this.fail(new Error('DCTX0 protocol buffer exceeded'));this.child.kill();return;}
    while(true) {
      const end=this.buffer.indexOf('\r\n\r\n');if(end<0) return;
      const match=/^Content-Length:\s*(\d+)$/mi.exec(this.buffer.subarray(0,end).toString());
      const length=match && Number(match[1]);
      if(length===null || !Number.isSafeInteger(length) || length<0 || length>4*1024*1024) {this.fail(new Error('Invalid DCTX0 protocol header'));this.child.kill();return;}
      if(this.buffer.length<end+4+length) return;
      const body=this.buffer.subarray(end+4,end+4+length);this.buffer=this.buffer.subarray(end+4+length);
      let message;try {message=JSON.parse(body.toString());} catch(error) {this.fail(error);this.child.kill();return;}
      if(message.id!==undefined && !message.method) {const pending=this.pending.get(message.id);this.pending.delete(message.id);pending?.finish(message.error?new Error(message.error.message):undefined,message.result);}
      else if(message.method==='textDocument/publishDiagnostics') {
        const p=message.params;const doc=vscode.workspace.textDocuments.find(d=>d.uri.toString()===p.uri);
        if(doc && p.version!=null && p.version!==doc.version) continue;
        this.diagnostics.set(vscode.Uri.parse(p.uri),p.diagnostics.map(d=>{const item=new vscode.Diagnostic(range(d.range),d.message,d.severity?d.severity-1:0);item.source=d.source;item.code=d.code;if(d.relatedInformation)item.relatedInformation=d.relatedInformation.map(r=>new vscode.DiagnosticRelatedInformation(location(r.location),r.message));return item;}));
        this.semanticChanged.fire();
      } else if(message.method==='window/logMessage') this.output.appendLine(message.params.message);
      else if(message.id!==undefined) {this.write({id:message.id,error:{code:-32601,message:'Client method not supported'}});}
    }
  }
  fail(error) {for(const p of this.pending.values())p.finish(error);this.pending.clear();if(!this.stopped)this.output.appendLine(error.message);}
  async dispose() {
    if(this.stopped) return;
    if(this.ready) {try {await this.request('shutdown',null);this.notify('exit',null);} catch {} }
    this.stopped=true;this.child?.stdin.end();
    const child=this.child;
    const timer=setTimeout(()=>child?.kill(),1000);timer.unref();
    child?.once('exit',()=>clearTimeout(timer));
    this.fail(new Error('DCTX0 client stopped'));for(const d of this.disposables)d.dispose();
  }
}
function range(r){return new vscode.Range(r.start.line,r.start.character,r.end.line,r.end.character);}
function location(l){return new vscode.Location(vscode.Uri.parse(l.uri),range(l.range));}
function symbol(s){const result=new vscode.DocumentSymbol(s.name,s.detail || '',s.kind-1,range(s.range),range(s.selectionRange));result.children=(s.children||[]).map(symbol);return result;}
async function activate(context) {
  async function restart(){await active?.dispose();active=new Client(context);try{await active.start();}catch(error){await active.dispose();vscode.window.showErrorMessage(`DCTX0: ${error.message}`);}}
  context.subscriptions.push(vscode.commands.registerCommand('dctx0.restartServer',restart),{dispose(){void active?.dispose();}});
  await restart();
}
async function deactivate(){await active?.dispose();active=undefined;}
module.exports={activate,deactivate};
