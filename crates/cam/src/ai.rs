use crate::{FeatureKind, Project};

/// Optional local advisor. Geometry recognition remains deterministic and never depends on it.
pub trait Advisor {
    fn name(&self) -> &'static str;
    fn advise(&self, project: &Project) -> Vec<String>;
}

/// Offline no-model mode: transparent rules only, with no network and no model download.
#[derive(Debug, Clone, Copy, Default)]
pub struct RuleAdvisor;

impl Advisor for RuleAdvisor {
    fn name(&self) -> &'static str {
        "deterministic-rule-advisor"
    }

    fn advise(&self, project: &Project) -> Vec<String> {
        let mut advice = vec!["Alle Vorschläge sind unverbindlich und müssen fachlich geprüft werden.".into()];
        if project.features.iter().any(|feature| feature.confidence < 0.75) {
            advice.push("Merkmale mit Konfidenz unter 75 % vor der Arbeitsplanung bestätigen oder umklassifizieren.".into());
        }
        if project.features.iter().any(|feature| feature.kind == FeatureKind::HoleCandidate) {
            advice.push("Bohrungskandidaten anhand von Ein-/Austrittsfläche und gewünschter Fertigungsrichtung prüfen.".into());
        }
        if project.machine.machine_id.is_none() {
            advice.push("Maschine, Steuerung, Werkzeuge und Spannmittel sind noch nicht verifiziert.".into());
        }
        advice
    }
}
