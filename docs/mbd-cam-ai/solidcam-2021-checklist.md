# SolidCAM 2021 – COM-/Automation-Prüfliste

Status: **nicht produktiv untersucht**. Version 0.1 enthält ausschließlich `MockCamBackend`.

Vor einer echten Implementierung müssen auf einer lizenzierten Testinstallation anhand offizieller
Dokumentation und reversibler Testprojekte mindestens diese Punkte nachgewiesen werden:

- [ ] unterstützte 64-Bit-COM-/Automation-Schnittstelle und registrierte ProgID/CLSID
- [ ] kompatible Host-CAD-Version und Prozess-/Threadingmodell (STA/MTA)
- [ ] Projekt anlegen, öffnen, speichern und sicher schließen
- [ ] CAD-Datei referenzieren, ohne Original zu überschreiben
- [ ] Maschine/Steuerung/Postprozessor ausschließlich über freigegebene IDs auswählen
- [ ] Koordinatensystem, Teilnullpunkt und Aufspannungen anlegen/lesen
- [ ] Rohteil und Zielteil referenzieren
- [ ] Face-/Edge-/Sketch-Geometrien stabil zu Operationen zuordnen
- [ ] Plan-, Bohr-, Taschen-, Nut- und Konturoperation anlegen
- [ ] Werkzeug/Holder aus einer verifizierten Bibliothek auswählen
- [ ] technologische Parameter lesen und setzen; Einheiten eindeutig nachweisen
- [ ] Werkzeugweg berechnen und Fehler/Warnungen strukturiert empfangen
- [ ] Simulationsaufruf sowie Ergebnisstatus nachweisen
- [ ] Postprozessoraufruf strikt getrennt und standardmäßig deaktiviert halten
- [ ] Abbruch, Timeout, Undo/Transaktion und Wiederanlauf nach COM-Fehler testen
- [ ] Nebenwirkungen, Dialoge und nicht-interaktive Ausführung dokumentieren
- [ ] Lizenzbedingungen und Verteilbarkeit von Interop-Metadaten klären
- [ ] Regressionstest mit einem nicht produktiven, synthetischen Projekt

Freigaberegeln:

1. Unbekannte Parameter werden nie geraten.
2. Programmatic calls dürfen keine unbeaufsichtigten Dialoge oder NC-Ausgabe auslösen.
3. Werkzeugweg/Simulation/Postprozess sind getrennte, explizite Aktionen.
4. Maschinenfreigabe bleibt eine menschliche, dokumentierte Entscheidung.
5. Ein COM-Fehler darf das Quellmodell und den gespeicherten JSON-Plan nicht beschädigen.
