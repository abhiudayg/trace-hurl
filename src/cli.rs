use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "tracehurl",
    version,
    about = "Evidence-first verification report generator for Hurl and test suites"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Generate an evidence-first interactive HTML report and machine-readable JSON model
    Report {
        /// Path to the verification run directory
        run_dir: PathBuf,

        /// Output directory for generated report (default: <run-dir>/report)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Open the report in default web browser upon generation
        #[arg(long)]
        open: bool,
    },

    /// Inspect phases, scenarios, entries, captures, and diagnostics without generating HTML
    Inspect {
        /// Path to the verification run directory
        run_dir: PathBuf,

        /// Print detailed entry information and diffs
        #[arg(short, long)]
        verbose: bool,
    },

    /// Validate that every artifact in the run manifest exists and matches its hash
    Validate {
        /// Path to the verification run directory
        run_dir: PathBuf,
    },

    /// Export all executed HTTP requests and responses as a standard HTTP Archive (HAR 1.2) file
    ExportHar {
        /// Path to the verification run directory
        run_dir: PathBuf,

        /// Output path for the .har file (default: <run-dir>/report/trace.har)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Compare two verification runs to detect regressions, fixes, and latency deltas
    Compare {
        /// Base run directory (e.g. prior baseline or main branch)
        run_a: PathBuf,

        /// Current run directory (e.g. current branch or fix attempt)
        run_b: PathBuf,

        /// Output directory for comparison delta report (default: <run-b>/comparison)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Generate the report and immediately view it in the default browser
    View {
        /// Path to the verification run directory
        run_dir: PathBuf,

        /// Output directory for generated report
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}
