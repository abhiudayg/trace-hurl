use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PhaseStatus {
    Passed,
    Failed,
    Blocked,
    Skipped,
    NotRun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EntryOutcome {
    Passed,
    Failed,
    ExpectedRejection,
    Blocked,
    Skipped,
    NotExecuted,
    Weak,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CaptureStatus {
    Ok,
    Failed,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SensitivityClass {
    Public,
    Internal,
    Sensitive,
    Secret,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProvenanceOrigin {
    Recorded,
    Derived,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Provenance {
    pub source_file: String,
    pub source_location: String,
    pub parser_name: String,
    pub origin: ProvenanceOrigin,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeaderItem {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Assertion {
    pub line: usize,
    pub expression: String,
    pub comparison_target: String,
    pub expected: Option<String>,
    pub actual: Option<String>,
    pub passed: bool,
    pub message: Option<String>,
    pub origin: ProvenanceOrigin,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capture {
    pub name: String,
    pub source_entry_index: usize,
    pub value: String,
    pub status: CaptureStatus,
    pub dependent_entries: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestData {
    pub method: String,
    pub url: String,
    pub headers: Vec<HeaderItem>,
    pub query_string: Vec<HeaderItem>,
    pub body_snippet: Option<String>,
    pub body_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseData {
    pub status: u16,
    pub headers: Vec<HeaderItem>,
    pub http_version: Option<String>,
    pub body_snippet: Option<String>,
    pub body_path: Option<String>,
    #[serde(default)]
    pub is_derived: bool,
    #[serde(default)]
    pub expected_body_snippet: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffItem {
    pub path: String,
    pub diff_type: String,
    pub expected: Option<String>,
    pub actual: Option<String>,
    pub origin: ProvenanceOrigin,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: String,
    pub index: usize,
    pub scenario_id: String,
    pub source_file: String,
    pub source_line: usize,
    pub method: String,
    pub url: String,
    pub status_code: u16,
    pub outcome: EntryOutcome,
    pub duration_ms: u64,
    pub request: RequestData,
    pub response: Option<ResponseData>,
    pub assertions: Vec<Assertion>,
    pub captures_set: Vec<Capture>,
    pub captures_used: Vec<String>,
    pub diffs: Vec<DiffItem>,
    pub curl_cmd: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioSummary {
    pub total_entries: usize,
    pub passed: usize,
    pub failed: usize,
    pub expected_rejections: usize,
    pub blocked: usize,
    pub skipped: usize,
    pub weak: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scenario {
    pub id: String,
    pub source_file: String,
    pub name: String,
    pub description: String,
    pub entries: Vec<Entry>,
    pub summary: ScenarioSummary,
    #[serde(default)]
    pub all_captures: Vec<Capture>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Phase {
    pub name: String,
    pub status: PhaseStatus,
    pub exit_code: Option<i32>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub duration_ms: Option<u64>,
    pub log_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    pub path: String,
    pub artifact_type: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub sensitivity: SensitivityClass,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub code: String,
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub target_entity: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RunSummary {
    pub total_scenarios: usize,
    pub total_entries: usize,
    pub passed_entries: usize,
    pub failed_entries: usize,
    pub expected_rejections: usize,
    pub blocked_entries: usize,
    pub skipped_entries: usize,
    pub weak_entries: usize,
    pub total_diagnostics: usize,
    pub overall_status: String,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub schema_version: u32,
    pub run_id: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub tools: HashMap<String, String>,
    pub phases: Vec<Phase>,
    pub scenarios: Vec<Scenario>,
    pub artifacts: Vec<Artifact>,
    pub variables: HashMap<String, String>,
    pub diagnostics: Vec<Diagnostic>,
    pub summary: RunSummary,
}
