use std::{collections::BTreeSet, fs, path::Path};

use monstertruck::{
    meshing::prelude::*,
    modeling::{InnerSpace, ParametricSurface, ParametricSurface3D, Point3, Transform, Vector3},
    step::load::{
        Table,
        convert::StepCompressedShell,
        step_geometry::{Curve3D, ElementarySurface, Surface as StepSurface, SweepSurface},
    },
};

use crate::{Bounds, CamError, Evidence, Feature, FeatureKind, GeometryRef, ModelInfo, Result, Vec3, adapter::Analysis};

const MAX_BYTES: u64 = 256 << 20;
const MAX_SHELLS: usize = 4_096;
const MAX_FACES: usize = 500_000;
const MAX_EDGES: usize = 2_000_000;
const MAX_VERTICES: usize = 2_000_000;

#[derive(Debug, Clone)]
struct FaceInfo {
    index: usize,
    reference: GeometryRef,
    center: Vec3,
    area: f64,
    boundary_edges: usize,
    normal: Option<Vec3>,
    plane_level: Option<f64>,
    cylinder: Option<CylinderInfo>,
    neighbors: BTreeSet<usize>,
}

#[derive(Debug, Clone, Copy)]
struct CylinderInfo {
    origin: Vec3,
    axis: Vec3,
    radius: f64,
    span_min: f64,
    span_max: f64,
    internal: bool,
}

pub(super) fn analyze_path(path: &Path, adapter: &str) -> Result<Analysis> {
    let metadata = fs::metadata(path).map_err(|e| CamError::Io(e.to_string()))?;
    if metadata.len() > MAX_BYTES {
        return Err(CamError::InputTooLarge(format!("STEP-Datei überschreitet {} MB", MAX_BYTES >> 20)));
    }
    let bytes = fs::read(path).map_err(|e| CamError::Io(e.to_string()))?;
    std::panic::catch_unwind(|| analyze_bytes(&bytes, path, adapter))
        .map_err(|_| CamError::Step("interner Geometriefehler wurde abgefangen".into()))?
}

fn analyze_bytes(bytes: &[u8], path: &Path, adapter: &str) -> Result<Analysis> {
    let table = Table::from_step_bytes(bytes).map_err(|e| CamError::Step(e.to_string()))?;
    if table.shell.len() > MAX_SHELLS {
        return Err(CamError::InputTooLarge(format!("mehr als {MAX_SHELLS} STEP-Hüllen")));
    }
    let mut shell_ids: Vec<u64> = table.shell.keys().copied().collect();
    shell_ids.sort_unstable();
    let mut bounds: Option<Bounds> = None;
    let mut face_count = 0usize;
    let mut edge_count = 0usize;
    let mut vertex_count = 0usize;
    let mut features = Vec::new();

    for shell_id in &shell_ids {
        let Some(holder) = table.shell.get(shell_id) else { continue };
        let shell = table.to_compressed_shell(holder).map_err(|e| CamError::Step(format!("Hülle #{shell_id}: {e}")))?;
        face_count = face_count.checked_add(shell.faces.len()).ok_or_else(|| CamError::InputTooLarge("zu viele Flächen".into()))?;
        edge_count = edge_count.checked_add(shell.edges.len()).ok_or_else(|| CamError::InputTooLarge("zu viele Kanten".into()))?;
        vertex_count = vertex_count.checked_add(shell.vertices.len()).ok_or_else(|| CamError::InputTooLarge("zu viele Punkte".into()))?;
        if face_count > MAX_FACES || edge_count > MAX_EDGES || vertex_count > MAX_VERTICES {
            return Err(CamError::InputTooLarge("STEP-Topologie überschreitet die Analysegrenzen".into()));
        }
        for point in &shell.vertices {
            extend_bounds(&mut bounds, point)?;
        }
        let Some(global_bounds) = bounds else { continue };
        let infos = inspect_shell(*shell_id, &shell, global_bounds)?;
        recognize_shell(&infos, global_bounds, &mut features);
    }

    let bounds = bounds.ok_or_else(|| CamError::Step("keine endlichen Geometriepunkte gefunden".into()))?;
    if features.is_empty() {
        features.push(Feature {
            id: "feature-1".into(),
            kind: FeatureKind::OuterContour,
            label: "Bauteilkontur".into(),
            evidence: Evidence::Inferred,
            confidence: 0.45,
            geometry: Vec::new(),
            center: center(bounds),
            direction: Some(Vec3::Z),
            diameter_mm: None,
            depth_mm: Some(bounds.size().z.abs()),
            width_mm: Some(bounds.size().y.abs()),
            length_mm: Some(bounds.size().x.abs()),
            notes: "Keine unterstützten analytischen Merkmale; nur Hüllkörper abgeleitet.".into(),
        });
    }
    for (index, feature) in features.iter_mut().enumerate() {
        feature.id = format!("feature-{}", index + 1);
    }
    let name = path.file_stem().and_then(|value| value.to_str()).unwrap_or("STEP-Projekt");
    let model = ModelInfo {
        source_path: path.to_string_lossy().into_owned(),
        adapter: adapter.into(),
        bounds_mm: bounds,
        shell_count: shell_ids.len(),
        face_count,
        edge_count,
        vertex_count,
        geometry_digest: fnv1a_hex(bytes),
    };
    let _ = name;
    Ok(Analysis { model, features })
}

