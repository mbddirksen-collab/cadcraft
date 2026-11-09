//! MBD CAM AI — fully local, editable CAM planning assistant.
#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod database;
mod solidcraft;

use std::path::{Path, PathBuf};

use cadcraft_cam::CamBackend as _;
use cadcraft_cam::{
    Advisor, CadAdapter, FeatureKind, MockCamBackend, NativeStepAdapter, PlanOptions, Project, RuleAdvisor, plan_project, validate_references,
};

fn main() {
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(run));
    let error = match outcome {
        Ok(Ok(())) => return,
        Ok(Err(error)) => format!("MBD CAM AI: {error}"),
        Err(_) => "MBD CAM AI: interner Fehler wurde am Anwendungsrand abgefangen".into(),
    };
    eprintln!("{error}");
    if let Err(write_error) = std::fs::write(std::env::temp_dir().join("mbd-cam-last-error.txt"), format!("{error}\n")) {
        eprintln!("Fehlerprotokoll konnte nicht geschrieben werden: {write_error}");
    }
    std::process::exit(1);
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None | Some("--gui") => run_gui(),
        Some("--help" | "-h") => {
            print_help();
            Ok(())
        }
        Some("analyze" | "plan") => run_analyze(&args),
        Some("db-init") => {
            let path = args.get(1).map(PathBuf::from).unwrap_or_else(database::default_path);
            database::initialize(&path)?;
            println!("Datenbank initialisiert: {}", path.display());
            Ok(())
        }
        Some("solidcraft-check") => {
            let step = args.get(1).ok_or_else(|| "STEP-Datei fehlt".to_owned())?;
            let cli = args.get(2).map(PathBuf::from).unwrap_or_else(default_solidcraft_cli);
            let report = solidcraft::inspect(&cli, Path::new(step))?;
            println!("{}", serde_json::to_string_pretty(&report).map_err(|error| error.to_string())?);
            Ok(())
        }
        Some(other) => Err(format!("unbekannter Befehl {other:?}; --help zeigt die Befehle")),
    }
}

fn print_help() {
    println!(
        "MBD CAM AI 0.1\n\n\
         mbd-cam                         GUI starten\n\
         mbd-cam analyze PART.step [OUT.json]\n\
         mbd-cam plan PART.step [OUT.json]\n\
         mbd-cam solidcraft-check PART.step [solidcraft-cli.exe]\n\
         mbd-cam db-init [manufacturing.sqlite]\n\n\
         Vollständig offline. Keine Werkzeugwege, keine NC-Ausgabe, keine Maschinenfreigabe."
    );
}

fn run_analyze(args: &[String]) -> Result<(), String> {
    let path = args.get(1).map(PathBuf::from).ok_or_else(|| "STEP-Datei fehlt".to_owned())?;
    let adapter = NativeStepAdapter;
    let analysis = adapter.analyze(&path).map_err(|error| error.to_string())?;
    let name = path.file_stem().and_then(|value| value.to_str()).unwrap_or("STEP-Projekt");
    let mut project = Project::new(name, analysis.model, analysis.features);
    if args.first().is_some_and(|command| command == "plan") {
        plan_project(&mut project, PlanOptions::default());
    }
    let json = project.to_json_pretty().map_err(|error| error.to_string())?;
    if let Some(output) = args.get(2) {
        std::fs::write(output, json).map_err(|error| error.to_string())?;
        println!("Projekt gespeichert: {output}");
    } else {
        println!("{json}");
    }
    Ok(())
}

fn run_gui() -> Result<(), String> {
    let mut wgpu_options = eframe::WgpuConfiguration::default();
    if let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = &mut wgpu_options.wgpu_setup {
        // The workstation's Intel DX12 path terminates natively during surface creation. The UI
        // is 2D, so the GL backend is the conservative default. Vulkan and DX12 remain explicit
        // escape hatches for systems where their drivers are known to be stable.
        setup.instance_descriptor.backends = match std::env::var("MBD_CAM_WGPU_BACKEND").as_deref() {
            Ok("vulkan") => eframe::wgpu::Backends::VULKAN,
            Ok("dx12") => eframe::wgpu::Backends::DX12,
            _ => eframe::wgpu::Backends::GL,
        };
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_title("MBD CAM AI 0.1").with_inner_size([1380.0, 860.0]).with_min_inner_size([980.0, 640.0]),
        wgpu_options,
        ..Default::default()
    };
    eframe::run_native("MBD CAM AI", options, Box::new(|creation| Ok(Box::new(MbdCamApp::new(&creation.egui_ctx)))))
        .map_err(|error| error.to_string())
}

