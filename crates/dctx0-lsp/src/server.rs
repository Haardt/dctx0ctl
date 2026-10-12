use crate::{
    TOKEN_MODIFIERS, TOKEN_TYPES,
    workspace::{Snapshot, Workspace, file_path},
};
use crossbeam_channel::{RecvTimeoutError, Sender, bounded, select};
use lsp_server::{Connection, Message, Notification, Request, Response};
use lsp_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    InitializeParams,
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Default)]
pub struct ServerOptions {
    pub project: Option<PathBuf>,
}
/// Serve LSP over stdio. Stdout is reserved exclusively for protocol messages.
pub fn serve_stdio(options: ServerOptions) -> Result<(), String> {
    let (connection, writer, errors) = crate::transport::stdio();
    let result = serve(&connection, options);
    drop(connection);
    writer
        .join()
        .map_err(|_| "LSP writer panicked".to_string())??;
    if let Ok(error) = errors.try_recv() {
        return Err(error);
    }
    result
}
struct Session {
    workspace: Workspace,
    generation: u64,
    current: Option<(u64, Snapshot)>,
    pending: Vec<(Request, Instant)>,
    published: BTreeSet<String>,
    shutdown: bool,
    dynamic_watch: bool,
}
fn serve(connection: &Connection, options: ServerOptions) -> Result<(), String> {
    let (id, params) = connection.initialize_start().map_err(|e| e.to_string())?;
    let params: InitializeParams = serde_json::from_value(params).map_err(|e| e.to_string())?;
    let dynamic_watch = params
        .capabilities
        .workspace
        .as_ref()
        .and_then(|w| w.did_change_watched_files.as_ref())
        .is_some_and(|w| w.dynamic_registration == Some(true));
    let roots = roots(options, params)?;
    connection.initialize_finish(id,json!({"capabilities": {
        "positionEncoding":"utf-16", "textDocumentSync":{"openClose":true,"change":2,"save":true},
        "completionProvider":{"triggerCharacters":["."]},"hoverProvider":true,"definitionProvider":true,"referencesProvider":true,
        "documentSymbolProvider":true,"workspaceSymbolProvider":true,
        "semanticTokensProvider":{"legend":{"tokenTypes":TOKEN_TYPES,"tokenModifiers":TOKEN_MODIFIERS},"full":true}
    },"serverInfo":{"name":"dctx0-rust","version":env!("CARGO_PKG_VERSION")}})).map_err(|e| e.to_string())?;
    let (jobs, input) = bounded::<(u64, Workspace)>(1);
    let eviction = input.clone();
    let (results, output) = bounded::<(u64, Snapshot)>(1);
    let worker = std::thread::spawn(move || {
        while let Ok(mut job) = input.recv() {
            // Only one waiting snapshot; coalesce typing bursts instead of retaining history.
            loop {
                match input.recv_timeout(Duration::from_millis(60)) {
                    Ok(new) => job = new,
                    Err(RecvTimeoutError::Timeout) => break,
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }
            let snapshot = job.1.analyze();
            if results.send((job.0, snapshot)).is_err() {
                return;
            }
        }
    });
    let mut session = Session {
        workspace: Workspace {
            roots,
            documents: BTreeMap::new(),
        },
        generation: 1,
        current: None,
        pending: Vec::new(),
        published: BTreeSet::new(),
        shutdown: false,
        dynamic_watch,
    };
    session.schedule(&jobs)?;
    let outcome = (|| -> Result<(), String> {
        loop {
            select! {
                recv(connection.receiver) -> message => {
                    let Ok(message) = message else { break; };
                    match message {
                        Message::Request(request) => session.request(connection,request)?,
                        Message::Notification(notification) => {
                            if notification.method == "exit" {
                                if !session.shutdown { return Err("Client exited without shutdown".into()); }
                                break;
                            }
                            if session.notification(connection,notification)? {
                                let _ = eviction.try_recv();
                                session.schedule(&jobs)?;
                            }
                        }
                        Message::Response(_) => {}
                    }
                }
                recv(output) -> result => {
                    let (version,snapshot) = result.map_err(|_| "Analysis worker stopped")?;
                    session.publish(connection,version,snapshot)?;
                }
                default(Duration::from_millis(100)) => session.expire(connection)?
            }
        }
        Ok(())
    })();
    drop(jobs);
    drop(eviction);
    drop(output);
    worker
        .join()
        .map_err(|_| "Analysis worker panicked".to_string())?;
    outcome
}
fn roots(options: ServerOptions, params: InitializeParams) -> Result<Vec<PathBuf>, String> {
    let mut roots = if let Some(project) = options.project {
        vec![project]
    } else {
        let mut roots: Vec<_> = params
            .workspace_folders
            .unwrap_or_default()
            .iter()
            .filter_map(|f| file_path(f.uri.as_str()))
            .collect();
        #[allow(deprecated)]
        if roots.is_empty() {
            roots.extend(params.root_uri.as_ref().and_then(|u| file_path(u.as_str())));
        }
        if roots.is_empty() {
            roots.push(std::env::current_dir().map_err(|e| e.to_string())?);
        }
        roots
    };
    if roots.len() > 32 {
        return Err("At most 32 editor workspace folders are supported".into());
    }
    roots = roots
        .into_iter()
        .map(|p| std::fs::canonicalize(p).map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    roots.sort();
    roots.dedup();
    Ok(roots)
}
impl Session {
    fn schedule(&self, jobs: &Sender<(u64, Workspace)>) -> Result<(), String> {
        jobs.send((self.generation, self.workspace.clone()))
            .map_err(|e| e.to_string())
    }
    fn request(&mut self, connection: &Connection, request: Request) -> Result<(), String> {
        if request.method == "shutdown" {
            self.shutdown = true;
            for (request, _) in self.pending.drain(..) {
                reject(connection, request, -32800, "Server shutting down")?;
            }
            return send(
                connection,
                Message::Response(Response::new_ok(request.id, Value::Null)),
            );
        }
        if self.shutdown {
            return reject(connection, request, -32600, "Server has shut down");
        }
        if ![
            "textDocument/completion",
            "textDocument/hover",
            "textDocument/definition",
            "textDocument/references",
            "textDocument/documentSymbol",
            "workspace/symbol",
            "textDocument/semanticTokens/full",
        ]
        .contains(&request.method.as_str())
        {
            return reject(connection, request, -32601, "Method not supported");
        }
        if let Some((version, snapshot)) = &self.current {
            if *version == self.generation {
                return respond(connection, request, snapshot);
            }
        }
        if self.pending.len() >= 128 {
            reject(
                connection,
                request,
                -32802,
                "Too many pending editor requests",
            )
        } else {
            self.pending.push((request, Instant::now()));
            Ok(())
        }
    }
    fn notification(
        &mut self,
        connection: &Connection,
        notification: Notification,
    ) -> Result<bool, String> {
        if self.shutdown {
            return Ok(false);
        }
        if notification.method == "initialized" && self.dynamic_watch {
            send(
                connection,
                Message::Request(Request::new(
                    "dctx0-watch".to_string().into(),
                    "client/registerCapability".into(),
                    json!({"registrations":[{"id":"dctx0-spec-files","method":"workspace/didChangeWatchedFiles","registerOptions":{"watchers":[{"globPattern":"**/*.dctx0","kind":7}]}}]}),
                )),
            )?;
        }
        if notification.method == "$/cancelRequest" {
            if let Some(index) = self.pending.iter().position(|(r, _)| {
                serde_json::to_value(&r.id).ok().as_ref() == notification.params.get("id")
            }) {
                let (request, _) = self.pending.remove(index);
                reject(connection, request, -32800, "Request cancelled")?;
            }
            return Ok(false);
        }
        match update(&mut self.workspace, notification) {
            Ok(true) => {
                self.generation += 1;
                Ok(true)
            }
            Ok(false) => Ok(false),
            Err(error) => {
                log(connection, error)?;
                Ok(false)
            }
        }
    }
    fn publish(
        &mut self,
        connection: &Connection,
        version: u64,
        snapshot: Snapshot,
    ) -> Result<(), String> {
        if version != self.generation || self.shutdown {
            return Ok(());
        }
        for warning in &snapshot.warnings {
            log(connection, warning.clone())?;
        }
        let next: BTreeSet<_> = snapshot
            .projects
            .iter()
            .flat_map(|p| p.files().cloned())
            .collect();
        for uri in self.published.union(&next) {
            let diagnostics = snapshot
                .project(uri)
                .map_or(json!([]), |p| p.diagnostics(uri));
            send(
                connection,
                Message::Notification(Notification::new(
                    "textDocument/publishDiagnostics".into(),
                    json!({"uri":uri,"version":snapshot.versions.get(uri),"diagnostics":diagnostics}),
                )),
            )?;
        }
        self.published = next;
        for (request, _) in self.pending.drain(..) {
            respond(connection, request, &snapshot)?;
        }
        self.current = Some((version, snapshot));
        Ok(())
    }
    fn expire(&mut self, connection: &Connection) -> Result<(), String> {
        let mut i = 0;
        while i < self.pending.len() {
            if self.pending[i].1.elapsed() > Duration::from_secs(10) {
                let (request, _) = self.pending.remove(i);
                reject(
                    connection,
                    request,
                    -32802,
                    "Analysis not ready within 10 seconds",
                )?;
            } else {
                i += 1;
            }
        }
        Ok(())
    }
}
fn reject(
    connection: &Connection,
    request: Request,
    code: i32,
    message: &str,
) -> Result<(), String> {
    send(
        connection,
        Message::Response(Response::new_err(request.id, code, message.into())),
    )
}
fn respond(connection: &Connection, request: Request, snapshot: &Snapshot) -> Result<(), String> {
    let response = match snapshot.request(&request.method, &request.params) {
        Ok(value) => Response::new_ok(request.id, value),
        Err(e) => Response::new_err(request.id, -32602, e),
    };
    send(connection, Message::Response(response))
}
fn send(connection: &Connection, message: Message) -> Result<(), String> {
    serde_json::to_writer(&mut MessageSize(0), &message)
        .map_err(|e| format!("Outgoing LSP message exceeds protocol limits: {e}"))?;
    connection.sender.send(message).map_err(|e| e.to_string())
}
struct MessageSize(usize);
impl std::io::Write for MessageSize {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0 += bytes.len();
        if self.0 > 4 * 1024 * 1024 - 32 {
            return Err(std::io::Error::other("LSP response exceeds 4 MiB"));
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn log(connection: &Connection, message: String) -> Result<(), String> {
    send(
        connection,
        Message::Notification(Notification::new(
            "window/logMessage".into(),
            json!({"type":2,"message":message}),
        )),
    )
}
fn update(workspace: &mut Workspace, notification: Notification) -> Result<bool, String> {
    match notification.method.as_str() {
        "textDocument/didOpen" => {
            let p: DidOpenTextDocumentParams =
                serde_json::from_value(notification.params).map_err(|e| e.to_string())?;
            workspace.open(
                p.text_document.uri.to_string(),
                p.text_document.version,
                p.text_document.text,
            )?;
        }
        "textDocument/didChange" => {
            let p: DidChangeTextDocumentParams =
                serde_json::from_value(notification.params).map_err(|e| e.to_string())?;
            workspace.change(
                p.text_document.uri.as_str(),
                p.text_document.version,
                p.content_changes,
            )?;
        }
        "textDocument/didClose" => {
            let p: DidCloseTextDocumentParams =
                serde_json::from_value(notification.params).map_err(|e| e.to_string())?;
            workspace.documents.remove(p.text_document.uri.as_str());
        }
        "workspace/didChangeWatchedFiles" | "textDocument/didSave" => {}
        _ => return Ok(false),
    }
    Ok(true)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn request(id: i32, method: &str) -> Request {
        Request::new(
            id.into(),
            method.into(),
            json!({"textDocument":{"uri":"file:///missing.dctx0"}}),
        )
    }
    #[test]
    fn pending_requests_cancel_expire_and_shutdown_while_stale_results_are_ignored() {
        let (server, client) = Connection::memory();
        let mut session = Session {
            workspace: Workspace {
                roots: vec![],
                documents: BTreeMap::new(),
            },
            generation: 2,
            current: None,
            pending: vec![],
            published: BTreeSet::new(),
            shutdown: false,
            dynamic_watch: true,
        };
        session.request(&server, request(1, "unsupported")).unwrap();
        assert!(
            matches!(client.receiver.recv().unwrap(),Message::Response(r) if r.error.as_ref().unwrap().code == -32601)
        );
        session
            .request(&server, request(2, "textDocument/hover"))
            .unwrap();
        session
            .notification(
                &server,
                Notification::new("$/cancelRequest".into(), json!({"id":2})),
            )
            .unwrap();
        assert!(
            matches!(client.receiver.recv().unwrap(),Message::Response(r) if r.error.as_ref().unwrap().code == -32800)
        );
        session
            .request(&server, request(3, "textDocument/hover"))
            .unwrap();
        session.pending[0].1 = Instant::now() - Duration::from_secs(11);
        session.expire(&server).unwrap();
        assert!(
            matches!(client.receiver.recv().unwrap(),Message::Response(r) if r.error.as_ref().unwrap().code == -32802)
        );
        session
            .notification(&server, Notification::new("initialized".into(), json!({})))
            .unwrap();
        assert!(
            matches!(client.receiver.recv().unwrap(),Message::Request(r) if r.method == "client/registerCapability")
        );
        session
            .publish(&server, 1, session.workspace.analyze())
            .unwrap();
        assert!(session.current.is_none());
        session
            .request(&server, request(4, "textDocument/hover"))
            .unwrap();
        session.request(&server, request(5, "shutdown")).unwrap();
        assert!(
            matches!(client.receiver.recv().unwrap(),Message::Response(r) if r.error.as_ref().unwrap().code == -32800)
        );
        assert!(matches!(client.receiver.recv().unwrap(),Message::Response(r) if r.id == 5.into()));
        session
            .request(&server, request(6, "textDocument/hover"))
            .unwrap();
        assert!(
            matches!(client.receiver.recv().unwrap(),Message::Response(r) if r.error.as_ref().unwrap().code == -32600)
        );
    }
    #[test]
    fn unknown_documents_and_invalid_parameters_do_not_crash_the_service() {
        let snapshot = Workspace {
            roots: vec![],
            documents: BTreeMap::new(),
        }
        .analyze();
        assert_eq!(
            snapshot
                .request(
                    "textDocument/hover",
                    &json!({"textDocument":{"uri":"file:///missing"}})
                )
                .unwrap(),
            Value::Null
        );
        assert!(snapshot.request("textDocument/hover", &json!({})).is_err());
        assert_eq!(
            snapshot
                .request(
                    "textDocument/semanticTokens/full",
                    &json!({"textDocument":{"uri":"file:///missing"}})
                )
                .unwrap(),
            json!({"data":[]})
        );
        assert_eq!(
            snapshot
                .request(
                    "textDocument/references",
                    &json!({"textDocument":{"uri":"file:///missing"}})
                )
                .unwrap(),
            json!([])
        );
    }
}
#[cfg(test)]
mod protocol_tests {
    use super::*;
    use std::fs;
    fn call(client: &Connection, id: i32, method: &str, params: Value) -> Response {
        client
            .sender
            .send(Message::Request(Request::new(
                id.into(),
                method.into(),
                params,
            )))
            .unwrap();
        loop {
            if let Message::Response(r) = client
                .receiver
                .recv_timeout(Duration::from_secs(3))
                .unwrap()
            {
                if r.id == id.into() {
                    return r;
                }
            }
        }
    }
    #[test]
    fn workspace_folder_transport_serves_queries_rejects_edits_and_cancels_pending_work() {
        let root = std::env::temp_dir().join(format!("dctx0-lsp-memory-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("spec.dctx0");
        let uri = crate::workspace::file_uri(&path).unwrap();
        let source = "workspace W { domain D {\n value Id = UUID\n surface Api {\n intent \"*documented*\"\n operation echo(input: Id) -> Id { pure\n ensures result == input } } } }";
        fs::write(&path, source).unwrap();
        let (server, client) = Connection::memory();
        let handle = std::thread::spawn(move || serve(&server, ServerOptions::default()));
        let response = call(
            &client,
            1,
            "initialize",
            json!({"capabilities":{"workspace":{"didChangeWatchedFiles":{"dynamicRegistration":true}}},"workspaceFolders":[{"uri":crate::workspace::file_uri(&root).unwrap(),"name":"spec"}]}),
        );
        assert!(response.error.is_none());
        client
            .sender
            .send(Message::Notification(Notification::new(
                "initialized".into(),
                json!({}),
            )))
            .unwrap();
        for (id, method, params) in [
            (2, "workspace/symbol", json!({"query":"echo"})),
            (
                3,
                "textDocument/documentSymbol",
                json!({"textDocument":{"uri":uri}}),
            ),
            (
                4,
                "textDocument/semanticTokens/full",
                json!({"textDocument":{"uri":uri}}),
            ),
            (
                5,
                "textDocument/completion",
                json!({"textDocument":{"uri":uri},"position":{"line":0,"character":0}}),
            ),
            (
                6,
                "textDocument/hover",
                json!({"textDocument":{"uri":uri},"position":{"line":4,"character":12}}),
            ),
            (
                7,
                "textDocument/definition",
                json!({"textDocument":{"uri":uri},"position":{"line":4,"character":24}}),
            ),
            (
                8,
                "textDocument/references",
                json!({"textDocument":{"uri":uri},"position":{"line":4,"character":24},"context":{"includeDeclaration":true}}),
            ),
        ] {
            let r = call(&client, id, method, params);
            assert!(r.error.is_none(), "{r:?}");
            assert!(!r.result.unwrap().is_null());
        }
        assert_eq!(
            call(&client, 9, "unsupported", json!({}))
                .error
                .unwrap()
                .code,
            -32601
        );
        assert_eq!(
            call(
                &client,
                10,
                "textDocument/hover",
                json!({"textDocument":{"uri":uri}})
            )
            .error
            .unwrap()
            .code,
            -32602
        );
        client
            .sender
            .send(Message::Notification(Notification::new(
                "textDocument/didOpen".into(),
                json!({"textDocument":{"uri":uri,"languageId":"dctx0","version":1,"text":source}}),
            )))
            .unwrap();
        client
            .sender
            .send(Message::Notification(Notification::new(
                "textDocument/didChange".into(),
                json!({"textDocument":{"uri":uri,"version":1},"contentChanges":[]}),
            )))
            .unwrap();
        loop {
            if let Message::Notification(n) = client
                .receiver
                .recv_timeout(Duration::from_secs(3))
                .unwrap()
            {
                if n.method == "window/logMessage" {
                    assert!(n.params["message"].as_str().unwrap().contains("stale"));
                    break;
                }
            }
        }
        client
            .sender
            .send(Message::Notification(Notification::new(
                "textDocument/didChange".into(),
                json!({"textDocument":{"uri":uri,"version":2},"contentChanges":[{"text":source}]}),
            )))
            .unwrap();
        client
            .sender
            .send(Message::Request(Request::new(
                11.into(),
                "textDocument/hover".into(),
                json!({"textDocument":{"uri":uri},"position":{"line":0,"character":1}}),
            )))
            .unwrap();
        client
            .sender
            .send(Message::Notification(Notification::new(
                "$/cancelRequest".into(),
                json!({"id":11}),
            )))
            .unwrap();
        loop {
            if let Message::Response(r) = client
                .receiver
                .recv_timeout(Duration::from_secs(3))
                .unwrap()
            {
                if r.id == 11.into() {
                    assert_eq!(r.error.unwrap().code, -32800);
                    break;
                }
            }
        }
        std::thread::sleep(Duration::from_millis(150));
        assert!(call(&client, 12, "shutdown", Value::Null).error.is_none());
        client
            .sender
            .send(Message::Notification(Notification::new(
                "exit".into(),
                Value::Null,
            )))
            .unwrap();
        assert!(handle.join().unwrap().is_ok());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn initialization_rejects_invalid_parameters_roots_and_disconnected_clients() {
        assert!(
            roots(
                ServerOptions {
                    project: Some(PathBuf::from("/nonexistent/dctx0-root"))
                },
                serde_json::from_value(json!({"capabilities":{}})).unwrap()
            )
            .is_err()
        );
        assert!(
            !roots(
                ServerOptions::default(),
                serde_json::from_value(json!({"capabilities":{}})).unwrap()
            )
            .unwrap()
            .is_empty()
        );
        let root = std::env::temp_dir();
        assert_eq!(
            roots(
                ServerOptions::default(),
                serde_json::from_value(
                    json!({"capabilities":{},"rootUri":crate::workspace::file_uri(&root).unwrap()})
                )
                .unwrap()
            )
            .unwrap(),
            vec![std::fs::canonicalize(root).unwrap()]
        );
        let (server, client) = Connection::memory();
        drop(client);
        assert!(serve(&server, ServerOptions::default()).is_err());
        let (server, client) = Connection::memory();
        client
            .sender
            .send(Message::Request(Request::new(
                1.into(),
                "initialize".into(),
                json!({"capabilities":"bad"}),
            )))
            .unwrap();
        assert!(serve(&server, ServerOptions::default()).is_err());
        let (server, client) = Connection::memory();
        drop(client);
        let session = Session {
            workspace: Workspace {
                roots: vec![],
                documents: BTreeMap::new(),
            },
            generation: 1,
            current: None,
            pending: vec![],
            published: BTreeSet::new(),
            shutdown: false,
            dynamic_watch: false,
        };
        let (jobs, input) = bounded(1);
        drop(input);
        assert!(session.schedule(&jobs).is_err());
        assert!(
            send(
                &server,
                Message::Notification(Notification::new("test".into(), Value::Null))
            )
            .is_err()
        );
    }
    #[test]
    fn outgoing_resource_limits_and_bad_notification_shapes_are_explicit() {
        let (server, _client) = Connection::memory();
        let oversized = Message::Notification(Notification::new(
            "test".into(),
            json!({"text":"x".repeat(4*1024*1024)}),
        ));
        assert!(send(&server, oversized).is_err());
        let mut workspace = Workspace {
            roots: vec![],
            documents: BTreeMap::new(),
        };
        for method in [
            "textDocument/didOpen",
            "textDocument/didChange",
            "textDocument/didClose",
        ] {
            assert!(update(&mut workspace, Notification::new(method.into(), json!({}))).is_err());
        }
    }
}
#[cfg(test)]
mod initialization_tests {
    use super::*;
    #[test]
    fn client_disconnect_after_initialize_response_returns_a_technical_error() {
        let (server, client) = Connection::memory();
        let handle = std::thread::spawn(move || serve(&server, ServerOptions::default()));
        client
            .sender
            .send(Message::Request(Request::new(
                1.into(),
                "initialize".into(),
                json!({"capabilities":{}}),
            )))
            .unwrap();
        assert!(matches!(
            client
                .receiver
                .recv_timeout(Duration::from_secs(3))
                .unwrap(),
            Message::Response(_)
        ));
        drop(client);
        assert!(handle.join().unwrap().is_err());
    }
}
