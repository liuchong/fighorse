use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

struct StdioServer {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl StdioServer {
    fn start() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_fighorse"))
            .args(["mcp", "serve", "--transport", "stdio"])
            .env("FIGHORSE_MCP_ALLOW_MULTIPLE", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            stdin,
            stdout,
        }
    }

    fn request(&mut self, message: Value) -> Value {
        writeln!(self.stdin, "{}", serde_json::to_string(&message).unwrap()).unwrap();
        self.stdin.flush().unwrap();
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    }
}

impl Drop for StdioServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn standard_stdio_round_trip_initializes_and_lists_tools() {
    let mut server = StdioServer::start();
    let initialized = server.request(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "integration-test", "version": "1"}
        }
    }));
    assert_eq!(initialized["result"]["serverInfo"]["name"], "fighorse");

    writeln!(
        server.stdin,
        "{}",
        json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}})
    )
    .unwrap();
    server.stdin.flush().unwrap();

    let tools = server.request(json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {}
    }));
    assert!(
        tools["result"]["tools"]
            .as_array()
            .is_some_and(|v| !v.is_empty())
    );
}

fn temp_lock_path() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "fighorse-stdio-lock-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&root).unwrap();
    root.join("mcp.lock")
}

#[test]
fn stdio_eof_exits_cleanly_and_releases_the_singleton_lock() {
    let lock_path = temp_lock_path();
    let mut child = Command::new(env!("CARGO_BIN_EXE_fighorse"))
        .args(["mcp", "serve", "--transport", "stdio"])
        .env_remove("FIGHORSE_MCP_ALLOW_MULTIPLE")
        .env("FIGHORSE_MCP_LOCK_FILE", &lock_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    for _ in 0..100 {
        if lock_path.is_file() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(lock_path.is_file(), "stdio server never acquired its lock");

    drop(child.stdin.take());
    let status = child.wait().unwrap();
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    assert!(
        status.success() || (status.code() == Some(1) && stderr.contains("connection closed")),
        "unexpected stdio EOF status: {status:?}, stderr: {stderr}"
    );
    assert!(
        !lock_path.exists(),
        "stdin close must release the singleton lock"
    );
    let _ = fs::remove_dir_all(lock_path.parent().unwrap());
}