fn inspect_shell(shell_id: u64, shell: &StepCompressedShell, bounds: Bounds) -> Result<Vec<FaceInfo>> {
    let mut adjacent: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); shell.faces.len()];
    let mut edge_faces: Vec<Vec<usize>> = vec![Vec::new(); shell.edges.len()];
    for (face_index, face) in shell.faces.iter().enumerate() {
        for edge_use in face.boundaries.iter().flatten() {
            if let Some(users) = edge_faces.get_mut(edge_use.index)
                && users.len() < 64
            {
                users.push(face_index);
            }
        }
    }
    for users in edge_faces {
        for &a in &users {
            for &b in &users {
                if a != b
                    && let Some(set) = adjacent.get_mut(a)
                {
                    set.insert(b);
                }
            }
        }
    }

    let diameter = bounds.size().x.abs().max(bounds.size().y.abs()).max(bounds.size().z.abs());
    let tolerance = (diameter * 0.0005).clamp(1.0e-6, 0.1);
    let meshed = shell.robust_triangulation(tolerance);
    let mut infos = Vec::with_capacity(shell.faces.len());
    for (index, face) in shell.faces.iter().enumerate() {
        let vertices = face_vertices(shell, index);
        let face_center = points_center(&vertices).unwrap_or_else(|| center(bounds));
        let area = meshed.faces.get(index).and_then(|value| value.surface.as_ref()).map(mesh_area).unwrap_or(0.0);
        let normal = match &face.surface {
            StepSurface::ElementarySurface(ElementarySurface::Plane(plane)) => {
                let mut value = plane.normal();
                if !face.orientation {
                    value = -value;
                }
                vector(value)
            }
            _ => None,
        };
        let plane_level = match &face.surface {
            StepSurface::ElementarySurface(ElementarySurface::Plane(plane)) => finite(plane.origin().z),
            _ => None,
        };
        let cylinder = cylinder_info(shell, index, &vertices);
        let fingerprint = format!(
            "f:{}:{:.5}:{:.5}:{:.5}:{:.5}:{}",
            surface_tag(&face.surface),
            face_center.x,
            face_center.y,
            face_center.z,
            area,
            face.boundaries.iter().map(Vec::len).sum::<usize>()
        );
        infos.push(FaceInfo {
            index,
            reference: GeometryRef { id: format!("step:shell:{shell_id}:face:{index}"), fingerprint },
            center: face_center,
            area,
            boundary_edges: face.boundaries.iter().map(Vec::len).sum(),
            normal,
            plane_level,
            cylinder,
            neighbors: adjacent.get(index).cloned().unwrap_or_default(),
        });
    }
    Ok(infos)
}

