//! Actual captured CLI and a bounded stdio LSP client, with owned-child cleanup.
use super::*;
use dctx0_engine::{NativeCancellation, NativeCommand, NativeLimits};
use dctx0_target_sdk::ExecutionRecipe;
use std::{
    io::{BufRead, BufReader, Read, Write},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc,
    time::Duration,
};
fn executable() -> Result<PathBuf, String> {
    std::env::current_dir()
        .map_err(error)
        .map(|p| p.join("executables/dctx0"))
}
fn runtime() -> Result<tokio::runtime::Runtime, String> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(error)
}
impl Actual {
    fn invoke_cli(&self, arguments: Vec<String>) -> Result<Value, String> {
        if arguments.len() > 32 || arguments.iter().any(|a| a.len() > 65536) {
            return Err("CLI argument budget exceeded".into());
        }
        let recipe = ExecutionRecipe {
            program: executable()?.to_string_lossy().into_owned(),
            arguments,
        };
        let audit = runtime()?.block_on(async {
            let mut command = NativeCommand::spawn(
                &recipe,
                &self.root,
                NativeLimits {
                    run: Duration::from_secs(30),
                    log_bytes: 1_048_576,
                    ..Default::default()
                },
            )
            .await
            .map_err(error)?;
            command
                .wait_status(&NativeCancellation::default())
                .await
                .map_err(error)
        })?;
        Ok(
            json!({"exitCode":audit.exit_code.ok_or("CLI terminated by signal")?,"stdout":audit.stdout,"stderr":audit.stderr}),
        )
    }
    pub(super) fn command(&mut self, subject: &str, args: &[Value]) -> Result<Value, String> {
        match subject.rsplit('.').next().ok_or("Missing operation")? {
            "invoke" => self.invoke_cli(strings(&args[0])?),
            "jsonCheck" => self.invoke_cli(vec!["check".into(), "spec".into(), "--json".into()]),
            "localPackages" => {
                let skills = self.invoke_cli(vec![
                    "skills".into(),
                    "install".into(),
                    "--project".into(),
                    ".".into(),
                    "--agent".into(),
                    "agents".into(),
                    "--json".into(),
                ])?;
                let skill_path = self.root.join(".agents/skills/dctx0/SKILL.md");
                let custom = "local skill edit";
                fs::write(&skill_path, custom).map_err(error)?;
                let repeated = self.invoke_cli(vec![
                    "skills".into(),
                    "install".into(),
                    "--project".into(),
                    ".".into(),
                    "--agent".into(),
                    "agents".into(),
                    "--json".into(),
                ])?;
                let editor = self.invoke_cli(vec![
                    "editor".into(),
                    "install".into(),
                    "vscode".into(),
                    "--output".into(),
                    "editor".into(),
                    "--json".into(),
                ])?;
                let zed = self.invoke_cli(vec![
                    "editor".into(),
                    "install".into(),
                    "zed".into(),
                    "--output".into(),
                    "zed".into(),
                    "--json".into(),
                ])?;
                fs::write(self.root.join("editor/keep.txt"), "keep").map_err(error)?;
                let _ = self.invoke_cli(vec![
                    "editor".into(),
                    "install".into(),
                    "vscode".into(),
                    "--output".into(),
                    "editor".into(),
                    "--json".into(),
                ])?;
                Ok(
                    json!({"skillsExit":skills["exitCode"],"repeatExit":repeated["exitCode"],"editorExit":editor["exitCode"],"zedExit":zed["exitCode"],"skillPreserved":fs::read_to_string(skill_path).map_err(error)?==custom,"editorPreserved":fs::read_to_string(self.root.join("editor/keep.txt")).map_err(error)?=="keep","jsonValid":serde_json::from_str::<Value>(skills["stdout"].as_str().ok_or("Missing JSON")?).is_ok()}),
                )
            }
            _ => Err("Unknown distribution operation".into()),
        }
    }
    pub(super) fn editors(&mut self, subject: &str, args: &[Value]) -> Result<Value, String> {
        if subject.ends_with(".version") {
            return Ok(json!(self.lsp.as_ref().and_then(|lsp| lsp.last_version)));
        }
        if subject.ends_with(".shutdown") {
            if let Some(mut lsp) = self.lsp.take() {
                lsp.close()?;
            }
            return Ok(Value::Null);
        }
        if self.lsp.is_none() {
            self.lsp = Some(Lsp::start(&self.root)?);
        }
        let lsp = self.lsp.as_mut().ok_or("Missing LSP")?;
        match subject.rsplit('.').next().ok_or("Missing operation")? {
            "diagnostics" => {
                let uri = args[0].as_str().ok_or("Missing URI")?;
                let uri = lsp.uri(uri)?;
                let version = int(&args[1])?;
                let text = args[2].as_str().ok_or("Missing text")?;
                let diagnostics = lsp.document(&uri, version, text)?;
                Ok(json!(diagnostics["params"]["diagnostics"].as_array().ok_or("Missing diagnostics")?.iter().map(|d|json!({"code":d["code"].as_str().unwrap_or(""),"message":d["message"],"line":d["range"]["start"]["line"],"column":d["range"]["start"]["character"]})).collect::<Vec<_>>()))
            }
            "semanticTokens" => {
                let uri = lsp.uri(args[0].as_str().ok_or("Missing URI")?)?;
                let reply = lsp.request(
                    "textDocument/semanticTokens/full",
                    json!({"textDocument":{"uri":uri}}),
                )?;
                let data = reply["result"]["data"]
                    .as_array()
                    .ok_or("Missing semantic token data")?;
                let mut line = 0;
                let mut column = 0;
                let mut tokens = Vec::new();
                for token in data.chunks_exact(5) {
                    let delta = token[0].as_u64().ok_or("Invalid token line")?;
                    line += delta;
                    if delta > 0 {
                        column = 0;
                    }
                    column += token[1].as_u64().ok_or("Invalid token column")?;
                    tokens.push(
                        json!({"line":line,"column":column,"length":token[2],"tokenType":token[3]}),
                    );
                }
                Ok(json!(tokens))
            }
            "version" => Ok(json!(lsp.last_version)),
            "shutdown" => {
                lsp.close()?;
                self.lsp = None;
                Ok(Value::Null)
            }
            _ => Err("Unknown editor operation".into()),
        }
    }
}
pub(super) struct Lsp {
    child: Child,
    input: Option<ChildStdin>,
    messages: mpsc::Receiver<Result<Value, String>>,
    reader: Option<std::thread::JoinHandle<()>>,
    logs: Option<std::thread::JoinHandle<()>>,
    id: i64,
    root: PathBuf,
    opened: std::collections::BTreeSet<String>,
    last_version: Option<i64>,
    closed: bool,
}
impl Lsp {
    fn start(root: &std::path::Path) -> Result<Self, String> {
        let mut child = Command::new(executable()?)
            .args(["lsp", "--stdio", "--project"])
            .arg(root.join("spec"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(error)?;
        let input = child.stdin.take();
        let stdout = child.stdout.take().ok_or("No LSP output")?;
        let stderr = child.stderr.take().ok_or("No LSP stderr")?;
        let (send, messages) = mpsc::sync_channel(128);
        let reader = std::thread::spawn(move || {
            let mut read = BufReader::new(stdout);
            loop {
                let packet = (|| -> Result<Value, String> {
                    let mut length = None;
                    let mut header_bytes = 0;
                    loop {
                        let mut line = String::new();
                        if read.read_line(&mut line).map_err(error)? == 0 {
                            return Err("LSP output closed".into());
                        }
                        header_bytes += line.len();
                        if header_bytes > 8192 {
                            return Err("LSP header budget exceeded".into());
                        }
                        if line == "\r\n" {
                            break;
                        }
                        if let Some(text) = line.strip_prefix("Content-Length: ") {
                            length = Some(text.trim().parse::<usize>().map_err(error)?);
                        }
                    }
                    let length = length
                        .filter(|n| *n <= 1_048_576)
                        .ok_or("Invalid LSP message length")?;
                    let mut body = vec![0; length];
                    read.read_exact(&mut body).map_err(error)?;
                    serde_json::from_slice(&body).map_err(error)
                })();
                let stop = packet.is_err();
                if send.send(packet).is_err() || stop {
                    break;
                }
            }
        });
        let logs = std::thread::spawn(move || {
            let _ = std::io::copy(&mut stderr.take(1_048_577), &mut std::io::sink());
        });
        let mut current = Self {
            child,
            input,
            messages,
            reader: Some(reader),
            logs: Some(logs),
            id: 0,
            root: root.into(),
            opened: Default::default(),
            last_version: None,
            closed: false,
        };
        let reply=current.request("initialize",json!({"processId":std::process::id(),"rootUri":format!("file://{}",root.join("spec").display()),"capabilities":{}}))?;
        if reply.get("error").is_some() {
            return Err(format!("LSP initialization failed: {reply}"));
        }
        current.send(json!({"jsonrpc":"2.0","method":"initialized","params":{}}))?;
        Ok(current)
    }
    fn uri(&self, input: &str) -> Result<String, String> {
        // Reviewed fixture URIs are project-relative. Canonical aliases retain their spelling.
        if input.is_empty() || input.contains('\\') || input.starts_with('/') || input.contains(':')
        {
            return Err("Expected a relative fixture document".into());
        }
        if input.split('/').any(|part| part == "..") {
            return Err("Document escapes fixture".into());
        }
        Ok(format!(
            "file://{}",
            self.root.join("spec").join(input).display()
        ))
    }
    fn send(&mut self, packet: Value) -> Result<(), String> {
        let bytes = serde_json::to_vec(&packet).map_err(error)?;
        if bytes.len() > 1_048_576 {
            return Err("LSP input budget exceeded".into());
        }
        let input = self.input.as_mut().ok_or("LSP closed")?;
        write!(input, "Content-Length: {}\r\n\r\n", bytes.len()).map_err(error)?;
        input.write_all(&bytes).map_err(error)?;
        input.flush().map_err(error)
    }
    fn receive(&mut self) -> Result<Value, String> {
        self.messages
            .recv_timeout(Duration::from_secs(10))
            .map_err(error)?
    }
    fn request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        self.id += 1;
        let id = self.id;
        self.send(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))?;
        loop {
            let reply = self.receive()?;
            if reply["id"] == id {
                return Ok(reply);
            }
        }
    }
    fn document(&mut self, uri: &str, version: i64, text: &str) -> Result<Value, String> {
        let version = i32::try_from(version).map_err(error)?;
        if self.opened.insert(uri.into()) {
            self.send(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":uri,"languageId":"dctx0","version":version,"text":text}}}))?;
        } else {
            self.send(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":uri,"version":version},"contentChanges":[{"text":text}]}}))?;
        }
        loop {
            let reply = self.receive()?;
            if reply["method"] == "textDocument/publishDiagnostics"
                && reply["params"]["uri"] == uri
                && reply["params"]["version"] == version
            {
                self.last_version = Some(version.into());
                return Ok(reply);
            }
        }
    }
    pub(super) fn close(&mut self) -> Result<(), String> {
        if self.closed {
            return Ok(());
        }
        let reply = self.request("shutdown", Value::Null);
        if reply.is_ok() {
            let _ = self.send(json!({"jsonrpc":"2.0","method":"exit"}));
        }
        self.input.take();
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            if self.child.try_wait().map_err(error)?.is_some() {
                break;
            }
            if std::time::Instant::now() >= deadline {
                self.child.kill().map_err(error)?;
                self.child.wait().map_err(error)?;
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        self.closed = true;
        while self.messages.try_recv().is_ok() {}
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        if let Some(logs) = self.logs.take() {
            let _ = logs.join();
        }
        reply.map(|_| ())
    }
}
impl Drop for Lsp {
    fn drop(&mut self) {
        if !self.closed {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        self.input.take();
    }
}
