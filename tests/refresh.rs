//! End-to-end coverage of the refresh grant against a local token endpoint.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;

const STORE: &str = r#"{
  "claudeAiOauth": {
    "accessToken": "sk-ant-oat01-stale",
    "refreshToken": "sk-ant-ort01-stored",
    "expiresAt": 1000,
    "subscriptionType": "max"
  }
}"#;

struct Endpoint {
    url: String,
    request: std::sync::mpsc::Receiver<String>,
}

/// Serve exactly one request with `response`, handing back its body.
fn serve(status: &str, response: &'static str) -> Endpoint {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/v1/oauth/token", listener.local_addr().unwrap());
    let (sender, request) = std::sync::mpsc::channel();
    let status = status.to_owned();

    std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let body = read_request(&stream);
        let mut stream = stream;
        write!(
            stream,
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
            response.len()
        )
        .unwrap();
        stream.flush().unwrap();
        sender.send(body).unwrap();
    });

    Endpoint { url, request }
}

fn read_request(stream: &TcpStream) -> String {
    let mut reader = BufReader::new(stream);
    let mut length = 0;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        if line.trim().is_empty() {
            break;
        }
        if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
            length = value.trim().parse().unwrap();
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).unwrap();
    String::from_utf8(body).unwrap()
}

fn store_dir(name: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!("cdc-e2e-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join(".credentials.json"), STORE).unwrap();
    directory
}

fn run(directory: &Path, endpoint: &str) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_claude-desktop-cred"));
    // The real store: `$CLAUDE_CONFIG_DIR/.credentials.json`.
    command.env("CLAUDE_CONFIG_DIR", directory);
    // macOS keeps credentials in the Keychain, which cannot be seeded here,
    // so only there the binary is pointed at the same file instead.
    #[cfg(target_os = "macos")]
    command.env(
        "CLAUDE_DESKTOP_CRED_STORE_FILE",
        directory.join(".credentials.json"),
    );
    command
        .env("CLAUDE_DESKTOP_CRED_TOKEN_URL", endpoint)
        .env("CLAUDE_HELPER_CONTEXT", "mid-session-refresh")
        .output()
        .unwrap()
}

#[test]
fn refreshes_an_expired_token_and_persists_the_new_pair() {
    let endpoint = serve(
        "200 OK",
        r#"{"access_token":"sk-ant-oat01-fresh","refresh_token":"sk-ant-ort01-rotated","expires_in":3600}"#,
    );
    let directory = store_dir("ok");

    let output = run(&directory, &endpoint.url);
    assert!(output.status.success(), "{output:?}");

    let printed: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout is contract JSON");
    assert_eq!(printed["token"], "sk-ant-oat01-fresh");

    let sent: serde_json::Value = serde_json::from_str(&endpoint.request.recv().unwrap()).unwrap();
    assert_eq!(sent["grant_type"], "refresh_token");
    assert_eq!(sent["refresh_token"], "sk-ant-ort01-stored");
    assert!(sent["client_id"].is_string());

    let stored: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(directory.join(".credentials.json")).unwrap(),
    )
    .unwrap();
    let section = &stored["claudeAiOauth"];
    assert_eq!(section["accessToken"], "sk-ant-oat01-fresh");
    assert_eq!(section["refreshToken"], "sk-ant-ort01-rotated");
    assert!(section["expiresAt"].as_i64().unwrap() > 1000);
    assert_eq!(section["subscriptionType"], "max");

    std::fs::remove_dir_all(&directory).unwrap();
}

#[test]
fn a_rejected_grant_fails_without_touching_the_store() {
    let endpoint = serve("400 Bad Request", r#"{"error":"invalid_grant"}"#);
    let directory = store_dir("rejected");

    let output = run(&directory, &endpoint.url);
    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "stdout must stay empty on failure"
    );
    assert!(output.stderr.is_empty(), "silent contexts must not print");
    assert_eq!(
        std::fs::read_to_string(directory.join(".credentials.json")).unwrap(),
        STORE
    );

    std::fs::remove_dir_all(&directory).unwrap();
}