fn recognize_shell(infos: &[FaceInfo], bounds: Bounds, features: &mut Vec<Feature>) {
    let size = bounds.size();
    let tol = size.x.abs().max(size.y.abs()).max(size.z.abs()).max(1.0) * 1.0e-5;
    let cylinders: Vec<&FaceInfo> = infos.iter().filter(|face| face.cylinder.is_some_and(|cylinder| cylinder.internal)).collect();
    let mut used = BTreeSet::new();

    for (offset, left) in cylinders.iter().enumerate() {
        if used.contains(&left.index) {
            continue;
        }
        let Some(a) = left.cylinder else { continue };
        for right in cylinders.iter().skip(offset + 1) {
            if used.contains(&right.index) {
                continue;
            }
            let Some(b) = right.cylinder else { continue };
            if !parallel(a.axis, b.axis, 0.995) || (a.radius - b.radius).abs() > tol.max(a.radius * 0.02) {
                continue;
            }
            let shared_wall = left
                .neighbors
                .intersection(&right.neighbors)
                .any(|neighbor| infos.get(*neighbor).and_then(|face| face.normal).is_some_and(|normal| !parallel(normal, a.axis, 0.9)));
            let center_distance = distance(left.center, right.center);
            if !shared_wall || center_distance <= a.radius * 0.5 {
                continue;
            }
            used.insert(left.index);
            used.insert(right.index);
            features.push(Feature {
                id: String::new(),
                kind: FeatureKind::Slot,
                label: "Nut-Kandidat".into(),
                evidence: Evidence::Inferred,
                confidence: 0.88,
                geometry: vec![left.reference.clone(), right.reference.clone()],
                center: midpoint(left.center, right.center),
                direction: Some(canonical_direction(a.axis)),
                diameter_mm: None,
                depth_mm: Some((a.span_max - a.span_min).abs()),
                width_mm: Some(2.0 * a.radius),
                length_mm: Some(center_distance + 2.0 * a.radius),
                notes: "Zwei koaxiale Radiusflächen mit gemeinsamen geraden Nutwänden erkannt.".into(),
            });
            break;
        }
    }

    let mut cylinder_groups: Vec<Vec<&FaceInfo>> = Vec::new();
    for face in cylinders {
        if used.contains(&face.index) {
            continue;
        }
        if let Some(group) = cylinder_groups.iter_mut().find(|group| group.first().is_some_and(|first| same_cylinder(first, face, tol))) {
            group.push(face);
        } else {
            cylinder_groups.push(vec![face]);
        }
    }
    for group in cylinder_groups {
        let Some(face) = group.first() else { continue };
        let Some(cylinder) = face.cylinder else { continue };
        let (part_min, part_max) = project_bounds(bounds, cylinder.axis, cylinder.origin);
        let through = (cylinder.span_min - part_min).abs() <= tol && (cylinder.span_max - part_max).abs() <= tol;
        let kind = if through { FeatureKind::ThroughHole } else { FeatureKind::BlindHole };
        features.push(Feature {
            id: String::new(),
            kind,
            label: kind.label().into(),
            evidence: Evidence::Inferred,
            confidence: if through { 0.94 } else { 0.86 },
            geometry: group.iter().map(|part| part.reference.clone()).collect(),
            center: Vec3 {
                x: cylinder.origin.x + cylinder.axis.x * (cylinder.span_min + cylinder.span_max) * 0.5,
                y: cylinder.origin.y + cylinder.axis.y * (cylinder.span_min + cylinder.span_max) * 0.5,
                z: cylinder.origin.z + cylinder.axis.z * (cylinder.span_min + cylinder.span_max) * 0.5,
            },
            direction: Some(canonical_direction(cylinder.axis)),
            diameter_mm: Some(2.0 * cylinder.radius),
            depth_mm: Some((cylinder.span_max - cylinder.span_min).abs()),
            width_mm: None,
            length_mm: None,
            notes: if through {
                "Konkave Zylinderfläche durchspannt die Bauteilgrenzen entlang ihrer Achse.".into()
            } else {
                "Konkave Zylinderfläche endet innerhalb der Bauteilgrenzen.".into()
            },
        });
    }

    let top = infos.iter().filter(|face| face.normal.is_some_and(|normal| normal.z > 0.95)).max_by(|a, b| a.area.total_cmp(&b.area));
    if let Some(face) = top {
        features.push(Feature {
            id: String::new(),
            kind: FeatureKind::PlanarFace,
            label: "Bezugs-/Planfläche".into(),
            evidence: Evidence::Proven,
            confidence: 1.0,
            geometry: vec![face.reference.clone()],
            center: face.center,
            direction: face.normal,
            diameter_mm: None,
            depth_mm: None,
            width_mm: None,
            length_mm: None,
            notes: format!("Exakte ebene B-Rep-Fläche; triangulierte Fläche ca. {:.3} mm².", face.area),
        });
        features.push(Feature {
            id: String::new(),
            kind: FeatureKind::OuterContour,
            label: "Außenkontur-Kandidat".into(),
            evidence: Evidence::Inferred,
            confidence: 0.72,
            geometry: vec![face.reference.clone()],
            center: face.center,
            direction: face.normal,
            diameter_mm: None,
            depth_mm: Some(size.z.abs()),
            width_mm: Some(size.y.abs()),
            length_mm: Some(size.x.abs()),
            notes: "Aus größter +Z-Planfläche und Bauteilhülle abgeleitet.".into(),
        });
    }

    for face in infos {
        let Some(normal) = face.normal else { continue };
        let Some(level) = face.plane_level else { continue };
        if normal.z > 0.95 && level > bounds.min.z + tol && level < bounds.max.z - tol && face.boundary_edges == 4 {
            features.push(Feature {
                id: String::new(),
                kind: FeatureKind::RectangularPocket,
                label: "Rechtecktaschen-Kandidat".into(),
                evidence: Evidence::Inferred,
                confidence: 0.78,
                geometry: vec![face.reference.clone()],
                center: face.center,
                direction: Some(Vec3::Z),
                diameter_mm: None,
                depth_mm: Some(bounds.max.z - level),
                width_mm: None,
                length_mm: None,
                notes: "Innenliegende +Z-Planfläche mit vier Begrenzungskanten; offene Stufe manuell ausschließen.".into(),
            });
        }
    }
}

