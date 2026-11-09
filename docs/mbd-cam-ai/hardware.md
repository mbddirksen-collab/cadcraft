# Hardwarebewertung

## Vorhandener Rechner

Dell Precision 3630, Xeon E-2186G, 64 GB RAM, Quadro P4000 8 GB, 512-GB-SSD.

Für den MVP ist das System geeignet:

- STEP-B-Rep-Analyse, regelbasierte Feature-Erkennung und SQLite sind CPU-/RAM-seitig ausreichend.
- 64 GB RAM bieten guten Spielraum für mittelgroße Modelle; die App begrenzt Eingaben zusätzlich.
- 8 GB VRAM reichen für kleine, stark quantisierte lokale Sprachmodelle und einfache CAD-Ansichten,
  nicht für große Modelle oder hohe Kontextlängen. CPU-Fallback bleibt vorgesehen.
- Die 512-GB-SSD ist für Prototyp und kleine Modelldateien ausreichend; lokale Modellgewichte,
  Backups und künftige CAM-Projekte benötigen jedoch aktives Speicherplatzmanagement.

Wichtiger Befund: `solidcraft-cli gpu` erkannte im Test nur Intel UHD (Vulkan/DX12) und den Microsoft
Basic Render Driver, **nicht** die Quadro P4000. Vor GPU-Inferenz oder GPU-lastiger CAD-Nutzung sind
NVIDIA-Treiber, Gerätemanagerstatus, Monitorkabel/GPU-Zuordnung und Windows-Grafikeinstellungen zu
prüfen. Bis dahin darf die P4000 nicht als tatsächlich verfügbare Rechen-GPU eingeplant werden.

## Lokale KI für den Start

- deterministischer Regelkern bleibt die Standardausführung ohne Modell
- zunächst kleine 3B–7B-Instruct-Modelle in 4-Bit-Quantisierung
- bevorzugt llama.cpp oder Ollama mit dokumentierter Offline-Konfiguration
- CPU-Fallback, begrenzter Kontext und keine Geometriemessung durch das Sprachmodell
- keine automatischen Modelldownloads in der Produktionsumgebung

## Spätere Workstation – Anforderungsprofil, kein Kaufplan

- aktuelle Workstation-CPU mit hoher Single-Core-Leistung und mindestens 12–16 leistungsfähigen Kernen
- 128 GB ECC-RAM, wenn große Baugruppen/mehrere lokale Modelle realistisch werden
- NVIDIA-GPU mit mindestens 16 GB, besser 24 GB VRAM für komfortablere lokale Inferenz
- separate schnelle NVMe-SSD (2 TB oder mehr) für Modelle, CAD/CAM-Projekte und temporäre Netze
- zertifizierter Treiberpfad für die eingesetzte CAD/CAM-Software
- ausreichendes Netzteil, Kühlung, Backup- und Restore-Konzept

Ein Hardwarekauf ist für Version 0.1 nicht erforderlich. Zuerst sollte die vorhandene Quadro korrekt
sichtbar gemacht und die reale Modell-/Bauteilgröße gemessen werden.
