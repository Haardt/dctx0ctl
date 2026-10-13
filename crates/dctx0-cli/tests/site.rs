//! Actual CLI server lifecycle and HTTP boundary, using a real isolated history.
#[cfg(unix)]
#[test]
fn site_cli_exports_serves_etags_and_shuts_down_without_writing_history() {
    use std::{
        fs,
        io::{BufRead, BufReader, Read, Write},
        net::TcpStream,
        process::{Command, Stdio},
    };
    let root = std::env::temp_dir().join(format!("dctx0-site-cli-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("spec.dctx0"), "workspace W { type A = Int }").unwrap();
    let binary = env!("CARGO_BIN_EXE_dctx0");
    let db = root.join("custom.db");
    let invoke = |args: &[&str]| {
        Command::new(binary)
            .arg("--db")
            .arg(&db)
            .args(args)
            .output()
            .unwrap()
    };
    assert!(invoke(&["check", root.to_str().unwrap()]).status.success());
    let before = fs::read(&db).unwrap();
    let out = root.join("output");
    assert!(
        invoke(&[
            "site",
            "export",
            root.to_str().unwrap(),
            "--output",
            out.to_str().unwrap()
        ])
        .status
        .success()
    );
    assert!(
        !invoke(&[
            "site",
            "export",
            root.to_str().unwrap(),
            "--output",
            out.to_str().unwrap(),
            "--snapshot",
            "bad"
        ])
        .status
        .success()
    );
    let mut child = Command::new(binary)
        .arg("--db")
        .arg(&db)
        .args(["site", "serve"])
        .arg(&root)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut reader = BufReader::new(child.stdout.take().unwrap());
    let mut url = String::new();
    reader.read_line(&mut url).unwrap();
    let address = url.trim().strip_prefix("http://").unwrap();
    assert!(address.starts_with("127.0.0.1:"));
    let request = |path: &str, extra: &str, method: &str| {
        let mut socket = TcpStream::connect(address).unwrap();
        socket
            .set_read_timeout(Some(std::time::Duration::from_secs(10)))
            .unwrap();
        write!(
            socket,
            "{method} {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n{extra}\r\n"
        )
        .unwrap();
        let mut response = String::new();
        socket.read_to_string(&mut response).unwrap();
        response
    };
    let json = request("/api/site", "", "GET");
    assert!(json.starts_with("HTTP/1.1 200"));
    let etag = json
        .lines()
        .find_map(|line| line.strip_prefix("etag: "))
        .unwrap();
    assert!(
        request("/api/site", &format!("If-None-Match: {etag}\r\n"), "GET")
            .starts_with("HTTP/1.1 304")
    );
    assert!(request("/", "", "GET").contains("window.__SITE_LIVE__=true"));
    assert!(request("/api/site", "", "POST").starts_with("HTTP/1.1 405"));
    assert!(request("/other", "", "GET").starts_with("HTTP/1.1 404"));
    assert_eq!(before, fs::read(&db).unwrap());
    assert!(
        invoke(&["tasks", "plan", root.to_str().unwrap()])
            .status
            .success()
    );
    let refreshed = request("/api/site", &format!("If-None-Match: {etag}\r\n"), "GET");
    assert!(refreshed.starts_with("HTTP/1.1 200"));
    assert!(refreshed.contains("\"event_seq\""));
    let before = fs::read(&db).unwrap();

    assert!(
        Command::new("kill")
            .args(["-INT", &child.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    assert!(child.wait().unwrap().success());
    assert_eq!(before, fs::read(&db).unwrap());
    fs::remove_dir_all(root).unwrap();
}