fn same_cylinder(a: &FaceInfo, b: &FaceInfo, tolerance: f64) -> bool {
    let (Some(a), Some(b)) = (a.cylinder, b.cylinder) else { return false };
    if !parallel(a.axis, b.axis, 0.999) || (a.radius - b.radius).abs() > tolerance.max(a.radius * 0.01) {
        return false;
    }
    let delta = Vec3 { x: b.origin.x - a.origin.x, y: b.origin.y - a.origin.y, z: b.origin.z - a.origin.z };
    let along = delta.x * a.axis.x + delta.y * a.axis.y + delta.z * a.axis.z;
    let radial = Vec3 { x: delta.x - along * a.axis.x, y: delta.y - along * a.axis.y, z: delta.z - along * a.axis.z };
    let line_distance = (radial.x * radial.x + radial.y * radial.y + radial.z * radial.z).sqrt();
    if line_distance > tolerance.max(a.radius * 0.01) {
        return false;
    }
    let (a0, a1) = world_span(a, a.axis);
    let (b0, b1) = world_span(b, a.axis);
    (a0 - b0).abs() <= tolerance && (a1 - b1).abs() <= tolerance
}

fn world_span(cylinder: CylinderInfo, axis: Vec3) -> (f64, f64) {
    let origin = cylinder.origin.x * axis.x + cylinder.origin.y * axis.y + cylinder.origin.z * axis.z;
    let alignment = cylinder.axis.x * axis.x + cylinder.axis.y * axis.y + cylinder.axis.z * axis.z;
    let a = origin + cylinder.span_min * alignment;
    let b = origin + cylinder.span_max * alignment;
    (a.min(b), a.max(b))
}

