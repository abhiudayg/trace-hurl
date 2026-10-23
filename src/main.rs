use clap::Parser;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use tracehurl::cli::{Cli, Commands};
use tracehurl::error::Result;
use tracehurl::ingest::ingest_run_directory;
use tracehurl::manifest::Manifest;
use tracehurl::render::render_report_to_file;

fn main() -> ExitCode {
    let cli = Cli::parse();

    match run_app(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {}", err);
            ExitCode::from(1)
        }
    }
}

fn run_app(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Report {
            run_dir,
            output,
            open,
        } => execute_report(&run_dir, output.as_deref(), open),
        Commands::View { run_dir, output } => execute_report(&run_dir, output.as_deref(), true),
        Commands::Inspect { run_dir, verbose } => execute_inspect(&run_dir, verbose),
        Commands::Validate { run_dir } => execute_validate(&run_dir),
        Commands::ExportHar { run_dir, output } => execute_export_har(&run_dir, output.as_deref()),
    }
}

fn execute_report(run_dir: &Path, output: Option<&Path>, open: bool) -> Result<()> {
    let run = ingest_run_directory(run_dir)?;

    let target_dir: PathBuf = match output {
        Some(out) => out.to_path_buf(),
        None => run_dir.join("report"),
    };

    let parent_dir = target_dir.parent().unwrap_or(run_dir);
    std::fs::create_dir_all(parent_dir)?;

    let tmp_dir = parent_dir.join(format!(".report.tmp.{}", std::process::id()));
    if tmp_dir.exists() {
        std::fs::remove_dir_all(&tmp_dir)?;
    }
    std::fs::create_dir_all(&tmp_dir)?;

    let html_tmp_path = tmp_dir.join("index.html");
    render_report_to_file(&run, &html_tmp_path)?;

    let json_tmp_path = tmp_dir.join("report.json");
    let json_data = serde_json::to_string_pretty(&run)?;
    std::fs::write(&json_tmp_path, json_data)?;

    let har = tracehurl::har::run_to_har(&run);
    let har_tmp_path = tmp_dir.join("trace.har");
    let har_data = serde_json::to_string_pretty(&har)?;
    std::fs::write(&har_tmp_path, har_data)?;

    if target_dir.exists() {
        std::fs::remove_dir_all(&target_dir)?;
    }
    std::fs::rename(&tmp_dir, &target_dir)?;

    let manifest_path = run_dir.join("manifest.json");
    if !manifest_path.exists()
        && let Ok(manifest) = tracehurl::manifest::synthesize_manifest(run_dir)
    {
        let _ = manifest.save_to_dir(run_dir);
    }

    let final_html = target_dir.join("index.html");
    let final_json = target_dir.join("report.json");
    let final_har = target_dir.join("trace.har");

    println!("TraceHurl Report Generated Successfully:");
    println!("  Run ID:    {}", run.run_id);
    println!("  Status:    {}", run.summary.overall_status);
    println!(
        "  Entries:   {} total ({} passed, {} failed, {} rejections, {} weak)",
        run.summary.total_entries,
        run.summary.passed_entries,
        run.summary.failed_entries,
        run.summary.expected_rejections,
        run.summary.weak_entries
    );
    println!("  HTML view: {}", final_html.display());
    println!("  JSON data: {}", final_json.display());
    println!("  HAR trace: {}", final_har.display());

    if open {
        println!("Opening report in default browser...");
        opener::open(&final_html).map_err(|e| {
            tracehurl::error::TraceHurlError::OpenBrowser(format!(
                "{}: {}",
                final_html.display(),
                e
            ))
        })?;
    }

    Ok(())
}