struct MbdCamApp {
    project: Option<Project>,
    selected_feature: Option<usize>,
    status: String,
    solidcraft_cli: String,
    solidcraft_report: Option<solidcraft::SolidCraftReport>,
    database_path: PathBuf,
    advisor_messages: Vec<String>,
    solidcraft_task: Option<std::sync::mpsc::Receiver<Result<solidcraft::SolidCraftReport, String>>>,
}

impl MbdCamApp {
    fn new(context: &egui::Context) -> Self {
        context.set_visuals(egui::Visuals::light());
        let database_path = database::default_path();
        let status = match database::initialize(&database_path) {
            Ok(()) => format!("Bereit · lokale Datenbank {}", database_path.display()),
            Err(error) => format!("Datenbank konnte nicht initialisiert werden: {error}"),
        };
        Self {
            project: None,
            selected_feature: None,
            status,
            solidcraft_cli: default_solidcraft_cli().to_string_lossy().into_owned(),
            solidcraft_report: None,
            database_path,
            advisor_messages: Vec::new(),
            solidcraft_task: None,
        }
    }

    fn choose_step(&mut self) {
        if let Some(path) = rfd::FileDialog::new().add_filter("STEP-Modell", &["step", "stp"]).pick_file() {
            self.analyze_step(path);
        }
    }

    fn analyze_step(&mut self, path: PathBuf) {
        self.status = format!("Analysiere {} …", path.display());
        let adapter = NativeStepAdapter;
        match adapter.analyze(&path) {
            Ok(analysis) => {
                let name = path.file_stem().and_then(|value| value.to_str()).unwrap_or("STEP-Projekt");
                let project = Project::new(name, analysis.model, analysis.features);
                self.status = format!("{} Merkmale erkannt · Adapter {}", project.features.len(), adapter.name());
                self.project = Some(project);
                self.selected_feature = Some(0);
                self.solidcraft_report = None;
                self.refresh_advice();
            }
            Err(error) => self.status = format!("Analyse fehlgeschlagen: {error}"),
        }
    }

    fn load_project(&mut self) {
        let Some(path) = rfd::FileDialog::new().add_filter("MBD CAM Projekt", &["json"]).pick_file() else { return };
        let result = std::fs::read_to_string(&path)
            .map_err(|error| error.to_string())
            .and_then(|text| Project::from_json(&text).map_err(|error| error.to_string()));
        match result {
            Ok(project) => {
                let source = PathBuf::from(&project.model.source_path);
                let validation = if source.is_file() {
                    NativeStepAdapter.analyze(&source).ok().map(|current| validate_references(&project.features, &current.features))
                } else {
                    None
                };
                self.status = match validation {
                    Some(validation) if validation.stale.is_empty() => {
                        format!("Projekt geladen · {} Geometriereferenzen bestätigt", validation.valid)
                    }
                    Some(validation) => format!("Projekt geladen · {} Referenzen veraltet/fehlend", validation.stale.len()),
                    None => "Projekt geladen · Quelldatei fehlt, Referenzen nicht prüfbar".into(),
                };
                self.project = Some(project);
                self.selected_feature = Some(0);
                self.refresh_advice();
            }
            Err(error) => self.status = format!("Laden fehlgeschlagen: {error}"),
        }
    }