fn cylinder_info(shell: &StepCompressedShell, face_index: usize, vertices: &[Point3]) -> Option<CylinderInfo> {
    let face = shell.faces.get(face_index)?;
    let analytic = match &face.surface {
        StepSurface::ElementarySurface(ElementarySurface::CylindricalSurface(processor)) => {
            let revolution = processor.entity();
            let line = revolution.entity_curve();
            let transform = processor.transform();
            Some((
                transform.transform_point(line.0),
                transform.transform_point(line.1),
                transform.transform_point(revolution.origin()),
                transform.transform_vector(revolution.axis()),
                processor.subs(0.5, 0.0),
                processor.normal(0.5, 0.0),
            ))
        }
        StepSurface::SweepSurface(SweepSurface::RevolutionSurface(processor)) => {
            let revolution = processor.entity();
            let Curve3D::Line(line) = revolution.entity_curve() else { return None };
            let transform = processor.transform();
            Some((
                transform.transform_point(line.0),
                transform.transform_point(line.1),
                transform.transform_point(revolution.origin()),
                transform.transform_vector(revolution.axis()),
                processor.subs(0.5, 0.0),
                processor.normal(0.5, 0.0),
            ))
        }
        _ => None,
    };
    if analytic.is_none() {
        return spline_cylinder(&face.surface, face.orientation, vertices);
    }
    let (p0, p1, origin, axis, sample, mut normal) = analytic?;
    if !p0.x.is_finite() || !p0.y.is_finite() || !p0.z.is_finite() || axis.magnitude2() <= 1.0e-20 {
        return None;
    }
    let axis = axis.normalize();
    let generator = p1 - p0;
    if generator.magnitude2() <= 1.0e-20 || generator.normalize().dot(axis).abs() < 0.999 {
        return None;
    }
    let radial = p0 - origin - axis * (p0 - origin).dot(axis);
    let radius = radial.magnitude();
    if !radius.is_finite() || radius <= 1.0e-9 {
        return None;
    }
    let mut span_min = f64::INFINITY;
    let mut span_max = f64::NEG_INFINITY;
    for point in vertices {
        let projection = (*point - origin).dot(axis);
        if projection.is_finite() {
            span_min = span_min.min(projection);
            span_max = span_max.max(projection);
        }
    }
    if !span_min.is_finite() || !span_max.is_finite() {
        return None;
    }
    if !face.orientation {
        normal = -normal;
    }
    let sample_radial = sample - origin - axis * (sample - origin).dot(axis);
    let internal = sample_radial.dot(normal) < 0.0;
    Some(CylinderInfo { origin: point(origin)?, axis: vector(axis)?, radius, span_min, span_max, internal })
}

/// Recognizes a tensor-product spline that is straight in one parameter and circular in the
/// other. Some valid STEP exporters encode cylinders this way instead of CYLINDRICAL_SURFACE.
fn spline_cylinder(surface: &StepSurface, orientation: bool, vertices: &[Point3]) -> Option<CylinderInfo> {
    if !matches!(surface, StepSurface::BsplineSurface(_) | StepSurface::NurbsSurface(_)) {
        return None;
    }
    let (Some((u0, u1)), Some((v0, v1))) = surface.try_range_tuple() else { return None };
    let u = u0 + (u1 - u0) * 0.43;
    let v = v0 + (v1 - v0) * 0.47;
    let sample = surface.subs(u, v);
    let du = surface.derivative_u(u, v);
    let dv = surface.derivative_v(u, v);
    let duu = surface.derivative_uu(u, v);
    let dvv = surface.derivative_vv(u, v);
    let curvature_u = curve_curvature(du, duu)?;
    let curvature_v = curve_curvature(dv, dvv)?;
    let threshold = 1.0e-7;
    let (axis, curved_tangent, curved_second, curvature) = if curvature_u < threshold && curvature_v >= threshold {
        (du, dv, dvv, curvature_v)
    } else if curvature_v < threshold && curvature_u >= threshold {
        (dv, du, duu, curvature_u)
    } else {
        return None;
    };
    if axis.magnitude2() <= 1.0e-20 || curved_tangent.magnitude2() <= 1.0e-20 {
        return None;
    }
    let axis = axis.normalize();
    let radius = 1.0 / curvature;
    if !radius.is_finite() || radius <= 1.0e-8 {
        return None;
    }
    let tangent = curved_tangent.normalize();
    let center_direction = curved_second - tangent * curved_second.dot(tangent);
    if center_direction.magnitude2() <= 1.0e-20 {
        return None;
    }
    let center = sample + center_direction.normalize() * radius;
    let mut normal = surface.normal(u, v);
    if !orientation {
        normal = -normal;
    }
    let radial = sample - center - axis * (sample - center).dot(axis);
    let internal = radial.dot(normal) < 0.0;
    let mut span_min = f64::INFINITY;
    let mut span_max = f64::NEG_INFINITY;
    for vertex in vertices {
        let projection = (*vertex - center).dot(axis);
        if projection.is_finite() {
            span_min = span_min.min(projection);
            span_max = span_max.max(projection);
        }
    }
    if !span_min.is_finite() || !span_max.is_finite() {
        return None;
    }
    Some(CylinderInfo { origin: point(center)?, axis: vector(axis)?, radius, span_min, span_max, internal })
}

