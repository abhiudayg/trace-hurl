use crate::model::{Entry, EntryOutcome, FailureCluster, Scenario};
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

static ARRAY_INDEX_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\d+\]").unwrap_or_else(|_| Regex::new("$^").unwrap()));
static NUMERIC_PATH_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"/\d+").unwrap_or_else(|_| Regex::new("$^").unwrap()));
static UUID_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}")
        .unwrap_or_else(|_| Regex::new("$^").unwrap())
});

#[derive(Debug, Clone)]
struct FailureSignature {
    category: String,
    pattern: String,
    title: String,
    sample_error: String,
    remediation: Option<String>,
}

fn normalize_jsonpath(expr: &str) -> String {
    let unquoted = if let Some(start) = expr.find('"')
        && let Some(end) = expr.rfind('"')
        && start < end
    {
        &expr[start + 1..end]
    } else {
        expr
    };

    let without_uuid = UUID_RE.replace_all(unquoted, "<id>");
    ARRAY_INDEX_RE.replace_all(&without_uuid, "[*]").to_string()
}

fn normalize_path(path: &str) -> String {
    let without_uuid = UUID_RE.replace_all(path, "<id>");
    NUMERIC_PATH_RE.replace_all(&without_uuid, "/*").to_string()
}

fn extract_failure_signature(entry: &Entry) -> FailureSignature {
    if let Some(failed_assert) = entry.assertions.iter().find(|a| !a.passed) {
        if failed_assert.expression == "status" || failed_assert.comparison_target == "status" {
            let expected = failed_assert.expected.as_deref().unwrap_or("unknown");
            let actual = failed_assert.actual.as_deref().unwrap_or("unknown");
            return FailureSignature {
                category: "HTTP Status Mismatch".to_string(),
                pattern: format!("status == {expected} (actual: {actual})"),
                title: format!("HTTP Status Mismatch: expected {expected}, got {actual}"),
                sample_error: failed_assert.message.clone().unwrap_or_else(|| {
                    format!("Expected HTTP status {expected}, but received {actual}")
                }),
                remediation: Some(
                    "Inspect server logs for 5xx errors or verify endpoint authorization."
                        .to_string(),
                ),
            };
        }

        if failed_assert.expression.contains("jsonpath") {
            let normalized = normalize_jsonpath(&failed_assert.expression);
            return FailureSignature {
                category: "Contract Schema Mismatch".to_string(),
                pattern: format!("Missing or invalid attribute: {normalized}"),
                title: format!("Schema Contract Mismatch: {normalized}"),
                sample_error: failed_assert
                    .message
                    .clone()
                    .unwrap_or_else(|| format!("Assertion on JSONPath {normalized} failed")),
                remediation: Some(
                    "Ensure backend API service populates all required schema attributes in response."
                        .to_string(),
                ),
            };
        }

        if failed_assert.expression.contains("header") {
            return FailureSignature {
                category: "Header Mismatch".to_string(),
                pattern: format!("Header mismatch: {}", failed_assert.expression),
                title: format!("Header Mismatch: {}", failed_assert.expression),
                sample_error: failed_assert.message.clone().unwrap_or_else(|| {
                    format!("Header assertion failed: {}", failed_assert.expression)
                }),
                remediation: Some(
                    "Verify upstream service response headers and content-type negotiation."
                        .to_string(),
                ),
            };
        }

        return FailureSignature {
            category: "Assertion Failed".to_string(),
            pattern: failed_assert.expression.clone(),
            title: format!("Assertion Failed: {}", failed_assert.expression),
            sample_error: failed_assert.message.clone().unwrap_or_else(|| {
                format!(
                    "Assertion expression '{}' evaluated to false",
                    failed_assert.expression
                )
            }),
            remediation: Some(
                "Review test assertion expectations against backend behavior.".to_string(),
            ),
        };
    }

    if let Some(diff) = entry.diffs.iter().find(|d| d.diff_type == "Removed") {
        let norm = normalize_path(&diff.path);
        return FailureSignature {
            category: "Missing Contract Field".to_string(),
            pattern: format!("Missing required field: {norm}"),
            title: format!("Missing Field: {norm}"),
            sample_error: format!(
                "Contract field '{norm}' is missing from actual response payload"
            ),
            remediation: Some(
                "Populate required attribute in response or update OpenAPI contract schema."
                    .to_string(),
            ),
        };
    }

    if let Some(diff) = entry.diffs.iter().find(|d| d.diff_type == "TypeMismatch") {
        let norm = normalize_path(&diff.path);
        let expected = diff.expected.as_deref().unwrap_or("expected_type");
        let actual = diff.actual.as_deref().unwrap_or("actual_type");
        return FailureSignature {
            category: "Contract Type Mismatch".to_string(),
            pattern: format!("Type mismatch at {norm} ({expected} vs {actual})"),
            title: format!("Type Mismatch: {norm}"),
            sample_error: format!("Expected field '{norm}' to be {expected}, but got {actual}"),
            remediation: Some(
                "Fix serialization type on the backend service to match schema contract."
                    .to_string(),
            ),
        };
    }

    if let Some(diff) = entry.diffs.iter().find(|d| d.diff_type == "ValueMismatch") {
        let norm = normalize_path(&diff.path);
        return FailureSignature {
            category: "Contract Value Mismatch".to_string(),
            pattern: format!("Value mismatch at {norm}"),
            title: format!("Value Mismatch: {norm}"),
            sample_error: format!("Value at '{norm}' does not match contract expectation"),
            remediation: Some(
                "Verify backend business logic output for the requested entity.".to_string(),
            ),
        };
    }

    if entry.status_code >= 500 {
        return FailureSignature {
            category: "Server Error".to_string(),
            pattern: format!("HTTP {} Internal Server Error", entry.status_code),
            title: format!("HTTP {} Server Error", entry.status_code),
            sample_error: format!("Server responded with HTTP {}", entry.status_code),
            remediation: Some(
                "Check application logs, database connection pools, or upstream gateway timeouts."
                    .to_string(),
            ),
        };
    }

    if entry.status_code == 404 {
        return FailureSignature {
            category: "Route Not Found".to_string(),
            pattern: "HTTP 404 Route Not Found".to_string(),
            title: "HTTP 404 Route Not Found".to_string(),
            sample_error: format!("Endpoint {} returned 404 Not Found", entry.url),
            remediation: Some(
                "Verify route registration, base URL, and resource path identifiers.".to_string(),
            ),
        };
    }

    if entry.status_code == 401 || entry.status_code == 403 {
        return FailureSignature {
            category: "Authentication Failure".to_string(),
            pattern: format!(
                "HTTP {} Authentication / Authorization Failed",
                entry.status_code
            ),
            title: format!("HTTP {} Auth Error", entry.status_code),
            sample_error: format!("Endpoint {} returned HTTP {}", entry.url, entry.status_code),
            remediation: Some(
                "Check Bearer token expiration, client API key, or RBAC permissions.".to_string(),
            ),
        };
    }

    FailureSignature {
        category: "General Failure".to_string(),
        pattern: format!("Failed status {}", entry.status_code),
        title: format!("HTTP {} Failure", entry.status_code),
        sample_error: format!("Request failed with HTTP status {}", entry.status_code),
        remediation: None,
    }
}

