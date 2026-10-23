use crate::model::{EntryOutcome, Run};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ComparisonReport {
    pub base_run_id: String,
    pub current_run_id: String,
    pub generated_at: String,
    pub summary: ComparisonSummary,
    pub regressions: Vec<EntryDelta>,
    pub fixed: Vec<EntryDelta>,
    pub latency_regressions: Vec<LatencyDelta>,
    pub unchanged_passed: usize,
    pub unchanged_failed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ComparisonSummary {
    pub total_common_entries: usize,
    pub new_entries: usize,
    pub removed_entries: usize,
    pub regressions_count: usize,
    pub fixed_count: usize,
    pub latency_regressions_count: usize,
    pub base_status: String,
    pub current_status: String,
    pub net_health_delta: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EntryDelta {
    pub entry_key: String,
    pub method: String,
    pub url: String,
    pub source_file: String,
    pub base_outcome: String,
    pub current_outcome: String,
    pub base_status_code: u16,
    pub current_status_code: u16,
    pub base_duration_ms: u64,
    pub current_duration_ms: u64,
    pub details: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LatencyDelta {
    pub entry_key: String,
    pub method: String,
    pub url: String,
    pub base_duration_ms: u64,
    pub current_duration_ms: u64,
    pub increase_ms: i64,
    pub increase_percentage: f64,
}

pub fn compare_runs(base: &Run, current: &Run) -> ComparisonReport {
    let mut base_map = HashMap::new();
    for scenario in &base.scenarios {
        for entry in &scenario.entries {
            let key = (
                entry.source_file.clone(),
                entry.index,
                entry.method.clone(),
                entry.url.clone(),
            );
            base_map.insert(key, entry);
        }
    }

    let mut current_map = HashMap::new();
    for scenario in &current.scenarios {
        for entry in &scenario.entries {
            let key = (
                entry.source_file.clone(),
                entry.index,
                entry.method.clone(),
                entry.url.clone(),
            );
            current_map.insert(key, entry);
        }
    }

    let mut regressions = Vec::new();
    let mut fixed = Vec::new();
    let mut latency_regressions = Vec::new();
    let mut unchanged_passed = 0;
    let mut unchanged_failed = 0;
    let mut total_common = 0;

    for (key, cur_entry) in &current_map {
        if let Some(base_entry) = base_map.get(key) {
            total_common += 1;
            let entry_key = format!(
                "{}#{} {} {}",
                cur_entry.source_file, cur_entry.index, cur_entry.method, cur_entry.url
            );

            let is_base_fail = base_entry.outcome == EntryOutcome::Failed;
            let is_cur_fail = cur_entry.outcome == EntryOutcome::Failed;
            let is_base_pass = base_entry.outcome == EntryOutcome::Passed
                || base_entry.outcome == EntryOutcome::ExpectedRejection;
            let is_cur_pass = cur_entry.outcome == EntryOutcome::Passed
                || cur_entry.outcome == EntryOutcome::ExpectedRejection;

            if is_base_pass && is_cur_fail {
                regressions.push(EntryDelta {
                    entry_key: entry_key.clone(),
                    method: cur_entry.method.clone(),
                    url: cur_entry.url.clone(),
                    source_file: cur_entry.source_file.clone(),
                    base_outcome: format!("{:?}", base_entry.outcome),
                    current_outcome: format!("{:?}", cur_entry.outcome),
                    base_status_code: base_entry.status_code,
                    current_status_code: cur_entry.status_code,
                    base_duration_ms: base_entry.duration_ms,
                    current_duration_ms: cur_entry.duration_ms,
                    details: format!(
                        "Regression: was {:?} ({}), now {:?} ({})",
                        base_entry.outcome,
                        base_entry.status_code,
                        cur_entry.outcome,
                        cur_entry.status_code
                    ),
                });
            } else if is_base_fail && is_cur_pass {
                fixed.push(EntryDelta {
                    entry_key: entry_key.clone(),
                    method: cur_entry.method.clone(),
                    url: cur_entry.url.clone(),
                    source_file: cur_entry.source_file.clone(),
                    base_outcome: format!("{:?}", base_entry.outcome),
                    current_outcome: format!("{:?}", cur_entry.outcome),
                    base_status_code: base_entry.status_code,
                    current_status_code: cur_entry.status_code,
                    base_duration_ms: base_entry.duration_ms,
                    current_duration_ms: cur_entry.duration_ms,
                    details: format!(
                        "Fixed: was {:?} ({}), resolved to {:?} ({})",
                        base_entry.outcome,
                        base_entry.status_code,
                        cur_entry.outcome,
                        cur_entry.status_code
                    ),
                });
            } else if is_base_pass && is_cur_pass {
                unchanged_passed += 1;
            } else if is_base_fail && is_cur_fail {
                unchanged_failed += 1;
            }

            if cur_entry.duration_ms > 50
                && cur_entry.duration_ms > (base_entry.duration_ms * 3 / 2)
            {
                let diff_ms = cur_entry.duration_ms as i64 - base_entry.duration_ms as i64;
                let pct = if base_entry.duration_ms > 0 {
                    (diff_ms as f64 / base_entry.duration_ms as f64) * 100.0
                } else {
                    100.0
                };
                latency_regressions.push(LatencyDelta {
                    entry_key,
                    method: cur_entry.method.clone(),
                    url: cur_entry.url.clone(),
                    base_duration_ms: base_entry.duration_ms,
                    current_duration_ms: cur_entry.duration_ms,
                    increase_ms: diff_ms,
                    increase_percentage: (pct * 10.0).round() / 10.0,
                });
            }
        }
    }

    let new_entries = current_map
        .keys()
        .filter(|k| !base_map.contains_key(k))
        .count();
    let removed_entries = base_map
        .keys()
        .filter(|k| !current_map.contains_key(k))
        .count();

    let net_health_delta = if !regressions.is_empty() {
        "REGRESSED".to_string()
    } else if !fixed.is_empty() {
        "IMPROVED".to_string()
    } else {
        "UNCHANGED".to_string()
    };

    ComparisonReport {
        base_run_id: base.run_id.clone(),
        current_run_id: current.run_id.clone(),
        generated_at: chrono::Utc::now().to_rfc3339(),
        summary: ComparisonSummary {
            total_common_entries: total_common,
            new_entries,
            removed_entries,
            regressions_count: regressions.len(),
            fixed_count: fixed.len(),
            latency_regressions_count: latency_regressions.len(),
            base_status: base.summary.overall_status.clone(),
            current_status: current.summary.overall_status.clone(),
            net_health_delta,
        },
        regressions,
        fixed,
        latency_regressions,
        unchanged_passed,
        unchanged_failed,
    }
}

pub fn render_comparison_html(report: &ComparisonReport) -> String {
    let is_regressed = report.summary.net_health_delta == "REGRESSED";
    let is_improved = report.summary.net_health_delta == "IMPROVED";

    let health_badge = if is_regressed {
        r#"<span class="px-2 py-0.5 rounded bg-error/20 border border-error/40 text-error font-bold">REGRESSED</span>"#
    } else if is_improved {
        r#"<span class="px-2 py-0.5 rounded bg-secondary/20 border border-secondary/40 text-secondary font-bold">IMPROVED</span>"#
    } else {
        r#"<span class="px-2 py-0.5 rounded bg-surface-container border border-outline-variant text-outline font-bold">UNCHANGED</span>"#
    };

    let regressions_html = if report.regressions.is_empty() {
        r#"<div class="p-4 rounded border border-outline-variant bg-surface-container-low text-on-surface-variant text-code-sm">Zero regressions detected. All previously passing entries remained intact.</div>"#.to_string()
    } else {
        let rows: Vec<String> = report
            .regressions
            .iter()
            .map(|r| {
                format!(
                    r#"<tr class="hover:bg-surface-container/40 transition-colors">
                        <td class="py-2.5 px-3 font-mono font-bold text-error">{}</td>
                        <td class="py-2.5 px-3 font-semibold text-on-surface">{} {}</td>
                        <td class="py-2.5 px-3 text-outline font-mono">{} ({}ms)</td>
                        <td class="py-2.5 px-3 font-mono text-error font-semibold">{} ({}ms)</td>
                        <td class="py-2.5 px-3 text-on-surface-variant">{}</td>
                    </tr>"#,
                    r.entry_key,
                    r.method,
                    r.url,
                    r.base_outcome,
                    r.base_duration_ms,
                    r.current_outcome,
                    r.current_duration_ms,
                    r.details
                )
            })
            .collect();
        format!(
            r#"<div class="overflow-x-auto rounded border border-outline-variant bg-surface-container-lowest">
                <table class="w-full text-left font-code-sm text-code-sm border-collapse min-w-[700px]">
                    <thead>
                        <tr class="border-b border-outline-variant bg-surface text-outline text-label-caps">
                            <th class="py-2 px-3">Entry Target</th>
                            <th class="py-2 px-3">Request</th>
                            <th class="py-2 px-3">Base Run ({})</th>
                            <th class="py-2 px-3">Current Run ({})</th>
                            <th class="py-2 px-3">Regression Details</th>
                        </tr>
                    </thead>
                    <tbody class="divide-y divide-outline-variant/40">{}</tbody>
                </table>
            </div>"#,
            report.base_run_id,
            report.current_run_id,
            rows.join("")
        )
    };

    let fixed_html = if report.fixed.is_empty() {
        r#"<div class="p-4 rounded border border-outline-variant bg-surface-container-low text-on-surface-variant text-code-sm">Zero fixes detected.</div>"#.to_string()
    } else {
        let rows: Vec<String> = report
            .fixed
            .iter()
            .map(|f| {
                format!(
                    r#"<tr class="hover:bg-surface-container/40 transition-colors">
                        <td class="py-2.5 px-3 font-mono font-bold text-secondary">{}</td>
                        <td class="py-2.5 px-3 font-semibold text-on-surface">{} {}</td>
                        <td class="py-2.5 px-3 text-error font-mono font-semibold">{}</td>
                        <td class="py-2.5 px-3 font-mono text-secondary font-bold">{}</td>
                        <td class="py-2.5 px-3 text-on-surface-variant">{}</td>
                    </tr>"#,
                    f.entry_key, f.method, f.url, f.base_outcome, f.current_outcome, f.details
                )
            })
            .collect();
        format!(
            r#"<div class="overflow-x-auto rounded border border-outline-variant bg-surface-container-lowest">
                <table class="w-full text-left font-code-sm text-code-sm border-collapse min-w-[700px]">
                    <thead>
                        <tr class="border-b border-outline-variant bg-surface text-outline text-label-caps">
                            <th class="py-2 px-3">Entry Target</th>
                            <th class="py-2 px-3">Request</th>
                            <th class="py-2 px-3">Base Run ({})</th>
                            <th class="py-2 px-3">Current Run ({})</th>
                            <th class="py-2 px-3">Resolution Details</th>
                        </tr>
                    </thead>
                    <tbody class="divide-y divide-outline-variant/40">{}</tbody>
                </table>
            </div>"#,
            report.base_run_id,
            report.current_run_id,
            rows.join("")
        )
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="en" class="dark">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>TraceHurl Compare: {} vs {}</title>
  <script src="https://cdn.tailwindcss.com"></script>
  <style>
    body {{ background-color: #0f131c; color: #e1e2ec; }}
  </style>
</head>
<body class="p-4 md:p-8 space-y-6 max-w-7xl mx-auto font-sans">
  <header class="border-b border-gray-800 pb-4 flex items-center justify-between flex-wrap gap-4">
    <div>
      <div class="flex items-center gap-3">
        <h1 class="text-2xl font-bold tracking-tight">TraceHurl Run Comparison</h1>
        {}
      </div>
      <p class="text-sm text-gray-400 mt-1">
        Comparing Base <code>{}</code> with Current <code>{}</code>
      </p>
    </div>
    <div class="flex items-center gap-3 text-sm">
      <div class="px-3 py-1 rounded bg-gray-900 border border-gray-800">
        <span class="text-gray-500">Regressions:</span> <span class="font-bold text-red-400">{}</span>
      </div>
      <div class="px-3 py-1 rounded bg-gray-900 border border-gray-800">
        <span class="text-gray-500">Fixed:</span> <span class="font-bold text-green-400">{}</span>
      </div>
      <div class="px-3 py-1 rounded bg-gray-900 border border-gray-800">
        <span class="text-gray-500">Latency Regressions:</span> <span class="font-bold text-amber-400">{}</span>
      </div>
    </div>
  </header>

  <section class="space-y-3">
    <h2 class="text-lg font-semibold flex items-center gap-2 text-red-400">
      <span>Regressions (Passed ➔ Failed)</span>
      <span class="text-xs px-2 py-0.5 rounded-full bg-red-950 text-red-300">{}</span>
    </h2>
    {}
  </section>

  <section class="space-y-3">
    <h2 class="text-lg font-semibold flex items-center gap-2 text-green-400">
      <span>Fixed Entries (Failed ➔ Passed)</span>
      <span class="text-xs px-2 py-0.5 rounded-full bg-green-950 text-green-300">{}</span>
    </h2>
    {}
  </section>
</body>
</html>"#,
        report.base_run_id,
        report.current_run_id,
        health_badge,
        report.base_run_id,
        report.current_run_id,
        report.summary.regressions_count,
        report.summary.fixed_count,
        report.summary.latency_regressions_count,
        report.summary.regressions_count,
        regressions_html,
        report.summary.fixed_count,
        fixed_html
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        Entry, EntryOutcome, HeaderItem, RequestData, Run, RunSummary, Scenario, ScenarioSummary,
    };

    fn make_entry(id: &str, outcome: EntryOutcome, duration_ms: u64, status_code: u16) -> Entry {
        Entry {
            id: id.to_string(),
            index: 1,
            scenario_id: "sc-1".to_string(),
            source_file: "test.hurl".to_string(),
            source_line: 1,
            method: "POST".to_string(),
            url: "https://api.example.com/test".to_string(),
            status_code,
            outcome,
            duration_ms,
            request: RequestData {
                method: "POST".to_string(),
                url: "https://api.example.com/test".to_string(),
                headers: vec![HeaderItem {
                    name: "Content-Type".to_string(),
                    value: "application/json".to_string(),
                }],
                query_string: vec![],
                body_snippet: None,
                body_path: None,
            },
            response: None,
            assertions: vec![],
            captures_set: vec![],
            captures_used: vec![],
            diffs: vec![],
            curl_cmd: None,
            cluster_id: None,
        }
    }

    fn make_run(run_id: &str, entries: Vec<Entry>) -> Run {
        Run {
            schema_version: 1,
            run_id: run_id.to_string(),
            started_at: None,
            finished_at: None,
            tools: HashMap::new(),
            phases: vec![],
            scenarios: vec![Scenario {
                id: "sc-1".to_string(),
                source_file: "test.hurl".to_string(),
                name: "Scenario".to_string(),
                description: "Desc".to_string(),
                entries,
                summary: ScenarioSummary::default(),
                all_captures: vec![],
            }],
            artifacts: vec![],
            variables: HashMap::new(),
            diagnostics: vec![],
            summary: RunSummary::default(),
            failure_clusters: vec![],
        }
    }

    #[test]
    fn test_compare_detects_fixed_entry() {
        let base_entry = make_entry("e1", EntryOutcome::Failed, 100, 500);
        let cur_entry = make_entry("e1", EntryOutcome::Passed, 100, 200);

        let base_run = make_run("RUN_A", vec![base_entry]);
        let cur_run = make_run("RUN_B", vec![cur_entry]);

        let report = compare_runs(&base_run, &cur_run);
        assert_eq!(report.summary.fixed_count, 1);
        assert_eq!(report.summary.regressions_count, 0);
        assert_eq!(report.summary.net_health_delta, "IMPROVED");
        assert_eq!(report.fixed[0].current_status_code, 200);
    }

    #[test]
    fn test_compare_detects_regression_entry() {
        let base_entry = make_entry("e1", EntryOutcome::Passed, 100, 200);
        let cur_entry = make_entry("e1", EntryOutcome::Failed, 100, 500);

        let base_run = make_run("RUN_A", vec![base_entry]);
        let cur_run = make_run("RUN_B", vec![cur_entry]);

        let report = compare_runs(&base_run, &cur_run);
        assert_eq!(report.summary.regressions_count, 1);
        assert_eq!(report.summary.fixed_count, 0);
        assert_eq!(report.summary.net_health_delta, "REGRESSED");
        assert_eq!(report.regressions[0].current_status_code, 500);
    }

    #[test]
    fn test_compare_detects_latency_regression_over_50_pct() {
        let base_entry = make_entry("e1", EntryOutcome::Passed, 100, 200);
        let cur_entry = make_entry("e1", EntryOutcome::Passed, 220, 200);

        let base_run = make_run("RUN_A", vec![base_entry]);
        let cur_run = make_run("RUN_B", vec![cur_entry]);

        let report = compare_runs(&base_run, &cur_run);
        assert_eq!(report.summary.latency_regressions_count, 1);
        assert_eq!(report.latency_regressions[0].increase_ms, 120);
        assert_eq!(report.latency_regressions[0].increase_percentage, 120.0);
    }
}
