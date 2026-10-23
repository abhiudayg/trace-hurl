use regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;

static TOKEN_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(eyJ[a-zA-Z0-9_-]{10,}\.[a-zA-Z0-9_-]{10,}\.[a-zA-Z0-9_-]{10,})").unwrap()
});

#[derive(Debug, Clone)]
pub struct RedactionConfig {
    pub headers: HashSet<String>,
    pub query_params: HashSet<String>,
    pub json_keys: HashSet<String>,
}

impl Default for RedactionConfig {
    fn default() -> Self {
        let mut headers = HashSet::new();
        headers.insert("authorization".to_string());
        headers.insert("cookie".to_string());
        headers.insert("set-cookie".to_string());
        headers.insert("x-zeta-authtoken".to_string());
        headers.insert("x-api-key".to_string());
        headers.insert("x-auth-token".to_string());

        let mut query_params = HashSet::new();
        query_params.insert("token".to_string());
        query_params.insert("access_token".to_string());
        query_params.insert("secret".to_string());
        query_params.insert("key".to_string());
        query_params.insert("api_key".to_string());
        query_params.insert("auth".to_string());
        query_params.insert("password".to_string());

        let mut json_keys = HashSet::new();
        json_keys.insert("token".to_string());
        json_keys.insert("accessToken".to_string());
        json_keys.insert("password".to_string());
        json_keys.insert("secret".to_string());

        Self {
            headers,
            query_params,
            json_keys,
        }
    }
}

pub struct Redactor {
    config: RedactionConfig,
}

impl Redactor {
    pub fn new(config: RedactionConfig) -> Self {
        Self { config }
    }

    pub fn redact_header_value(&self, header_name: &str, value: &str) -> String {
        let lower = header_name.to_ascii_lowercase();
        if self.config.headers.contains(&lower) {
            if value.to_ascii_lowercase().starts_with("bearer ") {
                "Bearer [REDACTED]".to_string()
            } else {
                "[REDACTED]".to_string()
            }
        } else {
            self.redact_text(value)
        }
    }

    pub fn redact_url(&self, url: &str) -> String {
        let parts: Vec<&str> = url.splitn(2, '?').collect();
        if parts.len() < 2 {
            return self.redact_text(url);
        }

        let base = parts[0];
        let query = parts[1];
        let new_query: Vec<String> = query
            .split('&')
            .map(|param| {
                let kv: Vec<&str> = param.splitn(2, '=').collect();
                if kv.len() == 2 {
                    let key = kv[0];
                    if self.config.query_params.contains(&key.to_ascii_lowercase()) {
                        format!("{}=[REDACTED]", key)
                    } else {
                        param.to_string()
                    }
                } else {
                    param.to_string()
                }
            })
            .collect();

        format!("{}?{}", base, new_query.join("&"))
    }

    pub fn redact_text(&self, text: &str) -> String {
        TOKEN_PATTERN
            .replace_all(text, "[REDACTED_JWT]")
            .to_string()
    }

    pub fn redact_curl_cmd(&self, cmd: &str) -> String {
        let without_jwt = self.redact_text(cmd);
        let header_re =
            Regex::new(r"(?i)--header\s+'(authorization|cookie|x-zeta-authtoken):\s*[^']+'")
                .unwrap();
        header_re
            .replace_all(&without_jwt, "--header '$1: [REDACTED]'")
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_bearer_token() {
        let redactor = Redactor::new(RedactionConfig::default());
        let redacted = redactor.redact_header_value(
            "Authorization",
            "Bearer eyJhbGciOiJFZERTQSJ9.eyJzdWIiOiIxMjMifQ.abcde",
        );
        assert_eq!(redacted, "Bearer [REDACTED]");
    }

    #[test]
    fn test_redact_url_query_params() {
        let redactor = Redactor::new(RedactionConfig::default());
        let redacted = redactor
            .redact_url("https://api.example.com/v1/import?account=123&token=secret123&foo=bar");
        assert_eq!(
            redacted,
            "https://api.example.com/v1/import?account=123&token=[REDACTED]&foo=bar"
        );
    }

    #[test]
    fn test_redact_curl_command() {
        let redactor = Redactor::new(RedactionConfig::default());
        let cmd = "curl --header 'Authorization: Bearer secret' --header 'X-Zeta-AuthToken: zeta-secret' http://localhost";
        let redacted = redactor.redact_curl_cmd(cmd);
        assert!(!redacted.contains("Bearer secret"));
        assert!(!redacted.contains("zeta-secret"));
    }
}
