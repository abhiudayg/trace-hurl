use crate::diagnostics::{QualityAnalyzer, classify_entry_outcome};
use crate::diff::{diff_json, diff_scalar};
use crate::error::Result;
use crate::hurl_json::{RawHurlReport, parse_assert_message, parse_hurl_json};
use crate::hurl_source::{ParsedHurlSource, parse_hurl_source};
use crate::manifest::{Manifest, synthesize_manifest};
use crate::model::*;
use crate::redaction::{RedactionConfig, Redactor};
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub fn ingest_run_directory(run_dir: &Path) -> Result<Run> {
    let manifest = match Manifest::load_from_dir(run_dir) {
        Ok(m) => m,
        Err(_) => synthesize_manifest(run_dir)?,
    };

    let mut redaction_config = RedactionConfig::default();
    if let Some(ref r) = manifest.redaction {
        for h in &r.headers {
            redaction_config.headers.insert(h.to_ascii_lowercase());
        }
    }
    let redactor = Redactor::new(redaction_config);

    let hurl_reports = load_hurl_reports(run_dir, &manifest)?;
    let parsed_sources = load_hurl_sources(run_dir, &manifest)?;

    let mut defined_variables: HashSet<String> = manifest.variables.keys().cloned().collect();
    defined_variables.insert("run_id".to_string());
    defined_variables.insert("aion_url".to_string());
    defined_variables.insert("tenant_id".to_string());
    defined_variables.insert("source_account_id".to_string());

    let quality_analyzer = QualityAnalyzer::new(&defined_variables);

    let mut scenarios = Vec::new();
    let mut all_diagnostics = Vec::new();
    let mut request_map: HashMap<String, Vec<(usize, Option<String>)>> = HashMap::new();

    for (scenario_idx, (source_file, source_parsed)) in parsed_sources.iter().enumerate() {
        let scenario_id = format!("scenario-{}", scenario_idx + 1);

        let report = hurl_reports
            .iter()
            .find(|r| {
                r.filename
                    .as_ref()
                    .is_some_and(|f| f.ends_with(source_file) || source_file.ends_with(f))
            })
            .or_else(|| hurl_reports.first());

        let ctx = ScenarioProcessContext {
            scenario_id: &scenario_id,
            source_file,
            source_parsed,
            report,
            redactor: &redactor,
            quality_analyzer: &quality_analyzer,
            run_dir,
        };

        let (entries, scenario_diags, scenario_captures) =
            process_scenario_entries(&ctx, &mut request_map);

        all_diagnostics.extend(scenario_diags);

        let summary = compute_scenario_summary(&entries);

        scenarios.push(Scenario {
            id: scenario_id,
            source_file: source_file.clone(),
            name: source_parsed.scenario_name.clone(),
            description: source_parsed.scenario_description.clone(),
            entries,
            summary,
            all_captures: scenario_captures,
        });
    }

    all_diagnostics.extend(quality_analyzer.check_replayed_requests(&request_map));

    let (scenarios, failure_clusters) = crate::clustering::analyze_and_cluster_failures(&scenarios);

    let phases = process_phases(&manifest);
    let mut run_summary = compute_run_summary(&scenarios, &phases, all_diagnostics.len());
    run_summary.total_failure_clusters = failure_clusters.len();

    let artifacts: Vec<Artifact> = manifest
        .artifacts
        .iter()
        .map(|a| Artifact {
            path: a.path.clone(),
            artifact_type: a.artifact_type.clone(),
            size_bytes: a.size_bytes,
            sha256: a.sha256.clone(),
            sensitivity: a.sensitivity,
        })
        .collect();

    Ok(Run {
        schema_version: manifest.schema_version,
        run_id: manifest.run_id,
        started_at: manifest.started_at,
        finished_at: manifest.finished_at,
        tools: manifest.tools,
        phases,
        scenarios,
        artifacts,
        variables: manifest.variables,
        diagnostics: all_diagnostics,
        summary: run_summary,
        failure_clusters,
    })
}

