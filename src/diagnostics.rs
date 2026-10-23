use crate::hurl_source::HurlSourceEntry;
use crate::model::{Diagnostic, DiagnosticSeverity, EntryOutcome};
use std::collections::{HashMap, HashSet};

pub struct QualityAnalyzer<'a> {
    defined_variables: &'a HashSet<String>,
}

impl<'a> QualityAnalyzer<'a> {
    pub fn new(defined_variables: &'a HashSet<String>) -> Self {
        Self { defined_variables }
    }

    pub fn analyze_source_entry(&self, entry: &HurlSourceEntry, entry_id: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        if entry.asserts.is_empty() {
            diagnostics.push(Diagnostic {
                code: "TQ001_NO_ASSERTIONS".to_string(),
                severity: DiagnosticSeverity::Warning,
                message: format!("Entry at line {} has no assertions", entry.line),
                target_entity: Some(entry_id.to_string()),
            });
        }

        for assert in &entry.asserts {
            if assert.is_status_range {
                diagnostics.push(Diagnostic {
                    code: "TQ002_STATUS_RANGE_ASSERTION".to_string(),
                    severity: DiagnosticSeverity::Warning,
                    message: format!(
                        "Negative entry at line {} asserts status range (e.g. >= 400) instead of exact error code/body",
                        assert.line
                    ),
                    target_entity: Some(entry_id.to_string()),
                });
            }

            if assert.is_weak_type {
                diagnostics.push(Diagnostic {
                    code: "TQ003_WEAK_TYPE_ASSERTION".to_string(),
                    severity: DiagnosticSeverity::Info,
                    message: format!(
                        "Assertion at line {} checks only type without validating specific expected value",
                        assert.line
                    ),
                    target_entity: Some(entry_id.to_string()),
                });
            }
        }

        for var in &entry.variables_used {
            if !self.defined_variables.is_empty() && !self.defined_variables.contains(var) {
                diagnostics.push(Diagnostic {
                    code: "TQ004_UNDEFINED_VARIABLE".to_string(),
                    severity: DiagnosticSeverity::Warning,
                    message: format!(
                        "Variable '{}' referenced at line {} is not in run variables",
                        var, entry.line
                    ),
                    target_entity: Some(entry_id.to_string()),
                });
            }
        }

        if entry.is_retry && entry.asserts.is_empty() {
            diagnostics.push(Diagnostic {
                code: "TQ005_UNASSERTED_RETRY".to_string(),
                severity: DiagnosticSeverity::Warning,
                message: format!(
                    "Entry with retry option at line {} has no assertions on effect",
                    entry.line
                ),
                target_entity: Some(entry_id.to_string()),
            });
        }

        diagnostics
    }

    pub fn check_replayed_requests(
        &self,
        request_map: &HashMap<String, Vec<(usize, Option<String>)>>,
    ) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        for (req_id, occurrences) in request_map {
            if occurrences.len() > 1 {
                let first_body = &occurrences[0].1;
                for (idx, body) in &occurrences[1..] {
                    if body != first_body {
                        diagnostics.push(Diagnostic {
                            code: "TQ006_REUSED_REQUEST_ID_BODY_MUTATION".to_string(),
                            severity: DiagnosticSeverity::Info,
                            message: format!(
                                "Request ID '{}' was reused at entry #{} with a different payload body",
                                req_id, idx
                            ),
                            target_entity: Some(format!("entry-{}", idx)),
                        });
                    }
                }
            }
        }

        diagnostics
    }
}

pub fn classify_entry_outcome(
    has_failed_asserts: bool,
    expected_status: Option<u16>,
    actual_status: u16,
    is_blocked: bool,
    is_skipped: bool,
    has_weak_asserts: bool,
) -> EntryOutcome {
    if is_blocked {
        return EntryOutcome::Blocked;
    }
    if is_skipped {
        return EntryOutcome::Skipped;
    }

    if expected_status
        .is_some_and(|exp_st| exp_st >= 400 && actual_status == exp_st && !has_failed_asserts)
    {
        return EntryOutcome::ExpectedRejection;
    }

    if has_failed_asserts {
        return EntryOutcome::Failed;
    }

    if has_weak_asserts {
        return EntryOutcome::Weak;
    }

    EntryOutcome::Passed
}
