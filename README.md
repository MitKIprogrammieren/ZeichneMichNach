# ZeichneMichNach

Mac-App, die Fotos und Logos **lokal** in saubere Vektorgrafiken (SVG, PDF, DXF, EPS, PNG, 3MF) nachzeichnet – ohne Internet, ohne API.

Grundlage ist der Vektorix-Kern aus Mesh2STEP (Farbsuche, Bézierkurven, Kreise, Symmetrie). Dazu kommt **„Nachzeichnen wie von Hand“**:

- **Hintergrund außen freistellen** – einfarbiger Rand wird transparent
- **Text erkennen & ersetzen** – Apple-Texterkennung (Vision) findet den Text, dann wird jede Schrift auf deinem Mac Buchstabe für Buchstabe auf das Bild gelegt; die beste Familie gewinnt, je Buchstabe der passende Schnitt (fett und dünn gemischt geht)
- **Feine Linien als Striche** – Linien bleiben über Kreuzungen hinweg ganz, Kreisbögen werden echte Kreise
- **Fotobereiche beruhigen** – Fotos und Muster werden weiche Flächen statt Flecken
- **Farbverläufe erkennen** – Gold, Metall, Schatten als echter Verlauf

## Installieren (Mac)

```bash
curl -fsSL https://raw.githubusercontent.com/MitKIprogrammieren/ZeichneMichNach/main/scripts/mac-installieren.sh -o mac-installieren.sh
bash mac-installieren.sh
```

Das Skript holt den Code, baut App, Texterkennung (Swift) und Kommandozeile und legt `ZeichneMichNach.app` in den Programme-Ordner.

## Bedienung

Bild ins Fenster ziehen → links „Ergebnis verfeinern“ → „Nachzeichnen wie von Hand“. Rechts stehen die erkannten Texte samt Schrift. Hell/Dunkel in den Einstellungen (⚙︎, ⇧⌘D schaltet um).

## Kommandozeile

```bash
zmn bild.png -o ergebnis.svg [-k farben] [--ohne text,linien,foto,verlauf,hintergrund]
```

## Aufbau

- `core/` – Bildanalyse und Vektorisierung (ohne Fremdbibliotheken), `hand.rs` = Nachzeichnen wie von Hand
- `app/` – Oberfläche (egui), `fonts.rs` Schriftsuche, `ocr.rs` Texterkennung, `pipeline.rs` Ablauf
- `packaging/ocr/main.swift` – kleiner Helfer für Apple Vision
- `tools/zmn-cli/` – Kommandozeile
