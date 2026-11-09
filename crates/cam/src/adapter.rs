use std::path::Path;

use crate::{Feature, ModelInfo, Result};

/// Result returned by a CAD geometry adapter.
#[derive(Debug, Clone)]
pub struct Analysis {
    pub model: ModelInfo,
    pub features: Vec<Feature>,
}

/// Read-only geometry adapter. Implementations may use SolidCraft or a native STEP parser.
pub trait CadAdapter {
    fn name(&self) -> &'static str;
    fn analyze(&self, path: &Path) -> Result<Analysis>;
}

#[derive(Debug, Clone, Default)]
pub struct ReferenceValidation {
    pub valid: usize,
    pub stale: Vec<String>,
}

/// Revalidates saved face references by both adapter ID and geometry fingerprint.
pub fn validate_references(saved: &[Feature], current: &[Feature]) -> ReferenceValidation {
    let current_refs: std::collections::BTreeMap<&str, &str> =
        current.iter().flat_map(|feature| &feature.geometry).map(|reference| (reference.id.as_str(), reference.fingerprint.as_str())).collect();
    let mut validation = ReferenceValidation::default();
    for reference in saved.iter().flat_map(|feature| &feature.geometry) {
        if current_refs.get(reference.id.as_str()).is_some_and(|fingerprint| *fingerprint == reference.fingerprint) {
            validation.valid = validation.valid.saturating_add(1);
        } else {
            validation.stale.push(reference.id.clone());
        }
    }
    validation
}

/// Independent pure-Rust STEP fallback; SolidCraft is deliberately not a single point of failure.
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeStepAdapter;

impl CadAdapter for NativeStepAdapter {
    fn name(&self) -> &'static str {
        "native-step/monstertruck-0.4.1"
    }

    fn analyze(&self, path: &Path) -> Result<Analysis> {
        crate::step::analyze_path(path, self.name())
    }
}
