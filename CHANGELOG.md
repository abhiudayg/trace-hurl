# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