    fn save_project(&mut self) {
        let Some(project) = &self.project else {
            self.status = "Kein Projekt zum Speichern".into();
            return;
        };
        let Some(path) =
            rfd::FileDialog::new().add_filter("MBD CAM Projekt", &["json"]).set_file_name(format!("{}.mbd-cam.json", project.name)).save_file()
        else {
            return;
        };
        match project
            .to_json_pretty()
            .map_err(|error| error.to_string())
            .and_then(|text| std::fs::write(&path, text).map_err(|error| error.to_string()))
        {
            Ok(()) => self.status = format!("Projekt gespeichert: {}", path.display()),
            Err(error) => self.status = format!("Speichern fehlgeschlagen: {error}"),
        }
    }

    fn inspect_solidcraft(&mut self, context: &egui::Context) {
        if self.solidcraft_task.is_some() {
            self.status = "SolidCraft-Prüfung läuft bereits …".into();
            return;
        }
        let Some(project) = &self.project else {
            self.status = "Zuerst ein STEP-Modell analysieren".into();
            return;
        };
        let cli = PathBuf::from(&self.solidcraft_cli);
        let source = PathBuf::from(&project.model.source_path);
        let (sender, receiver) = std::sync::mpsc::channel();
        let repaint = context.clone();
        let worker = std::thread::Builder::new().name("solidcraft-helper-wait".into()).spawn(move || {
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_solidcraft_helper(&cli, &source)))
                .unwrap_or_else(|_| Err("interner SolidCraft-Hilfsprozessfehler wurde abgefangen".into()));
            let _ = sender.send(outcome);
            repaint.request_repaint();
        });
        match worker {
            Ok(_) => {
                self.solidcraft_task = Some(receiver);
                self.status = "SolidCraft-Prüfung läuft im Hintergrund …".into();
            }
            Err(error) => self.status = format!("SolidCraft-Prüfung konnte nicht gestartet werden: {error}"),
        }
    }

    fn poll_solidcraft(&mut self) {
        let outcome = self.solidcraft_task.as_ref().and_then(|receiver| match receiver.try_recv() {
            Ok(result) => Some(result),
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Some(Err("SolidCraft-Hilfsprozess wurde unerwartet beendet".into())),
        });
        if let Some(outcome) = outcome {
            self.solidcraft_task = None;
            match outcome {
                Ok(report) => {
                    self.status = format!("SolidCraft {} bestätigt {} exakte Flächen", report.version, report.exact_faces);
                    self.solidcraft_report = Some(report);
                }
                Err(error) => self.status = format!("SolidCraft-Prüfung fehlgeschlagen: {error}"),
            }
        }
    }

    fn generate_plan(&mut self) {
        if let Some(project) = &mut self.project {
            plan_project(project, PlanOptions::default());
            let mock = MockCamBackend;
            self.status = mock.synchronize_plan(project).unwrap_or_else(|error| error.to_string());
            self.refresh_advice();
        }
    }

    fn refresh_advice(&mut self) {
        self.advisor_messages = self.project.as_ref().map(|project| RuleAdvisor.advise(project)).unwrap_or_default();
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.heading("MBD CAM AI");
            ui.separator();
            if ui.button("STEP öffnen …").clicked() {
                self.choose_step();
            }
            if ui.button("Projekt laden …").clicked() {
                self.load_project();
            }
            if ui.add_enabled(self.project.is_some(), egui::Button::new("Arbeitsplan erzeugen")).clicked() {
                self.generate_plan();
            }
            if ui.add_enabled(self.project.is_some(), egui::Button::new("JSON speichern …")).clicked() {
                self.save_project();
            }
            if ui.add_enabled(self.project.is_some(), egui::Button::new("Mit SolidCraft prüfen")).clicked() {
                self.inspect_solidcraft(ui.ctx());
            }
        });
        ui.horizontal(|ui| {
            ui.label("SolidCraft CLI:");
            ui.add(egui::TextEdit::singleline(&mut self.solidcraft_cli).desired_width(650.0));
        });
    }

    fn feature_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Merkmale");
        ui.label("Auswahl hebt das Merkmal in der Modellansicht hervor.");
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            let Some(project) = &self.project else {
                ui.label("Noch kein STEP-Modell geladen.");
                return;
            };
            for (index, feature) in project.features.iter().enumerate() {
                let label = format!("{} · {:.0}%", feature.kind.label(), feature.confidence * 100.0);
                if ui.selectable_label(self.selected_feature == Some(index), label).clicked() {
                    self.selected_feature = Some(index);
                }
            }
        });
    }

    fn details_panel(&mut self, ui: &mut egui::Ui) {
        let Some(project) = &mut self.project else { return };
        ui.heading("Korrektur & Planung");
        if let Some(index) = self.selected_feature
            && let Some(feature) = project.features.get_mut(index)
        {
            ui.label(format!("ID: {}", feature.id));
            egui::ComboBox::from_id_salt("feature-kind").selected_text(feature.kind.label()).show_ui(ui, |ui| {
                for kind in FeatureKind::ALL {
                    ui.selectable_value(&mut feature.kind, kind, kind.label());
                }
            });
            ui.add(egui::Slider::new(&mut feature.confidence, 0.0..=1.0).text("Konfidenz"));
            ui.label("Notiz / manuelle Begründung:");
            ui.add(egui::TextEdit::multiline(&mut feature.notes).desired_rows(3));
            if ui.button("Als manuell bestätigt markieren").clicked() {
                feature.evidence = cadcraft_cam::Evidence::Manual;
                feature.confidence = 1.0;
            }
            ui.monospace(format!("Mittelpunkt: {:.2}, {:.2}, {:.2} mm", feature.center.x, feature.center.y, feature.center.z));
            if let Some(diameter) = feature.diameter_mm {
                ui.label(format!("Ø {:.3} mm", diameter));
            }
            if let Some(depth) = feature.depth_mm {
                ui.label(format!("Tiefe/Spanne {:.3} mm", depth));
            }
            ui.separator();
        }
        ui.collapsing("Rohteilzugaben", |ui| {
            ui.horizontal(|ui| {
                ui.label("X/Y");
                ui.add(egui::DragValue::new(&mut project.stock.allowance_x_mm).suffix(" mm"));
                ui.add(egui::DragValue::new(&mut project.stock.allowance_y_mm).suffix(" mm"));
            });
            ui.horizontal(|ui| {
                ui.label("Oben/Unten");
                ui.add(egui::DragValue::new(&mut project.stock.allowance_top_mm).suffix(" mm"));
                ui.add(egui::DragValue::new(&mut project.stock.allowance_bottom_mm).suffix(" mm"));
            });
        });
        ui.collapsing(format!("Aufspannungen ({})", project.setups.len()), |ui| {
            for setup in &mut project.setups {
                ui.group(|ui| {
                    ui.strong(&setup.label);
                    ui.horizontal(|ui| {
                        ui.label("Werkzeugachse");
                        ui.add(egui::DragValue::new(&mut setup.tool_axis.x).speed(0.1));
                        ui.add(egui::DragValue::new(&mut setup.tool_axis.y).speed(0.1));
                        ui.add(egui::DragValue::new(&mut setup.tool_axis.z).speed(0.1));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Nullpunkt");
                        ui.add(egui::DragValue::new(&mut setup.work_zero.x).suffix(" X"));
                        ui.add(egui::DragValue::new(&mut setup.work_zero.y).suffix(" Y"));
                        ui.add(egui::DragValue::new(&mut setup.work_zero.z).suffix(" Z"));
                    });
                });
            }
        });
        ui.collapsing(format!("Operationen ({})", project.operations.len()), |ui| {
            for operation in &mut project.operations {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut operation.enabled, "");
                    ui.label(format!("{:?}", operation.kind));
                    let mut tool = operation.tool_id.clone().unwrap_or_default();
                    if ui.add(egui::TextEdit::singleline(&mut tool).hint_text("Werkzeug-ID offen").desired_width(130.0)).changed() {
                        operation.tool_id = (!tool.trim().is_empty()).then_some(tool);
                    }
                });
            }
        });
    }

    fn model_view(&self, ui: &mut egui::Ui) {
        ui.heading("3D-Modellansicht (schematisch)");
        let desired = egui::vec2(ui.available_width(), (ui.available_height() * 0.58).max(260.0));
        let (response, painter) = ui.allocate_painter(desired, egui::Sense::hover());
        painter.rect_filled(response.rect, 4.0, egui::Color32::from_rgb(244, 247, 250));
        let Some(project) = &self.project else {
            painter.text(
                response.rect.center(),
                egui::Align2::CENTER_CENTER,
                "STEP-Datei öffnen",
                egui::FontId::proportional(20.0),
                egui::Color32::GRAY,
            );
            return;
        };
        let bounds = project.model.bounds_mm;
        let project_point = |p: cadcraft_cam::Vec3| -> egui::Pos2 {
            let size = bounds.size();
            let nx = if size.x.abs() > 1.0e-9 { (p.x - bounds.min.x) / size.x } else { 0.5 };
            let ny = if size.y.abs() > 1.0e-9 { (p.y - bounds.min.y) / size.y } else { 0.5 };
            let nz = if size.z.abs() > 1.0e-9 { (p.z - bounds.min.z) / size.z } else { 0.5 };
            let sx = (nx - ny) * 0.36;
            let sy = (nx + ny) * 0.18 - nz * 0.48;
            response.rect.center() + egui::vec2(sx as f32 * response.rect.width(), sy as f32 * response.rect.height())
        };
        let corners = [
            cadcraft_cam::Vec3 { x: bounds.min.x, y: bounds.min.y, z: bounds.min.z },
            cadcraft_cam::Vec3 { x: bounds.max.x, y: bounds.min.y, z: bounds.min.z },
            cadcraft_cam::Vec3 { x: bounds.max.x, y: bounds.max.y, z: bounds.min.z },
            cadcraft_cam::Vec3 { x: bounds.min.x, y: bounds.max.y, z: bounds.min.z },
            cadcraft_cam::Vec3 { x: bounds.min.x, y: bounds.min.y, z: bounds.max.z },
            cadcraft_cam::Vec3 { x: bounds.max.x, y: bounds.min.y, z: bounds.max.z },
            cadcraft_cam::Vec3 { x: bounds.max.x, y: bounds.max.y, z: bounds.max.z },
            cadcraft_cam::Vec3 { x: bounds.min.x, y: bounds.max.y, z: bounds.max.z },
        ];
        for (a, b) in [(0, 1), (1, 2), (2, 3), (3, 0), (4, 5), (5, 6), (6, 7), (7, 4), (0, 4), (1, 5), (2, 6), (3, 7)] {
            painter.line_segment([project_point(corners[a]), project_point(corners[b])], egui::Stroke::new(2.0, egui::Color32::from_rgb(45, 70, 95)));
        }
        for (index, feature) in project.features.iter().enumerate() {
            let selected = self.selected_feature == Some(index);
            let color = if selected { egui::Color32::from_rgb(235, 80, 35) } else { feature_color(feature.kind) };
            painter.circle_filled(project_point(feature.center), if selected { 9.0 } else { 5.5 }, color);
        }
        painter.text(
            response.rect.left_top() + egui::vec2(12.0, 10.0),
            egui::Align2::LEFT_TOP,
            format!("{:.1} × {:.1} × {:.1} mm · {} Flächen", bounds.size().x, bounds.size().y, bounds.size().z, project.model.face_count),
            egui::FontId::monospace(13.0),
            egui::Color32::DARK_GRAY,
        );
    }

    fn report_view(&self, ui: &mut egui::Ui) {
        if let Some(report) = &self.solidcraft_report {
            ui.group(|ui| {
                ui.strong(format!("SolidCraft {}", report.version));
                ui.label(format!(
                    "Körper: {} · Flächen: {} ({} exakt) · Kanten: {}",
                    report.body_name, report.faces, report.exact_faces, report.edges
                ));
                ui.label(&report.message);
            });
        }
        for message in &self.advisor_messages {
            ui.label(format!("• {message}"));
        }
    }
}

