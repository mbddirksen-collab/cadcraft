use std::collections::{BTreeMap, BTreeSet};

use crate::{FeatureKind, Operation, OperationKind, Project, Setup, Vec3};

#[derive(Debug, Clone, Copy)]
pub struct PlanOptions {
    pub combine_parallel_directions: bool,
}

impl Default for PlanOptions {
    fn default() -> Self {
        Self { combine_parallel_directions: true }
    }
}

pub fn plan_project(project: &mut Project, _options: PlanOptions) {
    project.setups.clear();
    project.operations.clear();
    let mut groups: BTreeMap<String, (Vec3, Vec<String>)> = BTreeMap::new();
    for feature in &project.features {
        let direction = feature.direction.unwrap_or(Vec3::Z);
        let direction = canonical_axis(direction);
        let key = direction_key(direction);
        groups.entry(key).or_insert_with(|| (direction, Vec::new())).1.push(feature.id.clone());
    }
    if groups.is_empty() {
        groups.insert("+Z".into(), (Vec3::Z, Vec::new()));
    }

    for (setup_index, (_, (axis, feature_ids))) in groups.into_iter().enumerate() {
        let setup_id = format!("setup-{}", setup_index + 1);
        let mut operation_ids = Vec::new();
        let mut seen = BTreeSet::new();
        for feature_id in feature_ids {
            let Some(feature) = project.features.iter().find(|candidate| candidate.id == feature_id) else { continue };
            for &operation_kind in operation_kinds(feature.kind) {
                let dedupe = format!("{operation_kind:?}:{}", feature.id);
                if !seen.insert(dedupe) {
                    continue;
                }
                let id = format!("op-{}", project.operations.len() + 1);
                let warning = "Schnittwerte, Werkzeug, Zustellungen und Kollisionsfreiheit sind nicht bestimmt.".into();
                project.operations.push(Operation {
                    id: id.clone(),
                    kind: operation_kind,
                    feature_ids: vec![feature.id.clone()],
                    tool_id: None,
                    spindle_rpm: None,
                    feed_mm_min: None,
                    axial_step_mm: None,
                    radial_step_mm: None,
                    enabled: true,
                    rationale: rationale(operation_kind, feature.kind),
                    warnings: vec![warning],
                });
                operation_ids.push(id);
            }
        }
        let bounds = project.model.bounds_mm;
        project.setups.push(Setup {
            id: setup_id,
            label: format!("Aufspannung {} ({})", setup_index + 1, direction_key(axis)),
            tool_axis: axis,
            work_zero: Vec3 { x: bounds.min.x, y: bounds.min.y, z: bounds.max.z },
            fixture_id: None,
            operation_ids,
            notes: "Vorschlag; Nullpunkt und Spannkonzept manuell bestätigen.".into(),
        });
    }
}

fn operation_kinds(kind: FeatureKind) -> &'static [OperationKind] {
    match kind {
        FeatureKind::PlanarFace | FeatureKind::Step => &[OperationKind::FaceMilling],
        FeatureKind::ThroughHole | FeatureKind::BlindHole | FeatureKind::HoleCandidate => &[OperationKind::Drill],
        FeatureKind::RectangularPocket | FeatureKind::PocketCandidate => &[OperationKind::PocketRough, OperationKind::PocketFinish],
        FeatureKind::Slot => &[OperationKind::SlotMilling],
        FeatureKind::OuterContour => &[OperationKind::Contour],
    }
}

fn rationale(operation: OperationKind, feature: FeatureKind) -> String {
    format!("Regelbasiert aus erkanntem Merkmal {} → Operation {operation:?}.", feature.label())
}

fn canonical_axis(direction: Vec3) -> Vec3 {
    let candidates = [
        (direction.x.abs(), Vec3 { x: direction.x.signum(), y: 0.0, z: 0.0 }),
        (direction.y.abs(), Vec3 { x: 0.0, y: direction.y.signum(), z: 0.0 }),
        (direction.z.abs(), Vec3 { x: 0.0, y: 0.0, z: direction.z.signum() }),
    ];
    candidates.into_iter().max_by(|a, b| a.0.total_cmp(&b.0)).map(|(_, axis)| axis).unwrap_or(Vec3::Z)
}

