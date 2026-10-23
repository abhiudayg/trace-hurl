use regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;

static VAR_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{\{([a-zA-Z0-9_-]+)\}\}").unwrap());
static STATUS_RANGE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"status\s*>=\s*400").unwrap());
static WEAK_TYPE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(isString|isNumber|isBoolean|isCollection)").unwrap());

#[derive(Debug, Clone)]
pub struct HurlSourceAssert {
    pub line: usize,
    pub raw: String,
    pub is_status_range: bool,
    pub is_weak_type: bool,
}

#[derive(Debug, Clone)]
pub struct HurlSourceEntry {
    pub line: usize,
    pub method: String,
    pub url: String,
    pub expected_status: Option<u16>,
    pub payload_file: Option<String>,
    pub captures: Vec<(String, String)>,
    pub asserts: Vec<HurlSourceAssert>,
    pub variables_used: HashSet<String>,
    pub is_retry: bool,
}

#[derive(Debug, Clone)]
pub struct ParsedHurlSource {
    pub scenario_name: String,
    pub scenario_description: String,
    pub entries: Vec<HurlSourceEntry>,
}

pub fn parse_hurl_source(content: &str, default_filename: &str) -> ParsedHurlSource {
    let lines: Vec<&str> = content.lines().collect();
    let mut scenario_name = default_filename.to_string();
    let mut scenario_description = String::new();

    let mut line_idx = 0;
    while line_idx < lines.len() {
        let line = lines[line_idx].trim();
        if line.starts_with('#') {
            let comment = line.trim_start_matches('#').trim();
            if scenario_description.is_empty() {
                if let Some(stripped) = comment.strip_prefix("Scenario:") {
                    scenario_name = stripped.trim().to_string();
                } else if !comment.is_empty() {
                    scenario_name = comment.to_string();
                }
                scenario_description = comment.to_string();
            } else {
                scenario_description.push(' ');
                scenario_description.push_str(comment);
            }
            line_idx += 1;
        } else if line.is_empty() {
            line_idx += 1;
        } else {
            break;
        }
    }

    let mut entries = Vec::new();
    let mut current_entry: Option<HurlSourceEntry> = None;
    let mut in_captures = false;
    let mut in_asserts = false;
    let mut in_options = false;

    for (idx, line_raw) in lines.iter().enumerate() {
        let line_num = idx + 1;
        let line = line_raw.trim();

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let is_http_request = is_method_start(line);
        if is_http_request {
            if let Some(entry) = current_entry.take() {
                entries.push(entry);
            }
            in_captures = false;
            in_asserts = false;
            in_options = false;

            let parts: Vec<&str> = line.split_whitespace().collect();
            let method = parts.first().unwrap_or(&"GET").to_string();
            let url = parts.get(1).unwrap_or(&"").to_string();

            let mut vars = HashSet::new();
            for cap in VAR_RE.captures_iter(line) {
                if let Some(var_match) = cap.get(1) {
                    vars.insert(var_match.as_str().to_string());
                }
            }

            current_entry = Some(HurlSourceEntry {
                line: line_num,
                method,
                url,
                expected_status: None,
                payload_file: None,
                captures: Vec::new(),
                asserts: Vec::new(),
                variables_used: vars,
                is_retry: false,
            });
            continue;
        }

        if let Some(entry) = current_entry.as_mut() {
            for cap in VAR_RE.captures_iter(line) {
                if let Some(var_match) = cap.get(1) {
                    entry.variables_used.insert(var_match.as_str().to_string());
                }
            }

            if line.starts_with("file,") {
                let file_part = line
                    .trim_start_matches("file,")
                    .trim()
                    .trim_end_matches(';');
                entry.payload_file = Some(file_part.to_string());
            } else if line.starts_with("HTTP ") || line.starts_with("HTTP/") {
                if let Some(st) = line
                    .split_whitespace()
                    .nth(1)
                    .and_then(|s| s.parse::<u16>().ok())
                {
                    entry.expected_status = Some(st);
                }
            } else if line == "[Captures]" {
                in_captures = true;
                in_asserts = false;
                in_options = false;
            } else if line == "[Asserts]" {
                in_asserts = true;
                in_captures = false;
                in_options = false;
            } else if line == "[Options]" {
                in_options = true;
                in_captures = false;
                in_asserts = false;
            } else if in_options {
                if line.starts_with("retry:") {
                    entry.is_retry = true;
                }
            } else if in_captures {
                if let Some(colon_pos) = line.find(':') {
                    let cap_name = line[..colon_pos].trim().to_string();
                    let cap_expr = line[colon_pos + 1..].trim().to_string();
                    entry.captures.push((cap_name, cap_expr));
                }
            } else if in_asserts {
                let is_status_range = STATUS_RANGE_RE.is_match(line);
                let is_weak_type = WEAK_TYPE_RE.is_match(line);
                entry.asserts.push(HurlSourceAssert {
                    line: line_num,
                    raw: line.to_string(),
                    is_status_range,
                    is_weak_type,
                });
            }
        }
    }

    if let Some(entry) = current_entry {
        entries.push(entry);
    }

    ParsedHurlSource {
        scenario_name,
        scenario_description,
        entries,
    }
}

fn is_method_start(line: &str) -> bool {
    let methods = [
        "GET ", "POST ", "PUT ", "DELETE ", "PATCH ", "HEAD ", "OPTIONS ",
    ];
    methods.iter().any(|m| line.starts_with(m))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hurl_source_scenario_header() {
        let content = r#"# Scenario: Loan Import Happy Path
# Verifies successful import of valid loans
POST http://localhost:8081/import
HTTP 202
[Asserts]
jsonpath "$.status" == "Accepted"
"#;
        let parsed = parse_hurl_source(content, "test.hurl");
        assert_eq!(parsed.scenario_name, "Loan Import Happy Path");
        assert!(parsed.scenario_description.contains("Verifies successful"));
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].method, "POST");
        assert_eq!(parsed.entries[0].expected_status, Some(202));
        assert_eq!(parsed.entries[0].asserts.len(), 1);
    }
}
