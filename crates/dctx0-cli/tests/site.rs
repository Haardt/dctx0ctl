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

#[test]
fn traced_export_reports_historical_work_without_changing_the_payload_or_database() {
    use std::{fs, process::Command};
    let root = std::env::temp_dir().join(format!("dctx0-traced-site-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let source = root.join("spec.dctx0");
    let binary = env!("CARGO_BIN_EXE_dctx0");
    for kind in ["Int", "String", "Boolean"] {
        fs::write(&source, format!("workspace W {{ type A = {kind} }}")).unwrap();
        for verb in [vec!["check"], vec!["tasks", "plan"]] {
            let result = Command::new(binary).args(verb).arg(&root).output().unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
        }
    }
    let database = root.join(".dctx0ctl/state.sqlite3");
    let before = fs::read(&database).unwrap();
    for (name, traced) in [("plain", false), ("traced", true)] {
        let mut command = Command::new(binary);
        command.env_remove("DCTX0_SITE_TRACE");
        if traced {
            command.env("DCTX0_SITE_TRACE", "1");
        }
        let result = command
            .args(["site", "export"])
            .arg(&root)
            .args(["--history", "all", "--progress", "--output"])
            .arg(root.join(name))
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(String::from_utf8_lossy(&result.stdout).starts_with("Exported "));
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(stderr.contains("READY: export written"));
        if traced {
            assert!(stderr.contains("historical snapshots packed ("));
            assert!(stderr.contains("task evidence packed ("));
        }
    }
    let plain = fs::read(root.join("plain/snapshot.json")).unwrap();
    assert_eq!(plain, fs::read(root.join("traced/snapshot.json")).unwrap());
    let view: serde_json::Value = serde_json::from_slice(&plain).unwrap();
    assert!(
        !view["records"]["requirement_snapshots"]
            .as_object()
            .unwrap()
            .is_empty()
    );
    assert_eq!(before, fs::read(database).unwrap());
    fs::remove_dir_all(root).unwrap();
}
