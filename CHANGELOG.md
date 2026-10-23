# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-10-23

### Added
- **Automated Failure Clustering & Root-Cause Analysis**: Automatically normalizes error signatures, clusters test failures into high-impact semantic categories, and adds a dedicated Root Causes tab and scenario filter dropdown (`src/clustering.rs`).
- **1-Click W3C HAR 1.2 Export**: CLI subcommand (`tracehurl export-har <RUN_DIR>`) and interactive UI header/footer buttons to export all executed requests and responses into standard HTTP Archive 1.2 format for Postman, Charles, Proxyman, and Insomnia (`src/har.rs`).
- **Cross-Run Comparison & Regression Diff**: CLI subcommand (`tracehurl compare <RUN_A> <RUN_B>`) generating regression delta reports highlighting fixed failures, new regressions, and latency regressions exceeding 50% (`src/compare.rs`).
- **Flaky Test & Retry Progression Visualizer**: Ingests intermediate attempt executions from `entry.calls` and visualizes step-by-step retry progression pills with attempt status and latency (`src/ingest.rs`).
- **Interactive cURL Replay Runner Sidecar**: Embedded HTTP sidecar server (`tracehurl serve <RUN_DIR>`) enabling 1-click in-browser cURL request re-execution (`POST /api/replay`) and side-by-side comparison modal between recorded and live responses (`src/serve.rs`).
- **agstack Verification Harness Alignment**: Re-architected `.verify/` harness adhering to agstack engineering policy (`verify.toml`, `run.sh`, `doctor.sh`, `gates.conf`, `high-risk.txt`), enforcing strict fixture immutability in `verify/` and runtime artifact isolation in `.verify/artifacts/`.

## [0.1.0] - 2026-10-23

### Added
- **Evidence-First HTML Report**: Interactive, standalone single-file HTML report built on the Google Stitch design system tokens (`#0f131c` dark theme, JetBrains Mono, Inter typography, Material Symbols).
- **Derived Wire Response Body Engine**: Automatically reconstructs response payloads from evaluated assertion actuals when Hurl `--report-json` raw files are omitted.
- **Superimposed Contract Diff Inspector**: Visualizes actual received wire responses with superimposed contract expectations, highlighting missing keys and mismatched types alongside a side-by-side split view.
- **Variable Provenance DAG**: Scenario DAG tracking variable definitions, extraction status (`ok`, `failed`, `unresolved`), and downstream dependent entry consumers.
- **Automated Test-Quality Diagnostics**: Analyzers for detecting unasserted requests (`TQ001`), weak HTTP status ranges like `>= 400` (`TQ002`), weak type-only checks (`TQ003`), undefined variables (`TQ004`), and unasserted retry calls (`TQ005`).
- **Synchronized Failure Stepper**: Jump to first failure and next/prev failure navigation that automatically updates active filters, applies active indicator rings, and scrolls failed scenario cards into view.
- **Cryptographic Artifact Integrity**: SHA-256 checksum verification and path-traversal directory escape protections for all referenced artifacts via `tracehurl validate`.
- **Universal Credential Redaction**: Automated masking of Bearer tokens, JWT patterns, cookies, and sensitive query parameters across assertions, wire headers, cURL repro commands, and exported JSON models.

### Fixed
- Fixed layout scrolling bug where `#scenarios-list` expanded the viewport height instead of scrolling vertically and independently.
- Fixed duplicate navigation options by removing redundant top header screen links in favor of contextual detail tabs with live count badges.
- Fixed initial filter chip highlight on report load so the `All` filter is prominently selected by default.
