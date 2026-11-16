# ZeichneMichNach

Mac-App, die Fotos und Logos **lokal** in saubere Vektorgrafiken (SVG, PDF, DXF, EPS, PNG, 3MF) nachzeichnet – ohne Internet, ohne API.

Grundlage ist der Vektorix-Kern aus Mesh2STEP (Farbsuche, Bézierkurven, Kreise, Symmetrie). Dazu kommt **„Nachzeichnen wie von Hand“**:

- **Hintergrund außen freistellen** – einfarbiger Rand wird transparent
- **Text erkennen & ersetzen** – Apple-Texterkennung (Vision) findet den Text, dann wird jede Schrift auf deinem Mac Buchstabe für Buchstabe auf das Bild gelegt; die beste Familie gewinnt, je Buchstabe der passende Schnitt (fett und dünn gemischt geht)
- **Feine Linien als Striche** – Linien bleiben über Kreuzungen hinweg ganz, Kreisbögen werden echte Kreise
- **Fotobereiche beruhigen** – Fotos und Muster werden weiche Flächen statt Flecken
- **Farbverläufe erkennen** – Gold, Metall, Schatten als echter Verlauf
- **Hintergrund entfernen** – auch mit Verlauf, Vignette oder ungleichmäßig ausgeleuchtetem Scan, wahlweise mit Innenflächen (Inneres von „O“)
- **Dünne Linien entmischen** – blasse Mischfarben aus Kantenglättung/Verkleinerung werden zur echten Farbe zurückgerechnet, Breite bleibt erhalten
- **Für den 3D-Druck** – Düse 0,2/0,4/0,6/0,8: schmalste Stelle 2 × Düse, spitze Ecken mit halber Düse abgerundet, Höhen auf die Schichthöhe gerundet

## Installieren (Mac)

```bash
curl -fsSL https://raw.githubusercontent.com/MitKIprogrammieren/ZeichneMichNach/main/scripts/mac-installieren.sh -o mac-installieren.sh
bash mac-installieren.sh
```

Das Skript holt den Code, baut App, Texterkennung (Swift) und Kommandozeile und legt `ZeichneMichNach.app` in den Programme-Ordner.

## Bedienung

Bild ins Fenster ziehen → links „Ergebnis verfeinern“ → „Nachzeichnen wie von Hand“. Rechts stehen die erkannten Texte samt Schrift. Hell/Dunkel in den Einstellungen (⚙︎, ⇧⌘D schaltet um).

## Im Browser (Container)

Dieselbe Engine als Web-App, z. B. auf dem NUC oder der Synology – bedienbar vom Mac, iPhone oder jedem Browser:

```bash
docker run -d -p 8094:8094 -v ./daten:/daten ghcr.io/mitkiprogrammieren/zeichnemichnach-web:latest
```

oder mit `packaging/web/compose.yaml`. Danach `http://<server>:8094` öffnen. Texterkennung läuft dort mit Tesseract statt Apple Vision; eigene Schriften nach `daten/schriften` legen. Das Image baut GitHub bei jeder Änderung automatisch (`.github/workflows/web-container.yml`).

## Prüfstand

```bash
cd tools/zmn-pruefstand && cargo run --release -- ../../pruefstand --anzahl 3
```

Erzeugt Testbilder mit bekanntem Original (flach, Wappen, Text, Linien, Verlauf, Freisteller – jeweils sauber, klein und als JPEG), zeichnet sie nach und misst Farbfehler, Fehlpixel, Kantenabstand, Knoten und zu schmale Stellen – auch im Druckmodus (0,4er und 0,2er Düse). VTracer läuft als Vergleichsmaßstab mit. Ergebnis: `bericht.html`, `werte.csv`, `zusammenfassung.txt`. In der Browser-Version unter ⚙︎ startbar.

## Kommandozeile

```bash
zmn bild.png -o ergebnis.svg [-k farben] [--ohne text,linien,foto,verlauf,hintergrund] [--innen]
zmn bild.png -o ergebnis.3mf --druck 80 --duese 0.4 [--ecken 0.2] [--farben 4]
```

## Aufbau

- `core/` – Bildanalyse und Vektorisierung (ohne Fremdbibliotheken), `hand.rs` = Nachzeichnen wie von Hand
- `app/` – Oberfläche (egui), `fonts.rs` Schriftsuche, `ocr.rs` Texterkennung, `pipeline.rs` Ablauf
- `packaging/ocr/main.swift` – kleiner Helfer für Apple Vision
- `tools/zmn-cli/` – Kommandozeile
- `tools/zmn-web/` – Browser-Version (Server + Oberfläche), `packaging/web/` Dockerfile und Compose, `packaging/linux/zmn-ocr` Texterkennung mit Tesseract
- `tools/zmn-pruefstand/` – Testbilder erzeugen, nachzeichnen, Abweichung messen
