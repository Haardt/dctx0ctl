use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{Receiver, channel},
    time::Duration,
};
struct Client {
    child: Child,
    input: ChildStdin,
    messages: Receiver<Value>,
    id: u32,
}
impl Client {
    fn new(root: &std::path::Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_dctx0"))
            .args(["lsp", "--stdio", "--project"])
            .arg(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (sender, messages) = channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut length = None;
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap() == 0 {
                        return;
                    }
                    if line == "\r\n" {
                        break;
                    }
                    if let Some(value) = line.strip_prefix("Content-Length: ") {
                        length = Some(value.trim().parse::<usize>().unwrap());
                    }
                }
                let mut body = vec![0; length.unwrap()];
                reader.read_exact(&mut body).unwrap();
                if sender.send(serde_json::from_slice(&body).unwrap()).is_err() {
                    return;
                }
            }
        });
        Self {
            child,
            input,
            messages,
            id: 0,
        }
    }
    fn send(&mut self, message: Value) {
        let body = message.to_string();
        write!(self.input, "Content-Length: {}\r\n\r\n{}", body.len(), body).unwrap();
        self.input.flush().unwrap();
    }
    fn notify(&mut self, method: &str, params: Value) {
        self.send(json!({"jsonrpc":"2.0","method":method,"params":params}));
    }
    fn request(&mut self, method: &str, params: Value) -> Value {
        self.id += 1;
        let id = self.id;
        self.send(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}));
        loop {
            let message = self.messages.recv_timeout(Duration::from_secs(10)).unwrap();
            if message["id"] == id {
                assert!(message.get("error").is_none(), "{message}");
                return message["result"].clone();
            }
        }
    }
    fn diagnostic(&self, uri: &str, version: i32, has_errors: bool) {
        loop {
            let message = self.messages.recv_timeout(Duration::from_secs(10)).unwrap();
            if message["method"] == "textDocument/publishDiagnostics"
                && message["params"]["uri"] == uri
                && message["params"]["version"] == version
            {
                assert_eq!(
                    !message["params"]["diagnostics"]
                        .as_array()
                        .unwrap()
                        .is_empty(),
                    has_errors
                );
                return;
            }
        }
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
#[test]
fn native_stdio_lsp_handles_unsaved_edits_navigation_colors_and_exit_without_stdin_eof() {
    let root = std::env::temp_dir().join(format!("dctx0-lsp-cli-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("types.dctx0"),
        "workspace W { domain D { scalar Id is UUID } }",
    )
    .unwrap();
    let valid = "fragment D {\r\n surface Api {\r\n intent \"emoji 😀\"\r\n operation echo(input: Id) -> Id {\r\n pure\r\n ensures result == input\r\n }\r\n }\r\n}";
    fs::write(root.join("api.dctx0"), valid).unwrap();
    let uri = format!("file://{}", root.join("api.dctx0").display());
    let mut client = Client::new(&root);
    let init = client.request("initialize", json!({"processId":null,"capabilities":{}}));
    assert_eq!(init["capabilities"]["positionEncoding"], "utf-16");
    client.notify("initialized", json!({}));
    client.notify("textDocument/didOpen",json!({"textDocument":{"uri":uri,"languageId":"dctx0","version":1,"text":valid.replace("input: Id","input: Missing")}}));
    client.diagnostic(&uri, 1, true);
    assert!(
        !client
            .request(
                "textDocument/documentSymbol",
                json!({"textDocument":{"uri":uri}})
            )
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        !client.request(
            "textDocument/semanticTokens/full",
            json!({"textDocument":{"uri":uri}})
        )["data"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    client.notify("textDocument/didChange",json!({"textDocument":{"uri":uri,"version":2},"contentChanges":[{"range":{"start":{"line":3,"character":23},"end":{"line":3,"character":30}},"text":"Id"}]}));
    client.diagnostic(&uri, 2, false);
    let at = json!({"textDocument":{"uri":uri},"position":{"line":3,"character":24}});
    let definition = client.request("textDocument/definition", at.clone());
    assert!(definition["uri"].as_str().unwrap().ends_with("types.dctx0"));
    let refs=client.request("textDocument/references",json!({"textDocument":{"uri":uri},"position":{"line":3,"character":24},"context":{"includeDeclaration":true}}));
    assert_eq!(refs.as_array().unwrap().len(), 3);
    let hover = client.request(
        "textDocument/hover",
        json!({"textDocument":{"uri":uri},"position":{"line":3,"character":12}}),
    );
    assert!(
        hover["contents"]["value"]
            .as_str()
            .unwrap()
            .contains("requirements, not proof")
    );
    assert!(
        client
            .request("textDocument/completion", at)
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["label"] == "Id")
    );
    assert!(
        !client
            .request("workspace/symbol", json!({"query":"echo"}))
            .as_array()
            .unwrap()
            .is_empty()
    );
    client.notify("textDocument/didClose", json!({"textDocument":{"uri":uri}}));
    client.request("shutdown", Value::Null);
    client.notify("exit", Value::Null);
    let start = std::time::Instant::now();
    loop {
        if let Some(status) = client.child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        assert!(
            start.elapsed() < Duration::from_secs(3),
            "Server waited for stdin EOF after exit"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!root.join(".dctx0ctl").exists());
    assert_eq!(fs::read_to_string(root.join("api.dctx0")).unwrap(), valid);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn malformed_stdio_messages_fail_without_emitting_non_protocol_stdout() {
    for input in [
        b"Content-Length: 1\r\n\r\nx".as_slice(),
        b"Content-Length: 4\r\n\r\nnull",
        b"Content-Length: 4194305\r\n\r\n",
        b"Content-Length: 1\r\nContent-Length: 1\r\n\r\nx",
        b"Content-Length: 2\r\n\r\n{",
    ] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_dctx0"))
            .args(["lsp", "--stdio"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(input).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
}
#[test]
fn invalid_utf8_headers_invalid_message_shapes_and_closed_stdout_are_rejected() {
    let bad_shape = br#"{"jsonrpc":"2.0","method":12}"#;
    let shape = format!(
        "Content-Length: {}\r\n\r\n{}",
        bad_shape.len(),
        std::str::from_utf8(bad_shape).unwrap()
    );
    for input in [b"\xff\r\n\r\n".as_slice(), shape.as_bytes()] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_dctx0"))
            .args(["lsp", "--stdio"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(input).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
    let root = std::env::temp_dir().join(format!("dctx0-lsp-closed-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_dctx0"))
        .args(["lsp", "--stdio", "--project"])
        .arg(&root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let mut input = child.stdin.take().unwrap();
    for message in [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}),
        json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
    ] {
        let text = message.to_string();
        write!(input, "Content-Length: {}\r\n\r\n{}", text.len(), text).unwrap();
    }
    drop(input);
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    fs::remove_dir_all(root).unwrap();
}