impl eframe::App for MbdCamApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_solidcraft();
        ui.painter().rect_filled(ui.max_rect(), 0.0, egui::Color32::from_rgb(248, 250, 252));
        self.toolbar(ui);
        ui.separator();
        ui.columns(3, |columns| {
            self.feature_panel(&mut columns[0]);
            self.model_view(&mut columns[1]);
            self.report_view(&mut columns[1]);
            self.details_panel(&mut columns[2]);
        });
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            ui.label(&self.status);
            ui.small(format!("DB: {} · Offline · keine NC-Ausgabe", self.database_path.display()));
        });
    }
}

fn run_solidcraft_helper(cli: &Path, source: &Path) -> Result<solidcraft::SolidCraftReport, String> {
    let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis();
    let temp_dir = std::env::temp_dir();
    let stdout_path = temp_dir.join(format!("mbd-cam-solidcraft-ui-{}-{nonce}.json", std::process::id()));
    let stderr_path = temp_dir.join(format!("mbd-cam-solidcraft-ui-{}-{nonce}.error.txt", std::process::id()));
    let result = (|| {
        let executable = std::env::current_exe().map_err(|error| error.to_string())?;
        let stdout = std::fs::File::create(&stdout_path).map_err(|error| error.to_string())?;
        let stderr = std::fs::File::create(&stderr_path).map_err(|error| error.to_string())?;
        let status = std::process::Command::new(executable)
            .arg("solidcraft-check")
            .arg(source)
            .arg(cli)
            .stdout(std::process::Stdio::from(stdout))
            .stderr(std::process::Stdio::from(stderr))
            .status()
            .map_err(|error| error.to_string())?;
        if status.success() {
            read_solidcraft_report(&stdout_path)
        } else {
            let details =
                read_capped(&stderr_path, 1024 * 1024).map(|bytes| String::from_utf8_lossy(&bytes).trim().to_owned()).unwrap_or_else(|error| error);
            Err(format!("Hilfsprozess endete mit {status}: {details}"))
        }
    })();
    cleanup_task_files(&stdout_path, &stderr_path);
    result
}