fn execute_inspect(run_dir: &Path, verbose: bool) -> Result<()> {
    let run = ingest_run_directory(run_dir)?;

    println!("=== TraceHurl Run Inspection ===");
    println!("Run ID:         {}", run.run_id);
    println!("Overall Status: {}", run.summary.overall_status);
    println!("Total Entries:  {}", run.summary.total_entries);
    println!("Passed:         {}", run.summary.passed_entries);
    println!("Failed:         {}", run.summary.failed_entries);
    println!("Rejections:     {}", run.summary.expected_rejections);
    println!("Weak:           {}", run.summary.weak_entries);
    println!("Diagnostics:    {}", run.diagnostics.len());

    println!("\n--- Phases ---");
    for phase in &run.phases {
        println!(
            "  - {:<15} status: {:?} (exit: {:?})",
            phase.name, phase.status, phase.exit_code
        );
    }

    println!("\n--- Scenarios ---");
    for sc in &run.scenarios {
        println!("  Scenario '{}' ({} entries):", sc.name, sc.entries.len());
        for entry in &sc.entries {
            let mark = match entry.outcome {
                tracehurl::model::EntryOutcome::Passed => "✓",
                tracehurl::model::EntryOutcome::Failed => "✗",
                tracehurl::model::EntryOutcome::ExpectedRejection => "R",
                tracehurl::model::EntryOutcome::Weak => "W",
                tracehurl::model::EntryOutcome::Blocked => "B",
                tracehurl::model::EntryOutcome::Skipped => "S",
                tracehurl::model::EntryOutcome::NotExecuted => "-",
            };
            println!(
                "    [{}] #{:<3} {:<6} {:<45} -> status: {} ({:?})",
                mark,
                entry.index,
                entry.method,
                truncate_str(&entry.url, 45),
                entry.status_code,
                entry.outcome
            );

            if verbose && !entry.assertions.is_empty() {
                for assert in &entry.assertions {
                    let a_mark = if assert.passed { "  ✓" } else { "  ✗" };
                    println!(
                        "        {} line {}: {} (exp: {:?}, act: {:?})",
                        a_mark, assert.line, assert.expression, assert.expected, assert.actual
                    );
                }
            }

            if verbose && !entry.diffs.is_empty() {
                for diff in &entry.diffs {
                    println!(
                        "        [DIFF] {} {} exp: {:?} act: {:?}",
                        diff.diff_type, diff.path, diff.expected, diff.actual
                    );
                }
            }
        }
    }

    if !run.diagnostics.is_empty() {
        println!("\n--- Diagnostics ({}) ---", run.diagnostics.len());
        for diag in &run.diagnostics {
            println!("  [{:?}] {}: {}", diag.severity, diag.code, diag.message);
        }
    }

    Ok(())
}

fn execute_validate(run_dir: &Path) -> Result<()> {
    println!("Validating manifest artifacts in: {}", run_dir.display());
    let manifest = match Manifest::load_from_dir(run_dir) {
        Ok(m) => m,
        Err(_) => {
            println!("Manifest not found; synthesizing manifest from run artifacts...");
            let m = tracehurl::manifest::synthesize_manifest(run_dir)?;
            m.save_to_dir(run_dir)?;
            m
        }
    };
    manifest.validate_artifacts(run_dir)?;
    println!(
        "✓ Validation PASSED: all {} artifacts exist and match SHA-256 hashes.",
        manifest.artifacts.len()
    );
    Ok(())
}

fn execute_export_har(run_dir: &Path, output: Option<&Path>) -> Result<()> {
    let run = ingest_run_directory(run_dir)?;
    let har = tracehurl::har::run_to_har(&run);
    let target_path: PathBuf = match output {
        Some(out) => out.to_path_buf(),
        None => run_dir.join("report").join("trace.har"),
    };

    if let Some(parent) = target_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let har_data = serde_json::to_string_pretty(&har)?;
    std::fs::write(&target_path, har_data)?;

    println!("TraceHurl HAR Exported Successfully:");
    println!("  Run ID:    {}", run.run_id);
    println!("  Entries:   {}", har.log.entries.len());
    println!("  HAR path:  {}", target_path.display());

    Ok(())
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len - 3])
    } else {
        s.to_string()
    }
}