fn direction_key(direction: Vec3) -> String {
    if direction.x > 0.5 {
        "+X".into()
    } else if direction.x < -0.5 {
        "-X".into()
    } else if direction.y > 0.5 {
        "+Y".into()
    } else if direction.y < -0.5 {
        "-Y".into()
    } else if direction.z < -0.5 {
        "-Z".into()
    } else {
        "+Z".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Bounds, Evidence, Feature, GeometryRef, ModelInfo};

    #[test]
    fn side_hole_creates_second_setup_and_unset_cutting_values() {
        let model = ModelInfo {
            source_path: "part.step".into(),
            adapter: "test".into(),
            bounds_mm: Bounds { min: Vec3 { x: 0.0, y: 0.0, z: 0.0 }, max: Vec3 { x: 100.0, y: 60.0, z: 10.0 } },
            shell_count: 1,
            face_count: 1,
            edge_count: 1,
            vertex_count: 1,
            geometry_digest: "x".into(),
        };
        let feature = |id: &str, direction: Vec3| Feature {
            id: id.into(),
            kind: FeatureKind::ThroughHole,
            label: id.into(),
            evidence: Evidence::Inferred,
            confidence: 0.9,
            geometry: vec![GeometryRef { id: id.into(), fingerprint: id.into() }],
            center: Vec3 { x: 0.0, y: 0.0, z: 0.0 },
            direction: Some(direction),
            diameter_mm: Some(6.0),
            depth_mm: Some(10.0),
            width_mm: None,
            length_mm: None,
            notes: String::new(),
        };
        let mut project = Project::new("test", model, vec![feature("top", Vec3::Z), feature("side", Vec3::Y)]);
        plan_project(&mut project, PlanOptions::default());
        assert_eq!(project.setups.len(), 2);
        assert!(project.operations.iter().all(|operation| operation.tool_id.is_none() && operation.feed_mm_min.is_none()));
    }

    fn planned_operations(kind: FeatureKind) -> Vec<OperationKind> {
        let model = ModelInfo {
            source_path: "synthetic.step".into(),
            adapter: "synthetic-test".into(),
            bounds_mm: Bounds { min: Vec3 { x: 0.0, y: 0.0, z: 0.0 }, max: Vec3 { x: 80.0, y: 50.0, z: 10.0 } },
            shell_count: 1,
            face_count: 12,
            edge_count: 24,
            vertex_count: 16,
            geometry_digest: "synthetic".into(),
        };
        let feature = Feature {
            id: "feature-1".into(),
            kind,
            label: kind.label().into(),
            evidence: Evidence::Inferred,
            confidence: 0.9,
            geometry: vec![GeometryRef { id: "face-1".into(), fingerprint: "synthetic".into() }],
            center: Vec3 { x: 20.0, y: 20.0, z: 5.0 },
            direction: Some(Vec3::Z),
            diameter_mm: Some(8.0),
            depth_mm: Some(5.0),
            width_mm: Some(8.0),
            length_mm: Some(30.0),
            notes: String::new(),
        };
        let mut project = Project::new("synthetic", model, vec![feature]);
        plan_project(&mut project, PlanOptions::default());
        project.operations.into_iter().map(|operation| operation.kind).collect()
    }

    #[test]
    fn synthetic_plate_with_through_hole_plans_drilling() {
        assert_eq!(planned_operations(FeatureKind::ThroughHole), vec![OperationKind::Drill]);
    }

    #[test]
    fn synthetic_plate_with_blind_hole_plans_drilling() {
        assert_eq!(planned_operations(FeatureKind::BlindHole), vec![OperationKind::Drill]);
    }

    #[test]
    fn synthetic_rectangular_pocket_plans_roughing_and_finishing() {
        assert_eq!(planned_operations(FeatureKind::RectangularPocket), vec![OperationKind::PocketRough, OperationKind::PocketFinish]);
    }

    #[test]
    fn synthetic_slot_plans_slot_milling() {
        assert_eq!(planned_operations(FeatureKind::Slot), vec![OperationKind::SlotMilling]);
    }
}
