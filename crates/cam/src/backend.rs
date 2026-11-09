use crate::{Project, Result};

/// Boundary to a commercial CAM system. v0.1 only ships a non-producing mock.
pub trait CamBackend {
    fn name(&self) -> &'static str;
    fn synchronize_plan(&self, project: &Project) -> Result<String>;
    fn generate_toolpaths(&self, project: &Project) -> Result<()>;
    fn simulate(&self, project: &Project) -> Result<()>;
    fn postprocess_nc(&self, project: &Project) -> Result<Vec<u8>>;
}

/// Safe mock that demonstrates the integration boundary without creating toolpaths or NC data.
#[derive(Debug, Clone, Copy, Default)]
pub struct MockCamBackend;

impl CamBackend for MockCamBackend {
    fn name(&self) -> &'static str {
        "mock-solidcam"
    }

    fn synchronize_plan(&self, project: &Project) -> Result<String> {
        Ok(format!("MOCK: {} Setup(s), {} Operation(en), keine CAM-Daten erzeugt", project.setups.len(), project.operations.len()))
    }

    fn generate_toolpaths(&self, _project: &Project) -> Result<()> {
        Err(crate::CamError::NotAvailable("Werkzeugwege sind in v0.1 bewusst deaktiviert".into()))
    }

    fn simulate(&self, _project: &Project) -> Result<()> {
        Err(crate::CamError::NotAvailable("CAM-Simulation ist in v0.1 nicht implementiert".into()))
    }

    fn postprocess_nc(&self, _project: &Project) -> Result<Vec<u8>> {
        Err(crate::CamError::NotAvailable("NC-Ausgabe ist in v0.1 verboten".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Bounds, MachineSelection, ModelInfo, PROJECT_SCHEMA, Stock, Vec3};

    fn empty_project() -> Project {
        Project {
            schema: PROJECT_SCHEMA.into(),
            name: "test".into(),
            model: ModelInfo {
                source_path: "test.step".into(),
                adapter: "test".into(),
                bounds_mm: Bounds { min: Vec3 { x: 0.0, y: 0.0, z: 0.0 }, max: Vec3 { x: 1.0, y: 1.0, z: 1.0 } },
                shell_count: 1,
                face_count: 6,
                edge_count: 12,
                vertex_count: 8,
                geometry_digest: "x".into(),
            },
            stock: Stock::default(),
            features: Vec::new(),
            setups: Vec::new(),
            operations: Vec::new(),
            machine: MachineSelection { machine_id: None, controller_id: None, postprocessor_id: None },
            warnings: Vec::new(),
        }
    }

    #[test]
    fn mock_never_generates_toolpaths_or_nc() {
        let backend = MockCamBackend;
        let project = empty_project();
        assert!(backend.synchronize_plan(&project).is_ok());
        assert!(backend.generate_toolpaths(&project).is_err());
        assert!(backend.simulate(&project).is_err());
        assert!(backend.postprocess_nc(&project).is_err());
    }
}
