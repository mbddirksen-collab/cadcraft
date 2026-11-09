# SolidCraft 0.30.1 – technische Bestandsaufnahme

Untersucht wurde die offizielle Windows-x64-Portable-Version 0.30.1. Das Archiv wurde gegen die
veröffentlichte SHA-256-Prüfsumme verifiziert.

- Archiv: `solidcraft-0.30.1-windows-x64-portable.zip`
- SHA-256: `14392c9d96f21a6bcd606d565bd676703be46c01fccf8956da2e8a89ab1f1d21`
- geprüfte Programme: `solidcraft.exe`, `solidcraft-cli.exe`

## Kompatibilitätsmatrix

| Fähigkeit | Status | Nachweis / Einschränkung |
|---|---|---|
| STEP-Import | nachgewiesen | UI öffnet Beispiel/STEP; CLI importierte das synthetische Frästeil |
| STEP-Export | nachgewiesen | CLI exportierte 86.951-Byte-STEP; Rückimport mit gleicher BBox und nahezu gleichem Volumen |
| B-Rep-Volumenkörper | nachgewiesen | `inspect.measure`: geschlossen, Volumen/BBox/Topologie verfügbar |
| einzelne Flächen | nachgewiesen | `model.faces`: Index, Name, `exact`, Fläche, Schwerpunkt, Ebene/Zylinder |
| einzelne Kanten | nachgewiesen | `model.edges` liefert die Körperkanten |
| UI-Flächenauswahl | nachgewiesen | Ebene und Zylinderwand im Beispiel angeklickt und blau hervorgehoben |
| Flächenmessung | nachgewiesen | UI „Messen“ meldete für die gewählte Fläche 3419,42 mm² |
| Transformation/Ausrichtung | nachgewiesen | Registry enthält `solid.move` und `solid.align`; parametrische Matrix/Up-Optionen beim Import |
| Koordinatensysteme | eingeschränkt | Körpertransformation nachgewiesen; dediziertes CAM-WKS nicht vorhanden |
| MCP | nachgewiesen | `solidcraft-cli mcp` vorhanden, headless oder Verbindung zur laufenden App |
| allgemeine CLI-Automation | nachgewiesen | `run`, `eval`, `exec`, `commands`, JSON-Skripte und strikte Parameterprüfung |
| Python-API | nicht verfügbar | in Paket/CLI/Command-Registry keine dokumentierte Python-Schnittstelle gefunden |
| Rohteil als CAM-Objekt | nicht verfügbar | kein `stock`-Command; als normaler Solid modellierbar, aber ohne CAM-Semantik |
| Spannmittel als CAM-Objekt | nicht verfügbar | kein `fixture`-Command; als Komponenten/Solids modellierbar |
| Maschine als CAM-Objekt | nicht verfügbar | keine Maschinenkinematik oder Maschinenstammdaten in der Command-Registry |
| Feature-Hervorhebung | eingeschränkt | `appearance.assign` kann Face-/Body-Farbe und Opazität setzen; Rückzuordnung bleibt Adapteraufgabe |
| Schnitt/Zugänglichkeit | eingeschränkt | `inspect.section` und `inspect.accessibility` vorhanden; keine CAM-Kollisionsgarantie |
| SolidCAM-2021-Automation | noch ungeprüft | gehört nicht zu SolidCraft; separate Hersteller-/Installationsprüfung nötig |

## Reproduzierter Test

Das synthetische Modell enthält eine 100 × 60 × 10-mm-Platte, zwei Ø10-Durchgangsbohrungen,
eine 30 × 20 × 4-mm-Tasche, eine 8-mm-Nut, ein Ø8 × 5-mm-Sackloch und eine seitliche
Ø6 × 20-mm-Bohrung.

SolidCraft meldete nach STEP-Rückimport:

- geschlossener Körper, 1 Shell
- BBox 100 × 60 × 10 mm
- 21 logische Flächen (`plane` 15, `cylinder` 6)
- Volumen 52.396,7118 mm³ (geringe Tessellierungs-/Exportabweichung zum Ursprungsmodell)
- exakte Ebenen-/Zylinderdaten über `model.faces`
- `inspect.accessibility` für +Z und `inspect.section` bei Z=5 ausführbar

Der integrierte Adapter nutzt SolidCraft nur zur optionalen Gegenprüfung. Die native STEP-Analyse
und Projektplanung bleiben ohne SolidCraft funktionsfähig.
