use crate::error::{Result, TraceHurlError};
use crate::render::render_report_html;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::Command;
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayRequest {
    pub entry_id: Option<String>,
    pub curl_cmd: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReplayResult {
    pub success: bool,
    pub status_code: u16,
    pub duration_ms: u64,
    pub body: String,
    pub headers_raw: String,
    pub error: Option<String>,
}

pub fn execute_live_curl(curl_cmd: &str) -> ReplayResult {
    let start = Instant::now();
    let trimmed = curl_cmd.trim();

    if !trimmed.starts_with("curl") {
        return ReplayResult {
            success: false,
            status_code: 0,
            duration_ms: 0,
            body: String::new(),
            headers_raw: String::new(),
            error: Some("Command must start with 'curl'".to_string()),
        };
    }

    let output = match Command::new("sh")
        .arg("-c")
        .arg(format!(
            "{} -s -i -w \"\\n__TH_STATUS__:%{{http_code}}\"",
            trimmed
        ))
        .output()
    {
        Ok(out) => out,
        Err(e) => {
            return ReplayResult {
                success: false,
                status_code: 0,
                duration_ms: start.elapsed().as_millis() as u64,
                body: String::new(),
                headers_raw: String::new(),
                error: Some(format!("Failed to execute command: {}", e)),
            };
        }
    };

    let elapsed = start.elapsed().as_millis() as u64;
    let full_response = String::from_utf8_lossy(&output.stdout);

    let (payload, status_code) = if let Some(marker_idx) = full_response.rfind("\n__TH_STATUS__:") {
        let status_str = &full_response[marker_idx + 15..].trim();
        let parsed_code = status_str.parse::<u16>().unwrap_or(0);
        (&full_response[..marker_idx], parsed_code)
    } else {
        (full_response.as_ref(), 0)
    };

    let (headers_raw, body) = if let Some(sep) = payload.find("\r\n\r\n") {
        (payload[..sep].to_string(), payload[sep + 4..].to_string())
    } else if let Some(sep) = payload.find("\n\n") {
        (payload[..sep].to_string(), payload[sep + 2..].to_string())
    } else {
        (String::new(), payload.to_string())
    };

    ReplayResult {
        success: output.status.success() && status_code > 0,
        status_code,
        duration_ms: elapsed,
        body,
        headers_raw,
        error: if output.status.success() {
            None
        } else {
            Some(String::from_utf8_lossy(&output.stderr).to_string())
        },
    }
}

pub fn serve_run_directory(run_dir: &Path, port: u16, open: bool) -> Result<()> {
    let run = crate::ingest::ingest_run_directory(run_dir)?;
    let html_content = render_report_html(&run)?;
    let json_content = serde_json::to_string_pretty(&run)?;
    let har = crate::har::run_to_har(&run);
    let har_content = serde_json::to_string_pretty(&har)?;

    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr).map_err(|e| {
        TraceHurlError::SidecarError(format!("Failed to bind server to {}: {}", addr, e))
    })?;

    let url = format!("http://localhost:{}", port);
    println!("TraceHurl Sidecar Server listening on: {}", url);
    println!("  Run ID:    {}", run.run_id);
    println!("  Status:    {}", run.summary.overall_status);
    println!("  Endpoints: GET / (HTML), GET /report.json, GET /trace.har, POST /api/replay");
    println!("Press Ctrl+C to stop.");

    if open {
        let _ = opener::open(&url);
    }

    for mut stream in listener.incoming().flatten() {
        let _ = handle_connection(&mut stream, &html_content, &json_content, &har_content);
    }

    Ok(())
}

fn handle_connection(
    stream: &mut TcpStream,
    html: &str,
    json: &str,
    har: &str,
) -> std::io::Result<()> {
    let mut buffer = [0; 8192];
    let bytes_read = stream.read(&mut buffer)?;
    if bytes_read == 0 {
        return Ok(());
    }

    let request_str = String::from_utf8_lossy(&buffer[..bytes_read]);
    let mut lines = request_str.lines();
    let request_line = match lines.next() {
        Some(l) => l,
        None => return Ok(()),
    };

    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return Ok(());
    }

    let method = parts[0];
    let path = parts[1];

    if method == "OPTIONS" {
        let response = "HTTP/1.1 204 No Content\r\n\
Access-Control-Allow-Origin: *\r\n\
Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
Access-Control-Allow-Headers: Content-Type\r\n\
Content-Length: 0\r\n\r\n";
        return stream.write_all(response.as_bytes());
    }

    if method == "GET" {
        if path == "/" || path == "/index.html" {
            let response = format!(
                "HTTP/1.1 200 OK\r\n\
Content-Type: text/html; charset=utf-8\r\n\
Access-Control-Allow-Origin: *\r\n\
Content-Length: {}\r\n\r\n{}",
                html.len(),
                html
            );
            return stream.write_all(response.as_bytes());
        }

        if path == "/report.json" {
            let response = format!(
                "HTTP/1.1 200 OK\r\n\
Content-Type: application/json\r\n\
Access-Control-Allow-Origin: *\r\n\
Content-Length: {}\r\n\r\n{}",
                json.len(),
                json
            );
            return stream.write_all(response.as_bytes());
        }

        if path == "/trace.har" {
            let response = format!(
                "HTTP/1.1 200 OK\r\n\
Content-Type: application/json\r\n\
Access-Control-Allow-Origin: *\r\n\
Content-Length: {}\r\n\r\n{}",
                har.len(),
                har
            );
            return stream.write_all(response.as_bytes());
        }
    }

    if method == "POST" && path == "/api/replay" {
        let body_start = if let Some(idx) = request_str.find("\r\n\r\n") {
            idx + 4
        } else if let Some(idx) = request_str.find("\n\n") {
            idx + 2
        } else {
            0
        };

        let post_body = &request_str[body_start..];
        let replay_req: std::result::Result<ReplayRequest, _> = serde_json::from_str(post_body);

        let result = match replay_req {
            Ok(req) => execute_live_curl(&req.curl_cmd),
            Err(e) => ReplayResult {
                success: false,
                status_code: 400,
                duration_ms: 0,
                body: String::new(),
                headers_raw: String::new(),
                error: Some(format!("Invalid request body: {}", e)),
            },
        };

        let result_json = serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string());
        let response = format!(
            "HTTP/1.1 200 OK\r\n\
Content-Type: application/json\r\n\
Access-Control-Allow-Origin: *\r\n\
Content-Length: {}\r\n\r\n{}",
            result_json.len(),
            result_json
        );
        return stream.write_all(response.as_bytes());
    }

    let not_found = "HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\n\r\nNot Found";
    stream.write_all(not_found.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rejects_non_curl_command() {
        let result = execute_live_curl("rm -rf /");
        assert!(!result.success);
        assert!(result.error.unwrap().contains("must start with 'curl'"));
    }

    #[test]
    fn test_replay_result_serialization() {
        let res = ReplayResult {
            success: true,
            status_code: 200,
            duration_ms: 45,
            body: "{\"status\":\"ok\"}".to_string(),
            headers_raw: "HTTP/1.1 200 OK".to_string(),
            error: None,
        };

        let json = serde_json::to_string(&res).unwrap();
        assert!(json.contains("\"statusCode\":200"));
        assert!(json.contains("\"durationMs\":45"));
    }
}
