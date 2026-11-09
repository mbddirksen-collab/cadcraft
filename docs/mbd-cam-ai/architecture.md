# MBD CAM AI 0.1 – Architektur und Stand

## Warum Rust statt Python

Der Auftrag bevorzugt Python. Das bestehende CADCraft-Repository verlangt jedoch ausschließlich Rust,
strikte Schichtengrenzen, `unsafe_code = "forbid"`, keine Panics in Produktionscode und weiterhin
baubares WASM. Das MVP folgt daher den Repository-Regeln. Daten- und Backend-Grenzen sind so gestaltet,
dass ein späterer separater Python-Prozess über JSON angebunden werden könnte, ohne den deterministischen
Kern zu ersetzen.

OpenCascade/pythonOCC wurde nicht verwendet: die lokalen Projektregeln schließen OpenCascade und
GPL/LGPL-Code ausdrücklich aus. Der unabhängige Adapter nutzt stattdessen das Apache-2.0-lizenzierte,
reine Rust-Paket `monstertruck`.

## Komponenten

```text
STEP / SolidCraft
       │
       ├── NativeStepAdapter ── exakte Topologie + regelbasierte Kandidaten
       │
       └── SolidCraft CLI ───── optionale, lesende Gegenprüfung
                    │
             Project JSON
          ┌─────────┼──────────┐
      Setup-     Technologie-  Referenz-
      planer       planer      validierung
          │             │
       SQLite      CamBackend
      Stammdaten   └── Mock (keine NC-Ausgabe)
```

- `crates/cam`: UI-unabhängiger, deterministischer Kern (Layer 3).
- `apps/mbd-cam`: Windows-Desktop-/CLI-App, SQLite und SolidCraft-Prozessadapter.
- `CadAdapter`: austauschbare Geometriequelle; `NativeStepAdapter` funktioniert ohne SolidCraft.
- `Advisor`: lokaler Erweiterungspunkt. `RuleAdvisor` ist offline, transparent und benötigt kein Modell.
- `CamBackend`: spätere SolidCAM-Grenze. Der Mock bestätigt nur die strukturierte Übergabe.

## JSON-Projektmodell

Schema `mbd-cam-project/0.1` enthält:

- Quelle, Adapter, Bounding Box, Topologiezählung und Dateidigest
- Features mit ID, Typ, Evidenz, Konfidenz, Maßen, Richtung und Geometriereferenzen
- Rohteilzugaben
- Aufspannungen mit Werkzeugachse, Nullpunkt, Spannmittel-ID und Operationen
- Operationen mit Begründung und Warnungen; technologische Werte sind optional
- optionale Maschinen-/Steuerungs-/Postprozessor-IDs

Geometriereferenzen kombinieren Adapter-ID und quantisierten Flächenfingerprint. Beim erneuten Laden
wird beides verglichen; fehlende oder geänderte Referenzen werden gemeldet und nicht stillschweigend
als gültig behandelt.

## Feature-Erkennung

- Planflächen sind als B-Rep-Ebene nachgewiesen.
- Zylinder werden aus `CYLINDRICAL_SURFACE` sowie geradlinig-kreisförmigen B-Spline-/NURBS-Flächen
  erkannt. Geteilte Halbflächen derselben Achse werden gruppiert.
- Konkave Zylinder werden anhand orientierter Normalen als Bohrungskandidaten behandelt.
- Durchgang/Sackloch folgt aus der axialen Spanne relativ zur Bauteil-BBox.
- Eine Nut ist ein Paar gleich großer Zylinderenden mit gemeinsamen geraden Seitenwänden.
- Eine Rechtecktasche ist derzeit eine innenliegende +Z-Ebene mit vier Kanten; offene Stufen müssen
  manuell ausgeschlossen werden.
- Außenkontur und Bearbeitungsrichtungen sind abgeleitet, nicht geometrisch/fertigungstechnisch bewiesen.

## Phasenstatus

| Phase | Stand 0.1 |
|---|---|
| 0 SolidCraft | untersucht und dokumentiert; CLI-Abfrage integriert |
| 1 STEP-Analyse | MVP lauffähig; einfache B-Rep-/Spline-Geometrie |
| 2 Ausrichtung/Aufspannung | Richtungsgruppen, editierbarer Nullpunkt/Zugaben; kein echtes Spannmodell |
| 3 Feature Recognition | Plan, Loch, Tasche, Nut, Kontur; Stufe nur im Schema vorbereitet |
| 4 Technologieplanung | regelbasierte Operationen, offene Technologiewerte, Begründungen |
| 5 Datenbank | SQLite-Schema und bekannte Zuordnungen; keine erfundenen Grenz-/Bestandsdaten |
| 6 lokale KI | Schnittstelle + No-Model-Regelberater; Ollama/llama.cpp noch nicht angebunden |
| 7 SolidCAM | `CamBackend` + Mock; COM/Automation ungeprüft und nicht produktiv |

## Tests

- JSON-Roundtrip und Schema-/Größenprüfung
- Mock verweigert Werkzeugwege, Simulation und NC
- synthetische Planungsfälle: Durchgang, Sackloch, Rechtecktasche, Nut, mehrere Seiten
- zur Laufzeit generierter STEP-Quader: Bounding Box, Planfläche, Außenkontur
- feindliche STEP-Bytes bleiben als Fehler innerhalb der Panic-Grenze
- manuell erzeugtes Frästeil (100 × 60 × 10 mm): 2 Durchgangslöcher, 2 Sacklöcher (eins seitlich),
  Rechtecktasche, Nut, +Z/+Y-Aufspannung; SolidCraft-Roundtrip separat gegengeprüft

## Bewusste Grenzen

Keine Freiformfeature-Erkennung, Gewinde, PMI/MBD-Toleranzen, Restmaterial-, 4/5-Achs- oder
Drehplanung. Keine echte Sichtbarkeits-/Werkzeugzugänglichkeitsanalyse, Kollision, Simulation,
Werkzeugweg- oder Postprozessorfunktion. Die Oberfläche visualisiert BBox und Featurezentren,
nicht das vollständige Dreiecksnetz.