fn load_hurl_reports(run_dir: &Path, manifest: &Manifest) -> Result<Vec<RawHurlReport>> {
    let mut reports = Vec::new();

    for art in &manifest.artifacts {
        if art.path.ends_with(".json")
            && (art.path.contains("functional")
                || art.path.contains("hurl")
                || art.path.contains("report"))
        {
            let full_path = run_dir.join(&art.path);
            if let Some(r_list) = try_read_hurl_report(&full_path) {
                reports.extend(r_list);
            }
        }
    }

    if reports.is_empty()
        && let Ok(entries) = std::fs::read_dir(run_dir)
    {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json")
                && let Some(r_list) = try_read_hurl_report(&path)
            {
                reports.extend(r_list);
            }
        }
    }

    Ok(reports)
}

fn try_read_hurl_report(path: &Path) -> Option<Vec<RawHurlReport>> {
    let content = std::fs::read_to_string(path).ok()?;
    parse_hurl_json(&content).ok()
}

fn load_hurl_sources(
    run_dir: &Path,
    manifest: &Manifest,
) -> Result<Vec<(String, ParsedHurlSource)>> {
    let mut sources = Vec::new();

    for sc in &manifest.scenarios {
        let full_path = run_dir.join(&sc.file);
        if let Some(source) = try_read_hurl_source(&full_path, &sc.name) {
            sources.push(source);
        }
    }

    if sources.is_empty() {
        for art in &manifest.artifacts {
            if art.path.ends_with(".hurl") {
                let full_path = run_dir.join(&art.path);
                if let Some(source) = try_read_hurl_source(&full_path, &art.path) {
                    sources.push(source);
                }
            }
        }
    }

    if sources.is_empty()
        && let Ok(entries) = std::fs::read_dir(run_dir)
    {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "hurl") {
                let rel = path.file_name().unwrap().to_string_lossy().to_string();
                if let Some(source) = try_read_hurl_source(&path, &rel) {
                    sources.push(source);
                }
            }
        }
    }

    Ok(sources)
}

fn try_read_hurl_source(path: &Path, rel: &str) -> Option<(String, ParsedHurlSource)> {
    let content = std::fs::read_to_string(path).ok()?;
    let parsed = parse_hurl_source(&content, rel);
    Some((rel.to_string(), parsed))
}

struct ScenarioProcessContext<'a> {
    pub scenario_id: &'a str,
    pub source_file: &'a str,
    pub source_parsed: &'a ParsedHurlSource,
    pub report: Option<&'a RawHurlReport>,
    pub redactor: &'a Redactor,
    pub quality_analyzer: &'a QualityAnalyzer<'a>,
    pub run_dir: &'a Path,
}

fn parse_scalar_or_json(s: &str) -> serde_json::Value {
    let trimmed = s.trim();
    if trimmed.is_empty() || trimmed == "null" {
        return serde_json::Value::Null;
    }
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
        return v;
    }
    if let Ok(n) = trimmed.parse::<i64>() {
        return serde_json::Value::Number(n.into());
    }
    if let Some(num) = trimmed
        .parse::<f64>()
        .ok()
        .and_then(serde_json::Number::from_f64)
    {
        return serde_json::Value::Number(num);
    }
    if let Ok(b) = trimmed.parse::<bool>() {
        return serde_json::Value::Bool(b);
    }
    serde_json::Value::String(trimmed.trim_matches('"').to_string())
}