fn read_solidcraft_report(path: &Path) -> Result<solidcraft::SolidCraftReport, String> {
    let bytes = read_capped(path, 16 * 1024 * 1024)?;
    serde_json::from_slice(&bytes).map_err(|error| format!("SolidCraft-Bericht ist ungültig: {error}"))
}

fn read_capped(path: &Path, max_bytes: u64) -> Result<Vec<u8>, String> {
    let metadata = std::fs::metadata(path).map_err(|error| error.to_string())?;
    if metadata.len() > max_bytes {
        return Err(format!("Hilfsprozess-Ausgabe überschreitet {} Byte", max_bytes));
    }
    std::fs::read(path).map_err(|error| error.to_string())
}

fn cleanup_task_files(stdout_path: &Path, stderr_path: &Path) {
    let _ = std::fs::remove_file(stdout_path);
    let _ = std::fs::remove_file(stderr_path);
}

fn feature_color(kind: FeatureKind) -> egui::Color32 {
    match kind {
        FeatureKind::ThroughHole | FeatureKind::BlindHole | FeatureKind::HoleCandidate => egui::Color32::from_rgb(35, 115, 210),
        FeatureKind::RectangularPocket | FeatureKind::PocketCandidate => egui::Color32::from_rgb(65, 155, 95),
        FeatureKind::Slot => egui::Color32::from_rgb(160, 90, 200),
        FeatureKind::PlanarFace | FeatureKind::Step => egui::Color32::from_rgb(30, 155, 165),
        FeatureKind::OuterContour => egui::Color32::from_rgb(220, 145, 35),
    }
}

fn default_solidcraft_cli() -> PathBuf {
    let relative = PathBuf::from("SolidCraft/0.30.1/app/solidcraft-0.30.1-windows-x64-portable/solidcraft-cli.exe");
    if let Ok(current) = std::env::current_dir()
        && let Some(parent) = current.parent()
    {
        let candidate = parent.join(&relative);
        if candidate.is_file() {
            return candidate;
        }
    }
    PathBuf::from(r"C:\Users\dirks\Documents\ChatGPT\SolidCraft\0.30.1\app\solidcraft-0.30.1-windows-x64-portable\solidcraft-cli.exe")
}
