use serde::{Deserialize, Serialize};

/// Current on-disk project format.
pub const PROJECT_SCHEMA: &str = "mbd-cam-project/0.1";

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    pub const X: Self = Self { x: 1.0, y: 0.0, z: 0.0 };
    pub const Y: Self = Self { x: 0.0, y: 1.0, z: 0.0 };
    pub const Z: Self = Self { x: 0.0, y: 0.0, z: 1.0 };
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Bounds {
    pub min: Vec3,
    pub max: Vec3,
}

impl Bounds {
    pub fn size(self) -> Vec3 {
        Vec3 { x: self.max.x - self.min.x, y: self.max.y - self.min.y, z: self.max.z - self.min.z }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Evidence {
    Proven,
    Inferred,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureKind {
    PlanarFace,
    ThroughHole,
    BlindHole,
    HoleCandidate,
    RectangularPocket,
    PocketCandidate,
    Slot,
    Step,
    OuterContour,
}

impl FeatureKind {
    pub const ALL: [Self; 9] = [
        Self::PlanarFace,
        Self::ThroughHole,
        Self::BlindHole,
        Self::HoleCandidate,
        Self::RectangularPocket,
        Self::PocketCandidate,
        Self::Slot,
        Self::Step,
        Self::OuterContour,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::PlanarFace => "Planfläche",
            Self::ThroughHole => "Durchgangsbohrung",
            Self::BlindHole => "Sackloch",
            Self::HoleCandidate => "Bohrungskandidat",
            Self::RectangularPocket => "Rechtecktasche",
            Self::PocketCandidate => "Taschen-Kandidat",
            Self::Slot => "Nut",
            Self::Step => "Stufe",
            Self::OuterContour => "Außenkontur",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeometryRef {
    /// Adapter-owned stable reference, e.g. `step:shell:42:face:3`.
    pub id: String,
    /// Geometry fingerprint used to detect stale references after re-import.
    pub fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Feature {
    pub id: String,
    pub kind: FeatureKind,
    pub label: String,
    pub evidence: Evidence,
    pub confidence: f32,
    pub geometry: Vec<GeometryRef>,
    pub center: Vec3,
    pub direction: Option<Vec3>,
    pub diameter_mm: Option<f64>,
    pub depth_mm: Option<f64>,
    pub width_mm: Option<f64>,
    pub length_mm: Option<f64>,
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelInfo {
    pub source_path: String,
    pub adapter: String,
    pub bounds_mm: Bounds,
    pub shell_count: usize,
    pub face_count: usize,
    pub edge_count: usize,
    pub vertex_count: usize,
    pub geometry_digest: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stock {
    pub kind: String,
    pub allowance_x_mm: f64,
    pub allowance_y_mm: f64,
    pub allowance_top_mm: f64,
    pub allowance_bottom_mm: f64,
}

impl Default for Stock {
    fn default() -> Self {
        Self { kind: "quader".into(), allowance_x_mm: 2.0, allowance_y_mm: 2.0, allowance_top_mm: 1.0, allowance_bottom_mm: 0.0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationKind {
    FaceMilling,
    Drill,
    PocketRough,
    PocketFinish,
    SlotMilling,
    Contour,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Operation {
    pub id: String,
    pub kind: OperationKind,
    pub feature_ids: Vec<String>,
    pub tool_id: Option<String>,
    pub spindle_rpm: Option<f64>,
    pub feed_mm_min: Option<f64>,
    pub axial_step_mm: Option<f64>,
    pub radial_step_mm: Option<f64>,
    pub enabled: bool,
    pub rationale: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Setup {
    pub id: String,
    pub label: String,
    pub tool_axis: Vec3,
    pub work_zero: Vec3,
    pub fixture_id: Option<String>,
    pub operation_ids: Vec<String>,
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MachineSelection {
    pub machine_id: Option<String>,
    pub controller_id: Option<String>,
    pub postprocessor_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub schema: String,
    pub name: String,
    pub model: ModelInfo,
    pub stock: Stock,
    pub features: Vec<Feature>,
    pub setups: Vec<Setup>,
    pub operations: Vec<Operation>,
    pub machine: MachineSelection,
    pub warnings: Vec<String>,
}

impl Project {
    pub fn new(name: impl Into<String>, model: ModelInfo, features: Vec<Feature>) -> Self {
        Self {
            schema: PROJECT_SCHEMA.into(),
            name: name.into(),
            model,
            stock: Stock::default(),
            features,
            setups: Vec::new(),
            operations: Vec::new(),
            machine: MachineSelection { machine_id: None, controller_id: None, postprocessor_id: None },
            warnings: vec![
                "Planungsvorschlag: Vor Fertigung vollständig durch eine CNC-Fachkraft prüfen.".into(),
                "Keine Kollisions-, Spannmittel-, Werkzeugweg- oder NC-Sicherheitsfreigabe.".into(),
            ],
        }
    }

    pub fn to_json_pretty(&self) -> crate::Result<String> {
        serde_json::to_string_pretty(self).map_err(|e| crate::CamError::Project(e.to_string()))
    }

    pub fn from_json(text: &str) -> crate::Result<Self> {
        if text.len() > 32 * 1024 * 1024 {
            return Err(crate::CamError::InputTooLarge("Projekt-JSON überschreitet 32 MB".into()));
        }
        let project: Self = serde_json::from_str(text).map_err(|e| crate::CamError::Project(e.to_string()))?;
        if project.schema != PROJECT_SCHEMA {
            return Err(crate::CamError::Project(format!("nicht unterstütztes Schema {}", project.schema)));
        }
        if project.features.len() > 100_000 || project.operations.len() > 100_000 || project.setups.len() > 10_000 {
            return Err(crate::CamError::InputTooLarge("Projekt enthält zu viele Elemente".into()));
        }
        Ok(project)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_json_round_trip_keeps_schema_and_edits() {
        let model = ModelInfo {
            source_path: "part.step".into(),
            adapter: "test".into(),
            bounds_mm: Bounds { min: Vec3 { x: 0.0, y: 0.0, z: 0.0 }, max: Vec3 { x: 10.0, y: 20.0, z: 3.0 } },
            shell_count: 1,
            face_count: 6,
            edge_count: 12,
            vertex_count: 8,
            geometry_digest: "test".into(),
        };
        let project = Project::new("Teil", model, Vec::new());
        let json = project.to_json_pretty().expect("serialization must work");
        let loaded = Project::from_json(&json).expect("round trip must work");
        assert_eq!(loaded, project);
    }
}
