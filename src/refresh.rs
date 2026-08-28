use std::io::Write;
use std::process::{Command, Stdio};

use serde::Deserialize;

/// Claude Code's public OAuth client id; the refresh grant is bound to it.
const CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";

/// Anthropic moved token exchange from `console` to `platform`; older installs
/// still answer on the former, so try both before giving up.
const ENDPOINTS: [&str; 2] = [
    "https://platform.claude.com/v1/oauth/token",
    "https://console.anthropic.com/v1/oauth/token",
];

/// Overrides `ENDPOINTS`, so tests can point the grant at a local server.
const ENDPOINT_ENV: &str = "CLAUDE_DESKTOP_CRED_TOKEN_URL";

/// Claude Desktop kills a `mid-session-refresh` helper after 20s.
const TIMEOUT_SECS: &str = "15";

/// Cloudflare fronts the token endpoint and answers generic clients (a bare
/// `curl` or `Mozilla/5.0` agent) with a 429, so identify ourselves.
const USER_AGENT: &str = concat!("claude-desktop-cred/", env!("CARGO_PKG_VERSION"));

#[derive(Debug)]
pub struct Refreshed {
    pub access_token: String,
    pub refresh_token: String,
    /// Epoch milliseconds.
    pub expires_at: Option<i64>,
}

#[derive(Debug)]
pub enum RefreshError {
    Transport(String),
    Rejected { status: String, body: String },
    Malformed(String),
}

impl std::fmt::Display for RefreshError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transport(cause) => write!(f, "could not reach the token endpoint: {cause}"),
            Self::Rejected { status, body } => {
                write!(f, "the token endpoint returned {status}: {body}")
            }
            Self::Malformed(cause) => write!(f, "unexpected token endpoint response: {cause}"),
        }
    }
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
}

struct Response {
    status: String,
    body: String,
}

pub fn refresh(refresh_token: &str, now_ms: i64) -> Result<Refreshed, RefreshError> {
    let request = serde_json::json!({
        "grant_type": "refresh_token",
        "refresh_token": refresh_token,
        "client_id": CLIENT_ID,
    })
    .to_string();

    let mut last = None;
    for endpoint in endpoints() {
        match post(&endpoint, &request) {
            Ok(response) if response.status == "404" => {
                last = Some(RefreshError::Rejected {
                    status: response.status,
                    body: response.body,
                });
            }
            Ok(response) => return interpret(&response, refresh_token, now_ms),
            Err(error) => last = Some(error),
        }
    }
    Err(last.unwrap_or_else(|| RefreshError::Transport("no token endpoint configured".to_owned())))
}

fn endpoints() -> Vec<String> {
    match std::env::var(ENDPOINT_ENV) {
        Ok(url) if !url.is_empty() => vec![url],
        _ => ENDPOINTS.map(str::to_owned).to_vec(),
    }
}

fn post(url: &str, body: &str) -> Result<Response, RefreshError> {
    let transport = |cause: String| RefreshError::Transport(cause);
    let mut child = Command::new("curl")
        .args([
            "--silent",
            "--show-error",
            "--max-time",
            TIMEOUT_SECS,
            "--user-agent",
            USER_AGENT,
            "--request",
            "POST",
            "--header",
            "Content-Type: application/json",
            "--header",
            "Accept: application/json",
            // The grant carries the refresh token, so it goes over stdin
            // rather than argv, which is world-readable via `ps`.
            "--data-binary",
            "@-",
            "--write-out",
            "\n%{http_code}",
            url,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| transport(format!("failed to run `curl`: {error}")))?;

    {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| transport("failed to write the request body".to_owned()))?;
        stdin
            .write_all(body.as_bytes())
            .map_err(|error| transport(format!("failed to write the request body: {error}")))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|error| transport(format!("`curl` failed: {error}")))?;
    if !output.status.success() {
        return Err(transport(format!(
            "`curl` failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    split_response(&String::from_utf8_lossy(&output.stdout))
        .ok_or_else(|| transport("`curl` produced no status code".to_owned()))
}

fn split_response(stdout: &str) -> Option<Response> {
    let (body, status) = stdout.rsplit_once('\n')?;
    Some(Response {
        status: status.trim().to_owned(),
        body: body.to_owned(),
    })
}

fn interpret(
    response: &Response,
    previous_refresh_token: &str,
    now_ms: i64,
) -> Result<Refreshed, RefreshError> {
    if response.status != "200" {
        return Err(RefreshError::Rejected {
            status: response.status.clone(),
            body: summarize(&response.body),
        });
    }
    let token: TokenResponse = serde_json::from_str(&response.body)
        .map_err(|error| RefreshError::Malformed(error.to_string()))?;
    Ok(Refreshed {
        access_token: token.access_token,
        refresh_token: token
            .refresh_token
            .unwrap_or_else(|| previous_refresh_token.to_owned()),
        expires_at: token.expires_in.map(|seconds| now_ms + seconds * 1000),
    })
}

fn summarize(body: &str) -> String {
    let body = body.trim();
    match body.char_indices().nth(200) {
        Some((cut, _)) => format!("{}…", &body[..cut]),
        None => body.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(status: &str, body: &str) -> Response {
        Response {
            status: status.to_owned(),
            body: body.to_owned(),
        }
    }

    #[test]
    fn interprets_a_rotated_pair() {
        let body =
            r#"{"access_token":"new-access","refresh_token":"new-refresh","expires_in":3600}"#;
        let refreshed = interpret(&response("200", body), "old-refresh", 1_000).unwrap();
        assert_eq!(refreshed.access_token, "new-access");
        assert_eq!(refreshed.refresh_token, "new-refresh");
        assert_eq!(refreshed.expires_at, Some(3_601_000));
    }

    #[test]
    fn keeps_the_previous_refresh_token_when_absent() {
        let refreshed = interpret(
            &response("200", r#"{"access_token":"new-access"}"#),
            "old-refresh",
            1_000,
        )
        .unwrap();
        assert_eq!(refreshed.refresh_token, "old-refresh");
        assert_eq!(refreshed.expires_at, None);
    }

    #[test]
    fn reports_a_rejected_grant() {
        let error = interpret(&response("400", r#"{"error":"invalid_grant"}"#), "old", 0);
        assert!(matches!(error, Err(RefreshError::Rejected { .. })));
    }

    #[test]
    fn reports_a_malformed_success() {
        let error = interpret(&response("200", "not json"), "old", 0);
        assert!(matches!(error, Err(RefreshError::Malformed(_))));
    }

    #[test]
    fn splits_body_from_status() {
        let parsed = split_response("{\"a\":1}\n200").unwrap();
        assert_eq!(parsed.status, "200");
        assert_eq!(parsed.body, "{\"a\":1}");
    }
}