fn set_json_path_value(root: &mut serde_json::Value, path_str: &str, val: serde_json::Value) {
    let clean_path = path_str
        .trim()
        .trim_start_matches("jsonpath")
        .trim()
        .trim_matches('"');
    let path = if let Some(stripped) = clean_path.strip_prefix("$.") {
        stripped
    } else if let Some(stripped) = clean_path.strip_prefix('$') {
        stripped
    } else {
        clean_path
    };

    let segments: Vec<&str> = path.split('.').filter(|s| !s.is_empty()).collect();
    if segments.is_empty() {
        return;
    }

    let mut current = root;
    for (i, seg) in segments.iter().enumerate() {
        if i == segments.len() - 1 {
            if let serde_json::Value::Object(map) = current {
                map.insert(seg.to_string(), val);
                return;
            }
        } else {
            if !current.is_object() {
                *current = serde_json::Value::Object(serde_json::Map::new());
            }
            if let serde_json::Value::Object(map) = current {
                current = map
                    .entry(seg.to_string())
                    .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
            }
        }
    }
}

fn process_scenario_entries(
    ctx: &ScenarioProcessContext<'_>,
    request_map: &mut HashMap<String, Vec<(usize, Option<String>)>>,
) -> (Vec<Entry>, Vec<Diagnostic>, Vec<Capture>) {
    let mut entries = Vec::new();
    let mut diagnostics = Vec::new();
    let mut capture_table: HashMap<String, (usize, String, CaptureStatus)> = HashMap::new();
    let mut capture_consumers: HashMap<String, Vec<usize>> = HashMap::new();

    let accounts_json = std::fs::read_to_string(ctx.run_dir.join("created-accounts.json"))
        .ok()
        .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok());
    if let Some(json) = accounts_json {
        let accs = json
            .get("accounts")
            .and_then(|a| a.as_array())
            .cloned()
            .unwrap_or_default();
        for acc in accs {
            if let (Some(name), Some(val)) =
                (acc.get("name").and_then(|n| n.as_str()), acc.get("value"))
            {
                let val_str = match val {
                    serde_json::Value::String(s) => s.clone(),
                    v => v.to_string(),
                };
                capture_table.insert(name.to_string(), (0, val_str, CaptureStatus::Ok));
            }
        }
    }

    let raw_entries = ctx.report.map(|r| &r.entries[..]).unwrap_or(&[]);

    for (entry_idx, source_entry) in ctx.source_parsed.entries.iter().enumerate() {
        let current_index = entry_idx + 1;
        let entry_id = format!("{}-entry-{}", ctx.scenario_id, current_index);

        let raw_entry = raw_entries
            .iter()
            .find(|e| e.index == current_index || e.line == source_entry.line);

        let mut assertions = Vec::new();
        let mut diffs = Vec::new();
        let mut has_failed_asserts = false;
        let mut has_weak_asserts = false;

        if let Some(raw) = raw_entry {
            for assert in &raw.asserts {
                let parsed_msg = assert.message.as_deref().map(parse_assert_message);
                let passed = assert.success;
                if !passed {
                    has_failed_asserts = true;
                }

                let expression = parsed_msg
                    .as_ref()
                    .and_then(|p| p.expression.clone())
                    .or_else(|| {
                        source_entry
                            .asserts
                            .iter()
                            .find(|a| a.line == assert.line)
                            .map(|a| a.raw.clone())
                    })
                    .unwrap_or_else(|| format!("assert on line {}", assert.line));

                let comparison_target = if expression.starts_with("jsonpath ") {
                    expression
                        .split_whitespace()
                        .nth(1)
                        .unwrap_or("jsonpath")
                        .trim_matches('"')
                        .to_string()
                } else if expression.starts_with("status ") || expression.starts_with("HTTP ") {
                    "status".to_string()
                } else {
                    "assertion".to_string()
                };

                let expected = parsed_msg.as_ref().and_then(|p| p.expected.clone());
                let actual = parsed_msg.as_ref().and_then(|p| p.actual.clone());

                if !passed && let (Some(exp), Some(act)) = (&expected, &actual) {
                    if let Some(json_diffs) = diff_json(exp, act, ProvenanceOrigin::Recorded) {
                        diffs.extend(json_diffs);
                    } else if let Some(scalar_diff) = diff_scalar(
                        &comparison_target,
                        Some(exp),
                        Some(act),
                        ProvenanceOrigin::Recorded,
                    ) {
                        diffs.push(scalar_diff);
                    }
                }

                assertions.push(Assertion {
                    line: assert.line,
                    expression,
                    comparison_target,
                    expected,
                    actual,
                    passed,
                    message: assert.message.clone(),
                    origin: ProvenanceOrigin::Recorded,
                });
            }
        }

        for sa in &source_entry.asserts {
            if sa.is_status_range || sa.is_weak_type {
                has_weak_asserts = true;
            }
        }

        let entry_diags = ctx
            .quality_analyzer
            .analyze_source_entry(source_entry, &entry_id);
        diagnostics.extend(entry_diags);

        let mut captures_set = Vec::new();
        if let Some(raw) = raw_entry {
            for cap in &raw.captures {
                let val_str = match &cap.value {
                    serde_json::Value::String(s) => s.clone(),
                    v => v.to_string(),
                };
                let status = if has_failed_asserts {
                    CaptureStatus::Failed
                } else {
                    CaptureStatus::Ok
                };

                capture_table.insert(cap.name.clone(), (current_index, val_str.clone(), status));

                captures_set.push(Capture {
                    name: cap.name.clone(),
                    source_entry_index: current_index,
                    value: ctx.redactor.redact_text(&val_str),
                    status,
                    dependent_entries: Vec::new(),
                });
            }
        }

        let mut captures_used = Vec::new();
        for var in &source_entry.variables_used {
            if capture_table.contains_key(var) {
                captures_used.push(var.clone());
                capture_consumers
                    .entry(var.clone())
                    .or_default()
                    .push(current_index);
            }
        }

        let call = raw_entry.and_then(|r| r.calls.last().or_else(|| r.calls.first()));
        let request = if let Some(c) = call {
            let mut headers = Vec::new();
            for h in &c.request.headers {
                headers.push(HeaderItem {
                    name: h.name.clone(),
                    value: ctx.redactor.redact_header_value(&h.name, &h.value),
                });
            }

            let mut body_snippet = None;
            if let Some(ref pf) = source_entry.payload_file {
                let payload_path = ctx.run_dir.join(pf);
                if let Ok(c) = std::fs::read_to_string(&payload_path) {
                    body_snippet = Some(ctx.redactor.redact_text(&c));
                }
            }

            RequestData {
                method: c.request.method.clone(),
                url: ctx.redactor.redact_url(&c.request.url),
                headers,
                query_string: c.request.query_string.clone(),
                body_snippet,
                body_path: source_entry.payload_file.clone(),
            }
        } else {
            RequestData {
                method: source_entry.method.clone(),
                url: ctx.redactor.redact_url(&source_entry.url),
                headers: Vec::new(),
                query_string: Vec::new(),
                body_snippet: None,
                body_path: source_entry.payload_file.clone(),
            }
        };

        if let Some(ref pf) = source_entry.payload_file {
            request_map
                .entry(pf.clone())
                .or_default()
                .push((current_index, request.body_snippet.clone()));
        }

        let mut actual_json = serde_json::Value::Object(serde_json::Map::new());
        let mut expected_json = serde_json::Value::Object(serde_json::Map::new());
        let mut has_json_asserts = false;

        for a in &assertions {
            if a.comparison_target.contains("jsonpath") || a.comparison_target.starts_with("$.") {
                has_json_asserts = true;
                if let Some(ref exp) = a.expected {
                    let parsed_exp = parse_scalar_or_json(exp);
                    set_json_path_value(&mut expected_json, &a.comparison_target, parsed_exp);
                }
                if a.passed {
                    if let Some(ref exp) = a.expected {
                        let parsed_val = parse_scalar_or_json(exp);
                        set_json_path_value(&mut actual_json, &a.comparison_target, parsed_val);
                    }
                } else if let Some(act) = a
                    .actual
                    .as_deref()
                    .filter(|act| *act != "none" && *act != "null")
                {
                    let parsed_act = parse_scalar_or_json(act);
                    set_json_path_value(&mut actual_json, &a.comparison_target, parsed_act);
                }
            }
        }

        let response = call.and_then(|c| {
            c.response.as_ref().map(|res| {
                let mut headers = Vec::new();
                for h in &res.headers {
                    headers.push(HeaderItem {
                        name: h.name.clone(),
                        value: ctx.redactor.redact_header_value(&h.name, &h.value),
                    });
                }

                let mut body_snippet = None;
                let mut body_path = None;
                if let Some(ref b) = res.body {
                    if let Some(path_str) = b.as_str() {
                        let potential_path = ctx.run_dir.join(path_str);
                        if potential_path.exists() {
                            if let Ok(c) = std::fs::read_to_string(&potential_path) {
                                body_snippet = Some(ctx.redactor.redact_text(&c));
                                body_path = Some(path_str.to_string());
                            }
                        } else {
                            body_snippet = Some(ctx.redactor.redact_text(path_str));
                        }
                    } else {
                        body_snippet = Some(ctx.redactor.redact_text(&b.to_string()));
                    }
                }

                let (body_snippet, is_derived) = if let Some(snip) = body_snippet {
                    (Some(snip), false)
                } else if has_json_asserts
                    && actual_json.is_object()
                    && !actual_json.as_object().unwrap().is_empty()
                {
                    let pretty = serde_json::to_string_pretty(&actual_json).ok();
                    (pretty, true)
                } else {
                    (None, false)
                };

                let expected_body_snippet = if has_json_asserts
                    && expected_json.is_object()
                    && !expected_json.as_object().unwrap().is_empty()
                {
                    serde_json::to_string_pretty(&expected_json).ok()
                } else {
                    None
                };

                ResponseData {
                    status: res.status,
                    headers,
                    http_version: res.http_version.clone(),
                    body_snippet,
                    body_path,
                    is_derived,
                    expected_body_snippet,
                }
            })
        });

        let diffs = if let Some(ref res) = response {
            if let (Some(exp_body), Some(act_body)) =
                (&res.expected_body_snippet, &res.body_snippet)
            {
                if let Some(computed) = diff_json(exp_body, act_body, ProvenanceOrigin::Derived) {
                    if !computed.is_empty() {
                        computed
                    } else {
                        diffs
                    }
                } else {
                    diffs
                }
            } else {
                diffs
            }
        } else {
            diffs
        };

        let status_code = response.as_ref().map(|r| r.status).unwrap_or(0);
        let outcome = classify_entry_outcome(
            has_failed_asserts,
            source_entry.expected_status,
            status_code,
            false,
            false,
            has_weak_asserts,
        );

        let duration_ms = raw_entry.and_then(|r| r.time).unwrap_or(0);

        let curl_cmd = raw_entry
            .and_then(|r| r.curl_cmd.as_ref())
            .map(|cmd| ctx.redactor.redact_curl_cmd(cmd));

        let retry_attempts: Vec<crate::model::RetryAttempt> = raw_entry
            .map(|r| {
                if r.calls.len() > 1 {
                    r.calls
                        .iter()
                        .enumerate()
                        .map(|(attempt_idx, c)| {
                            let call_status =
                                c.response.as_ref().map(|res| res.status).unwrap_or(0);
                            let call_duration = c
                                .timings
                                .as_ref()
                                .and_then(|t| t.total)
                                .map(|us| us / 1000)
                                .unwrap_or(0);
                            let call_outcome = if (200..400).contains(&call_status) {
                                EntryOutcome::Passed
                            } else {
                                EntryOutcome::Failed
                            };
                            let err = if call_status >= 400 {
                                Some(format!(
                                    "HTTP status {} on attempt #{}",
                                    call_status,
                                    attempt_idx + 1
                                ))
                            } else {
                                None
                            };
                            crate::model::RetryAttempt {
                                attempt_number: attempt_idx + 1,
                                status_code: call_status,
                                duration_ms: call_duration,
                                outcome: call_outcome,
                                error_message: err,
                            }
                        })
                        .collect()
                } else {
                    Vec::new()
                }
            })
            .unwrap_or_default();

        entries.push(Entry {
            id: entry_id,
            index: current_index,
            scenario_id: ctx.scenario_id.to_string(),
            source_file: ctx.source_file.to_string(),
            source_line: source_entry.line,
            method: request.method.clone(),
            url: request.url.clone(),
            status_code,
            outcome,
            duration_ms,
            request,
            response,
            assertions,
            captures_set,
            captures_used,
            diffs,
            curl_cmd,
            cluster_id: None,
            retry_attempts,
        });
    }

    for entry in &mut entries {
        for cap in &mut entry.captures_set {
            if let Some(consumers) = capture_consumers.get(&cap.name) {
                cap.dependent_entries = consumers.clone();
            }
        }
    }

    let mut all_captures = Vec::new();
    for (name, (source_idx, val_str, status)) in &capture_table {
        let deps = capture_consumers.get(name).cloned().unwrap_or_default();
        all_captures.push(Capture {
            name: name.clone(),
            source_entry_index: *source_idx,
            value: ctx.redactor.redact_text(val_str),
            status: *status,
            dependent_entries: deps,
        });
    }
    all_captures.sort_by_key(|c| c.source_entry_index);

    (entries, diagnostics, all_captures)
}

