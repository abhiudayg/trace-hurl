use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TraceHurlError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Manifest file not found: {0}")]
    ManifestNotFound(PathBuf),

    #[error("Invalid manifest: {0}")]
    ManifestInvalid(String),

    #[error("Artifact not found: {0}")]
    ArtifactMissing(String),

    #[error("Artifact path escapes run directory: {0}")]
    PathTraversal(String),

    #[error("Hash mismatch for {path}: expected {expected}, actual {actual}")]
    HashMismatch {
        path: String,
        expected: String,
        actual: String,
    },

    #[error("Report generation failed: {0}")]
    ReportGeneration(String),

    #[error("Validation failed: {0}")]
    ValidationFailed(String),

    #[error("Open browser failed: {0}")]
    OpenBrowser(String),

    #[error("Sidecar server error: {0}")]
    SidecarError(String),
}

pub type Result<T> = std::result::Result<T, TraceHurlError>;
