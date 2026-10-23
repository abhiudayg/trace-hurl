use crate::error::Result;
use crate::model::HeaderItem;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum HurlJsonPayload {
    Single(RawHurlReport),
    Multiple(Vec<RawHurlReport>),
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawHurlReport {
    pub filename: Option<String>,
    pub success: Option<bool>,
    pub time: Option<u64>,
    #[serde(default)]
    pub entries: Vec<RawHurlEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawHurlEntry {
    pub index: usize,
    pub line: usize,
    pub time: Option<u64>,
    pub curl_cmd: Option<String>,
    #[serde(default)]
    pub asserts: Vec<RawHurlAssert>,
    #[serde(default)]
    pub captures: Vec<RawHurlCapture>,
    #[serde(default)]
    pub calls: Vec<RawHurlCall>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawHurlAssert {
    pub line: usize,
    pub success: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawHurlCapture {
    pub name: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawHurlCall {
    pub request: RawHurlRequest,
    pub response: Option<RawHurlResponse>,
    pub timings: Option<RawHurlTimings>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawHurlRequest {
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub headers: Vec<HeaderItem>,
    #[serde(default)]
    pub query_string: Vec<HeaderItem>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawHurlResponse {
    pub status: u16,
    #[serde(default)]
    pub headers: Vec<HeaderItem>,
    pub http_version: Option<String>,
    pub body: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawHurlTimings {
    pub begin_call: Option<String>,
    pub end_call: Option<String>,
    pub total: Option<u64>,
}

pub fn parse_hurl_json(json_content: &str) -> Result<Vec<RawHurlReport>> {
    let payload: HurlJsonPayload = serde_json::from_str(json_content)?;
    match payload {
        HurlJsonPayload::Single(r) => Ok(vec![r]),
        HurlJsonPayload::Multiple(v) => Ok(v),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedAssertMessage {
    pub expression: Option<String>,
    pub actual: Option<String>,
    pub expected: Option<String>,
    pub error_type: String,
}

pub fn parse_assert_message(msg: &str) -> ParsedAssertMessage {
    let mut expression = None;
    let mut actual = None;
    let mut expected = None;
    let error_type = if msg.contains("Undefined variable") {
        "UndefinedVariable".to_string()
    } else {
        "AssertFailure".to_string()
    };

    for line in msg.lines() {
        let trimmed = line.trim();
        if let Some(pos) = trimmed.find('|') {
            let after_pipe = trimmed[pos + 1..].trim();
            if after_pipe.starts_with("actual:") {
                let act = after_pipe.trim_start_matches("actual:").trim();
                actual = Some(strip_type_wrapper(act));
            } else if after_pipe.starts_with("expected:") {
                let exp = after_pipe.trim_start_matches("expected:").trim();
                expected = Some(strip_type_wrapper(exp));
            } else if trimmed.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                expression = Some(after_pipe.to_string());
            }
        }
    }

    ParsedAssertMessage {
        expression,
        actual,
        expected,
        error_type,
    }
}

fn strip_type_wrapper(raw: &str) -> String {
    let s = raw.trim();
    match (s.find('<'), s.rfind('>')) {
        (Some(start), Some(end)) if end > start => s[start + 1..end].to_string(),
        _ => s.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_assert_message_actual_expected() {
        let msg = r#"error: Assert failure
    --> /path/to/loan-import.hurl:20:0
     |
  20 | jsonpath "$.requestType" == "LOAN_CONVERSION"
     |   actual:   string <loanConversion>
     |   expected: string <LOAN_CONVERSION>
     |"#;
        let parsed = parse_assert_message(msg);
        assert_eq!(parsed.actual, Some("loanConversion".to_string()));
        assert_eq!(parsed.expected, Some("LOAN_CONVERSION".to_string()));
        assert_eq!(
            parsed.expression,
            Some(r#"jsonpath "$.requestType" == "LOAN_CONVERSION""#.to_string())
        );
    }

    #[test]
    fn test_parse_hurl_json_array_and_single() {
        let single_json = r#"{"entries": [{"index": 1, "line": 1}]}"#;
        let single_parsed = parse_hurl_json(single_json).expect("single");
        assert_eq!(single_parsed.len(), 1);

        let array_json =
            r#"[{"entries": [{"index": 1, "line": 1}]}, {"entries": [{"index": 2, "line": 5}]}]"#;
        let array_parsed = parse_hurl_json(array_json).expect("array");
        assert_eq!(array_parsed.len(), 2);
    }
}
