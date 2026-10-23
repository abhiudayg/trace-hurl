use crate::model::{HeaderItem, Run};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Har {
    pub log: HarLog,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HarLog {
    pub version: String,
    pub creator: HarCreator,
    pub entries: Vec<HarEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HarCreator {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HarEntry {
    pub started_date_time: String,
    pub time: u64,
    pub request: HarRequest,
    pub response: HarResponse,
    pub cache: serde_json::Value,
    pub timings: HarTimings,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HarRequest {
    pub method: String,
    pub url: String,
    pub http_version: String,
    pub cookies: Vec<serde_json::Value>,
    pub headers: Vec<HarHeader>,
    pub query_string: Vec<HarHeader>,
    pub post_data: Option<HarPostData>,
    pub headers_size: i64,
    pub body_size: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HarResponse {
    pub status: u16,
    pub status_text: String,
    pub http_version: String,
    pub cookies: Vec<serde_json::Value>,
    pub headers: Vec<HarHeader>,
    pub content: HarContent,
    pub redirect_url: String,
    pub headers_size: i64,
    pub body_size: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HarHeader {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HarPostData {
    pub mime_type: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HarContent {
    pub size: usize,
    pub mime_type: String,
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HarTimings {
    pub send: i64,
    pub wait: i64,
    pub receive: i64,
}

fn status_code_to_reason(code: u16) -> &'static str {
    match code {
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        304 => "Not Modified",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        422 => "Unprocessable Entity",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "Unknown",
    }
}

fn detect_mime_type(headers: &[HeaderItem]) -> String {
    headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case("content-type"))
        .map(|h| h.value.clone())
        .unwrap_or_else(|| "application/json".to_string())
}

pub fn run_to_har(run: &Run) -> Har {
    let started_time = run
        .started_at
        .clone()
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string());

    let mut har_entries = Vec::new();

    for scenario in &run.scenarios {
        for entry in &scenario.entries {
            let req_headers: Vec<HarHeader> = entry
                .request
                .headers
                .iter()
                .map(|h| HarHeader {
                    name: h.name.clone(),
                    value: h.value.clone(),
                })
                .collect();

            let query_string: Vec<HarHeader> = entry
                .request
                .query_string
                .iter()
                .map(|q| HarHeader {
                    name: q.name.clone(),
                    value: q.value.clone(),
                })
                .collect();

            let req_mime = detect_mime_type(&entry.request.headers);
            let post_data = entry
                .request
                .body_snippet
                .as_ref()
                .map(|snippet| HarPostData {
                    mime_type: req_mime,
                    text: snippet.clone(),
                });

            let body_size = post_data.as_ref().map(|p| p.text.len() as i64).unwrap_or(0);

            let har_req = HarRequest {
                method: entry.request.method.clone(),
                url: entry.request.url.clone(),
                http_version: "HTTP/1.1".to_string(),
                cookies: Vec::new(),
                headers: req_headers,
                query_string,
                post_data,
                headers_size: -1,
                body_size,
            };

            let resp_headers: Vec<HarHeader> = entry
                .response
                .as_ref()
                .map(|r| {
                    r.headers
                        .iter()
                        .map(|h| HarHeader {
                            name: h.name.clone(),
                            value: h.value.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default();

            let resp_mime = entry
                .response
                .as_ref()
                .map(|r| detect_mime_type(&r.headers))
                .unwrap_or_else(|| "application/json".to_string());

            let resp_body = entry.response.as_ref().and_then(|r| r.body_snippet.clone());
            let resp_body_size = resp_body.as_ref().map(|b| b.len()).unwrap_or(0);

            let har_resp = HarResponse {
                status: entry.status_code,
                status_text: status_code_to_reason(entry.status_code).to_string(),
                http_version: entry
                    .response
                    .as_ref()
                    .and_then(|r| r.http_version.clone())
                    .unwrap_or_else(|| "HTTP/1.1".to_string()),
                cookies: Vec::new(),
                headers: resp_headers,
                content: HarContent {
                    size: resp_body_size,
                    mime_type: resp_mime,
                    text: resp_body,
                },
                redirect_url: String::new(),
                headers_size: -1,
                body_size: resp_body_size as i64,
            };

            har_entries.push(HarEntry {
                started_date_time: started_time.clone(),
                time: entry.duration_ms,
                request: har_req,
                response: har_resp,
                cache: serde_json::json!({}),
                timings: HarTimings {
                    send: 0,
                    wait: entry.duration_ms as i64,
                    receive: 0,
                },
            });
        }
    }

    Har {
        log: HarLog {
            version: "1.2".to_string(),
            creator: HarCreator {
                name: "TraceHurl".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            entries: har_entries,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        Entry, EntryOutcome, HeaderItem, RequestData, ResponseData, Run, RunSummary, Scenario,
        ScenarioSummary,
    };
    use std::collections::HashMap;

    #[test]
    fn test_run_to_har_generates_valid_version_and_creator() {
        let run = Run {
            schema_version: 1,
            run_id: "RUN_001".to_string(),
            started_at: Some("2026-10-23T10:00:00Z".to_string()),
            finished_at: Some("2026-10-23T10:00:05Z".to_string()),
            tools: HashMap::new(),
            phases: vec![],
            scenarios: vec![],
            artifacts: vec![],
            variables: HashMap::new(),
            diagnostics: vec![],
            summary: RunSummary::default(),
            failure_clusters: vec![],
        };

        let har = run_to_har(&run);
        assert_eq!(har.log.version, "1.2");
        assert_eq!(har.log.creator.name, "TraceHurl");
        assert!(har.log.entries.is_empty());
    }

    #[test]
    fn test_run_to_har_maps_request_and_response_fields() {
        let entry = Entry {
            id: "e1".to_string(),
            index: 1,
            scenario_id: "sc1".to_string(),
            source_file: "test.hurl".to_string(),
            source_line: 1,
            method: "POST".to_string(),
            url: "https://api.example.com/v1/auth".to_string(),
            status_code: 201,
            outcome: EntryOutcome::Passed,
            duration_ms: 150,
            request: RequestData {
                method: "POST".to_string(),
                url: "https://api.example.com/v1/auth".to_string(),
                headers: vec![HeaderItem {
                    name: "Content-Type".to_string(),
                    value: "application/json".to_string(),
                }],
                query_string: vec![],
                body_snippet: Some(r#"{"username":"admin"}"#.to_string()),
                body_path: None,
            },
            response: Some(ResponseData {
                status: 201,
                headers: vec![HeaderItem {
                    name: "Content-Type".to_string(),
                    value: "application/json".to_string(),
                }],
                http_version: Some("HTTP/2.0".to_string()),
                body_snippet: Some(r#"{"token":"abc"}"#.to_string()),
                body_path: None,
                is_derived: false,
                expected_body_snippet: None,
            }),
            assertions: vec![],
            captures_set: vec![],
            captures_used: vec![],
            diffs: vec![],
            curl_cmd: None,
            cluster_id: None,
        };

        let scenario = Scenario {
            id: "sc1".to_string(),
            source_file: "test.hurl".to_string(),
            name: "Scenario 1".to_string(),
            description: "Test".to_string(),
            entries: vec![entry],
            summary: ScenarioSummary::default(),
            all_captures: vec![],
        };

        let run = Run {
            schema_version: 1,
            run_id: "RUN_002".to_string(),
            started_at: Some("2026-10-23T10:00:00Z".to_string()),
            finished_at: None,
            tools: HashMap::new(),
            phases: vec![],
            scenarios: vec![scenario],
            artifacts: vec![],
            variables: HashMap::new(),
            diagnostics: vec![],
            summary: RunSummary::default(),
            failure_clusters: vec![],
        };

        let har = run_to_har(&run);
        assert_eq!(har.log.entries.len(), 1);
        let har_entry = &har.log.entries[0];
        assert_eq!(har_entry.request.method, "POST");
        assert_eq!(har_entry.request.url, "https://api.example.com/v1/auth");
        assert_eq!(
            har_entry.request.post_data.as_ref().map(|p| &p.text),
            Some(&r#"{"username":"admin"}"#.to_string())
        );
        assert_eq!(har_entry.response.status, 201);
        assert_eq!(har_entry.response.status_text, "Created");
        assert_eq!(har_entry.response.http_version, "HTTP/2.0");
        assert_eq!(
            har_entry.response.content.text,
            Some(r#"{"token":"abc"}"#.to_string())
        );
        assert_eq!(har_entry.timings.wait, 150);
    }
}
