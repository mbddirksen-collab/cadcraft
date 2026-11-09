# MBD CAM AI 0.1

Lokaler, vollständig offline arbeitender CAM-Planungsassistent für Windows. Das MVP liest STEP,
erkennt einfache Fräsmerkmale, schlägt Aufspannungen und Operationen vor und speichert den
editierbaren Plan als JSON. Es erzeugt **keine Werkzeugwege, keinen NC-Code und keine
Maschinenfreigabe**.

## Funktionsumfang 0.1

- nativer, von SolidCraft unabhängiger STEP-Adapter (pure Rust, `monstertruck` 0.4.1)
- Bounding Box sowie Flächen-/Kanten-/Punktzählung
- analytische und spline-codierte Zylinderflächen
- regelbasierte Kandidaten für Planfläche, Durchgangs-/Sackloch, Rechtecktasche, Nut und Außenkontur
- Trennung von geometrisch nachgewiesen (`proven`), abgeleitet (`inferred`) und manuell bestätigt (`manual`)
- mehrere Aufspannungen anhand der Werkzeugachsen; Nullpunkt und Rohteilzugaben editierbar
- editierbarer Technologieplan ohne erfundene Werkzeuge oder Schnittwerte
- JSON-Speichern/-Laden mit Revalidierung der Geometriereferenzen
- optionale SolidCraft-0.30.1-Gegenprüfung über die dokumentierte CLI
- lokale SQLite-Datenbank und austauschbare lokale Advisor-Schnittstelle
- `CamBackend` mit sicherem Mock; Werkzeugweg, Simulation und Postprozessor verweigern in 0.1

## Bauen und starten

Voraussetzung ist Rust 1.90 oder neuer plus eine Windows-C/C++-Linkerumgebung. Empfohlen sind
Visual Studio Build Tools 2022 mit „Desktopentwicklung mit C++“.

```powershell
cargo run -p mbd-cam
```

Auf dem Testrechner fehlt `link.exe`; dort wurde der bereits installierte GNU-Toolchain verwendet:

```powershell
$mingw = "$env:LOCALAPPDATA\Microsoft\WinGet\Packages\BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\mingw64\bin"
$env:PATH = "$mingw;$env:PATH"
cargo +stable-x86_64-pc-windows-gnu run -p mbd-cam
```

Headless:

```powershell
mbd-cam analyze PART.step PART.mbd-cam.json
mbd-cam plan PART.step PART.mbd-cam.json
mbd-cam solidcraft-check PART.step C:\Pfad\zu\solidcraft-cli.exe
mbd-cam db-init manufacturing.sqlite
```

## Bedienablauf

1. **STEP öffnen** und native Analyse abwarten.
2. Ein Merkmal auswählen; der Punkt wird in der schematischen isometrischen Ansicht hervorgehoben.
3. Fehlklassifizierungen im Typfeld korrigieren, Konfidenz/Notiz bearbeiten und optional manuell bestätigen.
4. Rohteilzugaben ändern und **Arbeitsplan erzeugen**.
5. Werkzeugachsen und Werkstücknullpunkte der vorgeschlagenen Aufspannungen prüfen.
6. Optional **Mit SolidCraft prüfen**; die App fragt Flächen/Kanten ausschließlich lesend ab.
7. Projekt als JSON speichern. Beim Laden werden gespeicherte Face-IDs und Fingerprints gegen das
   STEP-Modell geprüft, soweit die Quelldatei noch vorhanden ist.

Die 3D-Ansicht in 0.1 ist eine sichere schematische BBox-/Feature-Darstellung, kein CAM-Simulator.

## Sicherheit und Datenqualität

- Neutraler STEP-Inhalt belegt keine Gewinde, Toleranzen, Oberflächen, Werkstoffe oder Fertigungsabsicht.
- `inferred` bedeutet Kandidat, nicht Nachweis.
- Werkzeug, Halter, Spannmittel, Maschine, Schnittwerte, Zustellungen und Sicherheitsabstände bleiben
  leer, solange keine freigegebenen Stammdaten vorliegen.
- Ohne reale Maschinen-, Spannmittel-, Halter- und Werkzeuggeometrie gibt es keine Kollisionsaussage.
- Der Mock kann einen Plan synchronisieren, verweigert aber Werkzeugweg, Simulation und NC-Ausgabe.

Weitere Dokumentation:

- [Architektur](../../docs/mbd-cam-ai/architecture.md)
- [SolidCraft-Kompatibilität](../../docs/mbd-cam-ai/solidcraft-compatibility.md)
- [SolidCAM-2021-Prüfliste](../../docs/mbd-cam-ai/solidcam-2021-checklist.md)
- [Hardwarebewertung](../../docs/mbd-cam-ai/hardware.md)