fn curve_curvature(tangent: Vector3, second: Vector3) -> Option<f64> {
    let speed = tangent.magnitude();
    if !speed.is_finite() || speed <= 1.0e-10 {
        return None;
    }
    let value = tangent.cross(second).magnitude() / speed.powi(3);
    value.is_finite().then_some(value)
}

fn face_vertices(shell: &StepCompressedShell, face_index: usize) -> Vec<Point3> {
    let Some(face) = shell.faces.get(face_index) else { return Vec::new() };
    let mut indexes = BTreeSet::new();
    for edge_use in face.boundaries.iter().flatten() {
        if let Some(edge) = shell.edges.get(edge_use.index) {
            indexes.insert(edge.vertices.0);
            indexes.insert(edge.vertices.1);
        }
    }
    indexes.into_iter().filter_map(|index| shell.vertices.get(index)).collect()
}

fn mesh_area(mesh: &PolygonMesh) -> f64 {
    let positions = mesh.positions();
    let triangle = |a: usize, b: usize, c: usize| -> f64 {
        let Some(a) = positions.get(a) else { return 0.0 };
        let Some(b) = positions.get(b) else { return 0.0 };
        let Some(c) = positions.get(c) else { return 0.0 };
        0.5 * (b - a).cross(c - a).magnitude()
    };
    let tri_area: f64 = mesh.tri_faces().iter().map(|face| triangle(face[0].pos, face[1].pos, face[2].pos)).sum();
    let quad_area: f64 =
        mesh.quad_faces().iter().map(|face| triangle(face[0].pos, face[1].pos, face[2].pos) + triangle(face[0].pos, face[2].pos, face[3].pos)).sum();
    tri_area + quad_area
}

fn extend_bounds(bounds: &mut Option<Bounds>, value: &Point3) -> Result<()> {
    let point = point(*value).ok_or_else(|| CamError::Step("nicht endlicher Punkt".into()))?;
    match bounds {
        Some(bounds) => {
            bounds.min.x = bounds.min.x.min(point.x);
            bounds.min.y = bounds.min.y.min(point.y);
            bounds.min.z = bounds.min.z.min(point.z);
            bounds.max.x = bounds.max.x.max(point.x);
            bounds.max.y = bounds.max.y.max(point.y);
            bounds.max.z = bounds.max.z.max(point.z);
        }
        None => *bounds = Some(Bounds { min: point, max: point }),
    }
    Ok(())
}

fn points_center(points: &[Point3]) -> Option<Vec3> {
    if points.is_empty() {
        return None;
    }
    let mut sum = Vec3 { x: 0.0, y: 0.0, z: 0.0 };
    for value in points {
        let value = point(*value)?;
        sum.x += value.x;
        sum.y += value.y;
        sum.z += value.z;
    }
    let count = points.len() as f64;
    Some(Vec3 { x: sum.x / count, y: sum.y / count, z: sum.z / count })
}

fn point(value: Point3) -> Option<Vec3> {
    (value.x.is_finite() && value.y.is_finite() && value.z.is_finite()).then_some(Vec3 { x: value.x, y: value.y, z: value.z })
}

fn vector(value: Vector3) -> Option<Vec3> {
    (value.x.is_finite() && value.y.is_finite() && value.z.is_finite()).then_some(Vec3 { x: value.x, y: value.y, z: value.z })
}

fn finite(value: f64) -> Option<f64> {
    value.is_finite().then_some(value)
}

fn center(bounds: Bounds) -> Vec3 {
    midpoint(bounds.min, bounds.max)
}

fn midpoint(a: Vec3, b: Vec3) -> Vec3 {
    Vec3 { x: (a.x + b.x) * 0.5, y: (a.y + b.y) * 0.5, z: (a.z + b.z) * 0.5 }
}