struct GroupCollector {
    category: String,
    pattern: String,
    title: String,
    sample_error: String,
    remediation: Option<String>,
    scenario_ids: Vec<String>,
    entry_ids: Vec<String>,
}

pub fn analyze_and_cluster_failures(
    scenarios: &[Scenario],
) -> (Vec<Scenario>, Vec<FailureCluster>) {
    let mut groups: HashMap<(String, String), GroupCollector> = HashMap::new();

    for scenario in scenarios {
        for entry in &scenario.entries {
            if entry.outcome == EntryOutcome::Failed {
                let sig = extract_failure_signature(entry);
                let key = (sig.category.clone(), sig.pattern.clone());

                let entry_slot = groups.entry(key).or_insert_with(|| GroupCollector {
                    category: sig.category,
                    pattern: sig.pattern,
                    title: sig.title,
                    sample_error: sig.sample_error,
                    remediation: sig.remediation,
                    scenario_ids: Vec::new(),
                    entry_ids: Vec::new(),
                });

                if !entry_slot.scenario_ids.contains(&scenario.id) {
                    entry_slot.scenario_ids.push(scenario.id.clone());
                }
                entry_slot.entry_ids.push(entry.id.clone());
            }
        }
    }

    let mut collectors: Vec<GroupCollector> = groups.into_values().collect();
    collectors.sort_by(|a, b| {
        b.entry_ids
            .len()
            .cmp(&a.entry_ids.len())
            .then_with(|| a.pattern.cmp(&b.pattern))
    });

    let mut clusters = Vec::with_capacity(collectors.len());
    let mut entry_to_cluster: HashMap<String, String> = HashMap::new();

    for (idx, mut col) in collectors.into_iter().enumerate() {
        let cluster_id = format!("cluster-{}", idx + 1);
        col.scenario_ids.sort();

        for eid in &col.entry_ids {
            entry_to_cluster.insert(eid.clone(), cluster_id.clone());
        }

        clusters.push(FailureCluster {
            id: cluster_id,
            title: col.title,
            root_cause_category: col.category,
            error_pattern: col.pattern,
            entry_count: col.entry_ids.len(),
            scenario_ids: col.scenario_ids,
            entry_ids: col.entry_ids,
            sample_error: col.sample_error,
            suggested_remediation: col.remediation,
        });
    }

    let updated_scenarios = scenarios
        .iter()
        .map(|s| {
            let updated_entries = s
                .entries
                .iter()
                .map(|e| {
                    let mut updated_e = e.clone();
                    updated_e.cluster_id = entry_to_cluster.get(&e.id).cloned();
                    updated_e
                })
                .collect();

            let mut updated_s = s.clone();
            updated_s.entries = updated_entries;
            updated_s
        })
        .collect();

    (updated_scenarios, clusters)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        Assertion, DiffItem, Entry, EntryOutcome, ProvenanceOrigin, RequestData, Scenario,
        ScenarioSummary,
    };

    fn make_test_entry(
        id: &str,
        outcome: EntryOutcome,
        status_code: u16,
        assertions: Vec<Assertion>,
        diffs: Vec<DiffItem>,
    ) -> Entry {
        Entry {
            id: id.to_string(),
            index: 1,
            scenario_id: "scenario-1".to_string(),
            source_file: "test.hurl".to_string(),
            source_line: 10,
            method: "POST".to_string(),
            url: "https://api.example.com/test".to_string(),
            status_code,
            outcome,
            duration_ms: 120,
            request: RequestData {
                method: "POST".to_string(),
                url: "https://api.example.com/test".to_string(),
                headers: vec![],
                query_string: vec![],
                body_snippet: None,
                body_path: None,
            },
            response: None,
            assertions,
            captures_set: vec![],
            captures_used: vec![],
            diffs,
            curl_cmd: None,
            cluster_id: None,
        }
    }

    fn make_test_scenario(entries: Vec<Entry>) -> Scenario {
        Scenario {
            id: "scenario-1".to_string(),
            source_file: "test.hurl".to_string(),
            name: "Test Scenario".to_string(),
            description: "Clustering test".to_string(),
            entries,
            summary: ScenarioSummary::default(),
            all_captures: vec![],
        }
    }

    #[test]
    fn test_zero_failures_produce_zero_clusters() {
        let entry = make_test_entry("e1", EntryOutcome::Passed, 200, vec![], vec![]);
        let scenario = make_test_scenario(vec![entry]);
        let (scenarios, clusters) = analyze_and_cluster_failures(&[scenario]);

        assert!(clusters.is_empty());
        assert!(scenarios[0].entries[0].cluster_id.is_none());
    }

    #[test]
    fn test_clusters_jsonpath_missing_attributes_with_normalization() {
        let assert1 = Assertion {
            line: 12,
            expression: r#"jsonpath "$.items[0].authCode""#.to_string(),
            comparison_target: "authCode".to_string(),
            expected: Some("string".to_string()),
            actual: None,
            passed: false,
            message: Some("Attribute authCode not found".to_string()),
            origin: ProvenanceOrigin::Recorded,
        };
        let assert2 = Assertion {
            line: 24,
            expression: r#"jsonpath "$.items[5].authCode""#.to_string(),
            comparison_target: "authCode".to_string(),
            expected: Some("string".to_string()),
            actual: None,
            passed: false,
            message: Some("Attribute authCode not found".to_string()),
            origin: ProvenanceOrigin::Recorded,
        };

        let e1 = make_test_entry("e1", EntryOutcome::Failed, 200, vec![assert1], vec![]);
        let e2 = make_test_entry("e2", EntryOutcome::Failed, 200, vec![assert2], vec![]);
        let scenario = make_test_scenario(vec![e1, e2]);

        let (scenarios, clusters) = analyze_and_cluster_failures(&[scenario]);

        assert_eq!(clusters.len(), 1);
        let cluster = &clusters[0];
        assert_eq!(cluster.id, "cluster-1");
        assert_eq!(cluster.entry_count, 2);
        assert_eq!(cluster.root_cause_category, "Contract Schema Mismatch");
        assert!(cluster.error_pattern.contains("$.items[*].authCode"));
        assert!(cluster.suggested_remediation.is_some());

        assert_eq!(
            scenarios[0].entries[0].cluster_id.as_deref(),
            Some("cluster-1")
        );
        assert_eq!(
            scenarios[0].entries[1].cluster_id.as_deref(),
            Some("cluster-1")
        );
    }

    #[test]
    fn test_clusters_multiple_distinct_patterns_ordered_by_impact() {
        let e1 = make_test_entry("e1", EntryOutcome::Failed, 500, vec![], vec![]);
        let e2 = make_test_entry("e2", EntryOutcome::Failed, 500, vec![], vec![]);
        let e3 = make_test_entry("e3", EntryOutcome::Failed, 500, vec![], vec![]);

        let diff_item = DiffItem {
            path: "/authCode".to_string(),
            diff_type: "Removed".to_string(),
            expected: Some("uuid".to_string()),
            actual: None,
            origin: ProvenanceOrigin::Derived,
        };
        let e4 = make_test_entry("e4", EntryOutcome::Failed, 200, vec![], vec![diff_item]);

        let scenario = make_test_scenario(vec![e1, e2, e3, e4]);
        let (_, clusters) = analyze_and_cluster_failures(&[scenario]);

        assert_eq!(clusters.len(), 2);
        assert_eq!(clusters[0].id, "cluster-1");
        assert_eq!(clusters[0].entry_count, 3);
        assert_eq!(clusters[0].root_cause_category, "Server Error");

        assert_eq!(clusters[1].id, "cluster-2");
        assert_eq!(clusters[1].entry_count, 1);
        assert_eq!(clusters[1].root_cause_category, "Missing Contract Field");
    }
}