fn process_phases(manifest: &Manifest) -> Vec<Phase> {
    manifest
        .phases
        .iter()
        .map(|p| {
            let status = match p.exit_code {
                Some(0) => PhaseStatus::Passed,
                Some(2) => PhaseStatus::Blocked,
                Some(_) => PhaseStatus::Failed,
                None => PhaseStatus::NotRun,
            };

            Phase {
                name: p.name.clone(),
                status,
                exit_code: p.exit_code,
                start_time: p.start_time.clone(),
                end_time: p.end_time.clone(),
                duration_ms: None,
                log_path: p.log_path.clone(),
            }
        })
        .collect()
}

fn compute_scenario_summary(entries: &[Entry]) -> ScenarioSummary {
    let mut summary = ScenarioSummary {
        total_entries: entries.len(),
        ..Default::default()
    };

    for e in entries {
        match e.outcome {
            EntryOutcome::Passed => summary.passed += 1,
            EntryOutcome::Failed => summary.failed += 1,
            EntryOutcome::ExpectedRejection => summary.expected_rejections += 1,
            EntryOutcome::Blocked => summary.blocked += 1,
            EntryOutcome::Skipped => summary.skipped += 1,
            EntryOutcome::Weak => summary.weak += 1,
            EntryOutcome::NotExecuted => {}
        }
    }

    summary
}

fn compute_run_summary(
    scenarios: &[Scenario],
    phases: &[Phase],
    total_diagnostics: usize,
) -> RunSummary {
    let mut summary = RunSummary {
        total_scenarios: scenarios.len(),
        total_diagnostics,
        ..Default::default()
    };

    for sc in scenarios {
        summary.total_entries += sc.summary.total_entries;
        summary.passed_entries += sc.summary.passed;
        summary.failed_entries += sc.summary.failed;
        summary.expected_rejections += sc.summary.expected_rejections;
        summary.blocked_entries += sc.summary.blocked;
        summary.skipped_entries += sc.summary.skipped;
        summary.weak_entries += sc.summary.weak;
    }

    let has_phase_failure = phases.iter().any(|p| p.status == PhaseStatus::Failed);
    let has_phase_blocked = phases.iter().any(|p| p.status == PhaseStatus::Blocked);

    if has_phase_blocked || summary.blocked_entries > 0 {
        summary.overall_status = "BLOCKED".to_string();
    } else if has_phase_failure || summary.failed_entries > 0 {
        summary.overall_status = "FAILED".to_string();
    } else {
        summary.overall_status = "PASSED".to_string();
    }

    summary
}
