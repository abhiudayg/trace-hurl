use std::path::Path;
use tempfile::tempdir;
use tracehurl::diff::diff_json;
use tracehurl::ingest::ingest_run_directory;
use tracehurl::manifest::{ManifestArtifact, synthesize_manifest};
use tracehurl::model::*;
use tracehurl::redaction::{RedactionConfig, Redactor};
use tracehurl::render::render_report_to_file;

fn get_test_run_dir() -> &'static Path {
    Path::new(".verify/fixtures/sample_run")
}

#[test]
fn test_ingest_and_render_real_artifacts() {
    let run_dir = get_test_run_dir();
    if !run_dir.exists() {
        return;
    }

    let run = ingest_run_directory(run_dir).expect("Ingestion must succeed");
    assert_eq!(run.run_id, "TRACEHURL_SAMPLE_001");
    assert_eq!(run.summary.overall_status, "FAILED");
    assert_eq!(run.summary.total_entries, 4);

    let temp = tempdir().expect("tempdir");
    let html_path = temp.path().join("report.html");
    render_report_to_file(&run, &html_path).expect("Render must succeed");

    assert!(html_path.exists());
    let html_content = std::fs::read_to_string(&html_path).expect("Read html");
    assert!(html_content.contains("TraceHurl"));
    assert!(html_content.contains("TRACEHURL_SAMPLE_001"));

    // Redaction check: ensure Bearer token is never leaked in the generated HTML
    assert!(!html_content.contains("secret-admin-token"));
}

#[test]
fn test_manifest_validation_and_integrity() {
    let run_dir = get_test_run_dir();
    if !run_dir.exists() {
        return;
    }

    let manifest = synthesize_manifest(run_dir).expect("Synthesize manifest");
    assert!(!manifest.artifacts.is_empty());
    assert!(manifest.validate_artifacts(run_dir).is_ok());
}

#[test]
fn test_path_traversal_detection() {
    let temp = tempdir().expect("tempdir");
    let mut manifest = synthesize_manifest(temp.path()).expect("Synthesize manifest");

    manifest.artifacts.push(ManifestArtifact {
        path: "../../../etc/passwd".to_string(),
        artifact_type: "raw".to_string(),
        size_bytes: 10,
        sha256: "0000".to_string(),
        sensitivity: SensitivityClass::Secret,
    });

    let res = manifest.validate_artifacts(temp.path());
    assert!(res.is_err());
    let err = res.unwrap_err().to_string();
    assert!(err.contains("escapes run directory"));
}

#[test]
fn test_json_diff_deep_nested_and_keys() {
    let expected = r#"{"user": {"name": "Alice", "role": "admin", "limits": [10, 20]}}"#;
    let actual = r#"{"user": {"name": "Alice", "role": "user", "limits": [10, 30, 40]}}"#;

    let diffs = diff_json(expected, actual, ProvenanceOrigin::Recorded).expect("Diff");
    assert_eq!(diffs.len(), 3);

    let role_diff = diffs.iter().find(|d| d.path == "/user/role").unwrap();
    assert_eq!(role_diff.diff_type, "Changed");
    assert_eq!(role_diff.expected.as_deref(), Some("\"admin\""));
    assert_eq!(role_diff.actual.as_deref(), Some("\"user\""));

    let arr_change = diffs.iter().find(|d| d.path == "/user/limits/1").unwrap();
    assert_eq!(arr_change.diff_type, "Changed");

    let arr_add = diffs.iter().find(|d| d.path == "/user/limits/2").unwrap();
    assert_eq!(arr_add.diff_type, "Added");
}

#[test]
fn test_redaction_strictness() {
    let redactor = Redactor::new(RedactionConfig::default());

    let header_val = "Bearer secret-token-12345";
    assert_eq!(
        redactor.redact_header_value("authorization", header_val),
        "Bearer [REDACTED]"
    );
    assert_eq!(
        redactor.redact_header_value("X-Zeta-AuthToken", "custom-token-xyz"),
        "[REDACTED]"
    );
    assert_eq!(
        redactor.redact_header_value("Cookie", "session=abcdef; user=admin"),
        "[REDACTED]"
    );

    let url = "https://example.com/api/v1?token=jwt123&safe=public";
    assert_eq!(
        redactor.redact_url(url),
        "https://example.com/api/v1?token=[REDACTED]&safe=public"
    );
}

#[test]
fn test_response_body_and_dag_captures() {
    let run_dir = get_test_run_dir();
    if !run_dir.exists() {
        return;
    }

    let run = ingest_run_directory(run_dir).expect("Ingestion must succeed");
    let scenario = &run.scenarios[0];

    // Verify DAG captures are present and populated
    assert!(!scenario.all_captures.is_empty());
    let uid = scenario
        .all_captures
        .iter()
        .find(|c| c.name == "user_id")
        .expect("user_id capture must exist");
    assert_eq!(uid.value, "usr_998124a");

    // Verify rendered HTML contains the superimposed wire diff and DAG tables
    let temp = tempdir().expect("tempdir");
    let html_path = temp.path().join("report.html");
    render_report_to_file(&run, &html_path).expect("Render must succeed");

    let html = std::fs::read_to_string(&html_path).expect("Read html");
    assert!(html.contains("id=\"diff-superimposed-container\""));
    assert!(html.contains("id=\"captures-dag-table-body\""));
    assert!(html.contains("id=\"tab-response-provenance-badge\""));
    assert!(html.contains("id=\"btn-diff-mode-superimposed\""));
}
