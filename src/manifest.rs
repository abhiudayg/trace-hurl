use crate::error::{Result, TraceHurlError};
use crate::model::SensitivityClass;
use hex::encode as hex_encode;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestPhase {
    pub name: String,
    pub exit_code: Option<i32>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub log_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestScenario {
    pub file: String,
    pub name: String,
    pub entry_count: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestArtifact {
    pub path: String,
    #[serde(rename = "type")]
    pub artifact_type: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub sensitivity: SensitivityClass,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ManifestRedaction {
    #[serde(default)]
    pub headers: Vec<String>,
    #[serde(default)]
    pub json_paths: Vec<String>,
    #[serde(default)]
    pub text_patterns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub schema_version: u32,
    pub run_id: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    #[serde(default)]
    pub tools: HashMap<String, String>,
    #[serde(default)]
    pub phases: Vec<ManifestPhase>,
    #[serde(default)]
    pub scenarios: Vec<ManifestScenario>,
    #[serde(default)]
    pub artifacts: Vec<ManifestArtifact>,
    #[serde(default)]
    pub variables: HashMap<String, String>,
    pub redaction: Option<ManifestRedaction>,
    pub live: Option<serde_json::Value>,
}

impl Manifest {
    pub fn load_from_dir(run_dir: &Path) -> Result<Self> {
        let manifest_path = run_dir.join("manifest.json");
        if !manifest_path.exists() {
            return Err(TraceHurlError::ManifestNotFound(manifest_path));
        }

        let content = std::fs::read_to_string(&manifest_path)?;
        let manifest: Manifest = serde_json::from_str(&content)?;
        Ok(manifest)
    }

    pub fn save_to_dir(&self, run_dir: &Path) -> Result<()> {
        let manifest_path = run_dir.join("manifest.json");
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&manifest_path, content)?;
        Ok(())
    }

    pub fn validate_artifacts(&self, run_dir: &Path) -> Result<()> {
        let canonical_run_dir = run_dir.canonicalize()?;

        for artifact in &self.artifacts {
            if artifact.path.contains("..")
                || artifact.path.starts_with('/')
                || artifact.path.starts_with('\\')
            {
                return Err(TraceHurlError::PathTraversal(artifact.path.clone()));
            }

            let full_path = run_dir.join(&artifact.path);
            if !full_path.exists() {
                return Err(TraceHurlError::ArtifactMissing(artifact.path.clone()));
            }

            let canonical_art_path = full_path.canonicalize()?;
            if !canonical_art_path.starts_with(&canonical_run_dir) {
                return Err(TraceHurlError::PathTraversal(artifact.path.clone()));
            }

            let actual_hash = compute_sha256(&full_path)?;
            if actual_hash.to_lowercase() != artifact.sha256.to_lowercase() {
                return Err(TraceHurlError::HashMismatch {
                    path: artifact.path.clone(),
                    expected: artifact.sha256.clone(),
                    actual: actual_hash,
                });
            }
        }

        Ok(())
    }
}

pub fn compute_sha256(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }

    Ok(hex_encode(hasher.finalize()))
}

pub fn synthesize_manifest(run_dir: &Path) -> Result<Manifest> {
    let run_id = run_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("RUN_UNKNOWN")
        .to_string();

    let mut artifacts = Vec::new();
    collect_artifacts_recursively(run_dir, run_dir, &mut artifacts)?;

    let mut scenarios = Vec::new();
    for art in &artifacts {
        if art.path.ends_with(".hurl") {
            let full_path = run_dir.join(&art.path);
            let name = full_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("scenario")
                .to_string();
            scenarios.push(ManifestScenario {
                file: art.path.clone(),
                name,
                entry_count: None,
            });
        }
    }

    let mut phases = Vec::new();
    let phases_tsv = run_dir.join("phases.tsv");
    if let Ok(content) = std::fs::read_to_string(&phases_tsv) {
        for line in content.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 2 {
                let name = parts[0].to_string();
                let exit_code = parts[1].parse::<i32>().ok();
                let start_time = parts.get(2).map(|s| s.to_string());
                let end_time = parts.get(3).map(|s| s.to_string());
                let log_path = parts.get(4).map(|s| s.to_string());
                phases.push(ManifestPhase {
                    name,
                    exit_code,
                    start_time,
                    end_time,
                    log_path,
                });
            }
        }
    }

    if phases.is_empty() {
        phases.push(ManifestPhase {
            name: "functional".to_string(),
            exit_code: Some(0),
            start_time: None,
            end_time: None,
            log_path: Some("functional-loan-import.err".to_string()),
        });
    }

    let mut tools = HashMap::new();
    tools.insert(
        "tracehurl".to_string(),
        env!("CARGO_PKG_VERSION").to_string(),
    );

    Ok(Manifest {
        schema_version: 1,
        run_id,
        started_at: None,
        finished_at: None,
        tools,
        phases,
        scenarios,
        artifacts,
        variables: HashMap::new(),
        redaction: Some(ManifestRedaction::default()),
        live: None,
    })
}

fn collect_artifacts_recursively(
    base_dir: &Path,
    current_dir: &Path,
    artifacts: &mut Vec<ManifestArtifact>,
) -> Result<()> {
    if !current_dir.is_dir() {
        return Ok(());
    }

    for entry in std::fs::read_dir(current_dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        if file_name.starts_with('.') || file_name == "manifest.json" {
            continue;
        }

        if path.is_dir() {
            collect_artifacts_recursively(base_dir, &path, artifacts)?;
        } else if path.is_file() {
            let rel_path = path
                .strip_prefix(base_dir)
                .map_err(|e| TraceHurlError::ValidationFailed(e.to_string()))?
                .to_string_lossy()
                .to_string();

            let metadata = entry.metadata()?;
            let size_bytes = metadata.len();
            let sha256 = compute_sha256(&path)?;

            let (artifact_type, sensitivity) = classify_artifact(&rel_path);

            artifacts.push(ManifestArtifact {
                path: rel_path,
                artifact_type,
                size_bytes,
                sha256,
                sensitivity,
            });
        }
    }

    Ok(())
}

fn classify_artifact(path: &str) -> (String, SensitivityClass) {
    if path.ends_with(".json") {
        if path.contains("payload") {
            ("payload".to_string(), SensitivityClass::Sensitive)
        } else {
            ("json-report".to_string(), SensitivityClass::Internal)
        }
    } else if path.ends_with(".hurl") {
        ("hurl-scenario".to_string(), SensitivityClass::Internal)
    } else if path.ends_with(".err") || path.ends_with(".log") {
        ("log".to_string(), SensitivityClass::Sensitive)
    } else if path.ends_with(".html") {
        ("html-report".to_string(), SensitivityClass::Internal)
    } else {
        ("raw".to_string(), SensitivityClass::Internal)
    }
}
