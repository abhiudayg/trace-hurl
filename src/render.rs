use crate::error::Result;
use crate::model::Run;
use std::fs::File;
use std::io::Write;
use std::path::Path;

const TEMPLATE_HTML: &str = include_str!("template.html");

pub fn render_report_to_file(run: &Run, output_path: &Path) -> Result<()> {
    let html = render_report_html(run)?;
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = File::create(output_path)?;
    file.write_all(html.as_bytes())?;
    Ok(())
}

pub fn render_report_html(run: &Run) -> Result<String> {
    let run_json = serde_json::to_string(run)?;
    let escaped_run_json = run_json.replace("</script>", "<\\/script>");

    let (overall_dot_class, overall_badge_class) = if run.summary.overall_status == "PASSED" {
        (
            "bg-secondary",
            "bg-secondary/15 border border-secondary/40 text-secondary glow-green",
        )
    } else if run.summary.overall_status == "BLOCKED" {
        (
            "bg-amber-400",
            "bg-amber-500/10 text-amber-300 border border-amber-500/30",
        )
    } else {
        (
            "bg-error",
            "bg-error-container/20 border border-error/50 text-error glow-rose",
        )
    };

    let html = TEMPLATE_HTML
        .replace("__RUN_ID__", &run.run_id)
        .replace("__VERSION__", env!("CARGO_PKG_VERSION"))
        .replace("__OVERALL_STATUS__", &run.summary.overall_status)
        .replace("__OVERALL_DOT_CLASS__", overall_dot_class)
        .replace("__OVERALL_BADGE_CLASS__", overall_badge_class)
        .replace("__RUN_DATA_JSON__", &escaped_run_json);

    Ok(html)
}