fn distance(a: Vec3, b: Vec3) -> f64 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2) + (a.z - b.z).powi(2)).sqrt()
}

fn parallel(a: Vec3, b: Vec3, threshold: f64) -> bool {
    let dot = a.x * b.x + a.y * b.y + a.z * b.z;
    let ma = (a.x * a.x + a.y * a.y + a.z * a.z).sqrt();
    let mb = (b.x * b.x + b.y * b.y + b.z * b.z).sqrt();
    ma > 0.0 && mb > 0.0 && (dot / (ma * mb)).abs() >= threshold
}

fn canonical_direction(axis: Vec3) -> Vec3 {
    let mut axis = axis;
    let dominant = if axis.x.abs() >= axis.y.abs() && axis.x.abs() >= axis.z.abs() {
        axis.x
    } else if axis.y.abs() >= axis.z.abs() {
        axis.y
    } else {
        axis.z
    };
    if dominant < 0.0 {
        axis = Vec3 { x: -axis.x, y: -axis.y, z: -axis.z };
    }
    axis
}

fn project_bounds(bounds: Bounds, axis: Vec3, origin: Vec3) -> (f64, f64) {
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for x in [bounds.min.x, bounds.max.x] {
        for y in [bounds.min.y, bounds.max.y] {
            for z in [bounds.min.z, bounds.max.z] {
                let value = (x - origin.x) * axis.x + (y - origin.y) * axis.y + (z - origin.z) * axis.z;
                min = min.min(value);
                max = max.max(value);
            }
        }
    }
    (min, max)
}

fn surface_tag(surface: &StepSurface) -> &'static str {
    match surface {
        StepSurface::ElementarySurface(ElementarySurface::Plane(_)) => "plane",
        StepSurface::ElementarySurface(ElementarySurface::CylindricalSurface(_)) => "cylinder",
        StepSurface::ElementarySurface(ElementarySurface::ConicalSurface(_)) => "cone",
        StepSurface::ElementarySurface(ElementarySurface::Sphere(_)) => "sphere",
        StepSurface::ElementarySurface(ElementarySurface::ToroidalSurface(_)) => "torus",
        StepSurface::SweepSurface(SweepSurface::RevolutionSurface(_)) => "revolution",
        StepSurface::SweepSurface(SweepSurface::ExtrusionSurface(_)) => "extrusion",
        StepSurface::BsplineSurface(_) => "bspline",
        StepSurface::NurbsSurface(_) => "nurbs",
    }
}

fn fnv1a_hex(bytes: &[u8]) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("fnv1a64:{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use monstertruck::{
        modeling::{BoundingBox, Solid, primitive},
        step::save::{CompleteStepDisplay, StepHeaderDescriptor, StepModel},
    };

    #[test]
    fn generated_cuboid_step_has_exact_bounds_and_planar_feature() {
        let cube: Solid = primitive::cuboid(BoundingBox::from_iter([Point3::new(0.0, 0.0, 0.0), Point3::new(40.0, 25.0, 8.0)]));
        let step = CompleteStepDisplay::new(
            StepModel::from(&cube.compress()),
            StepHeaderDescriptor { organization_system: "MBD CAM AI synthetic test".into(), ..StepHeaderDescriptor::default() },
        )
        .to_string();
        let analysis = analyze_bytes(step.as_bytes(), Path::new("synthetic-cuboid.step"), "test").expect("synthetic STEP must analyze");
        assert_eq!(analysis.model.bounds_mm.min, Vec3 { x: 0.0, y: 0.0, z: 0.0 });
        assert_eq!(analysis.model.bounds_mm.max, Vec3 { x: 40.0, y: 25.0, z: 8.0 });
        assert!(analysis.features.iter().any(|feature| feature.kind == FeatureKind::PlanarFace));
        assert!(analysis.features.iter().any(|feature| feature.kind == FeatureKind::OuterContour));
    }

    #[test]
    fn hostile_step_bytes_never_escape_as_panic() {
        let garbage = vec![0xff; 4096];
        let result = std::panic::catch_unwind(|| analyze_bytes(&garbage, Path::new("hostile.step"), "test"));
        assert!(result.is_ok());
        assert!(result.ok().is_some_and(|inner| inner.is_err()));
    }
}
