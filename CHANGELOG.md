# Changelog

## 1.0.0 – noch nicht veröffentlicht

### Neu
- **Browser-Version im Container** (`zmn-web`): gleiche Engine, Bedienung im Browser vom Mac oder iPhone, Original/SVG-Vergleich mit Schieberegler, Speichern als SVG, PNG, PDF, DXF, EPS, STEP und 3MF. Texterkennung mit Tesseract (Farbmasken je Farbe, damit farbige Schrift auf farbigem Grund gelesen wird). Image `ghcr.io/mitkiprogrammieren/zeichnemichnach-web`, baut automatisch.
- **Prüfstand** (`zmn-pruefstand`): Testbilder mit bekanntem Original, Messung von Farbfehler, Fehlpixeln, Kanten, Knoten, Zeit und zu schmalen Stellen; VTracer als Vergleich. Ergebnis: eigener Kern ist durchweg genauer als VTracer.
- **Dünne Linien entmischen**: Linien und Ränder aus Mischpixeln bekommen ihre echte Farbe (statt blass) und die richtige Breite (Deckung bleibt erhalten). Fehlpixel bei Linien 2,6 % → 2,0 %, bei kleinen Bildern 1,5 % → 1,0 %.
- **Hintergrund entfernen** auch bei Verlauf/Vignette (glattes Farbmodell aus dem Rand), optional mit Innenflächen.
- **Spitze Ecken abrunden** im Druckmodus (Radius einstellbar, Standard halbe Düse); Düse setzt schmalste Stelle und Eckenradius; Höhen und Rand werden an die Düse angepasst.
- Kommandozeile: `--duese`, `--ecken`, `--innen`.

### Behoben
- Druckmodus: verbreiterte Striche wurden im zweiten Durchgang wieder zu schmal (jetzt sicher ≥ schmalste Stelle).
- Eigene App **ZeichneMichNach** im Mesh2STEP-Look, dunkles Design als Standard, hell/dunkel/System in den Einstellungen.
- **Nachzeichnen wie von Hand**: Hintergrund außen freistellen, Text erkennen und durch passende Schrift ersetzen (Apple-Texterkennung, alle Schriften des Macs, Schnitt je Buchstabe), feine Linien als Striche (über Kreuzungen verbunden, Kreise ergänzt), Fotobereiche beruhigen, Farbverläufe.
- Tafel „Erkannter Text“ mit gewählter Schrift und Übereinstimmung.
- Kommandozeile `zmn`.
- Installationsskript `scripts/mac-installieren.sh`.
- **Für den 3D-Druck**: Druckbreite, Anzahl Farben (Zahl eintippen), Düse, schmalste Stelle; flache Farben ohne Verläufe, dünne Striche und Schrift werden verstärkt statt gelöscht, Schattierungen derselben Farbe zusammengelegt, Fotobereiche als eine Farbe oder ruhige Stufen, eigene Filamentfarben (AMS).
- **STEP speichern** mit echten Flächen und Farben, ein Körper je Farbe; dazu 3MF mit demselben Aufbau.
- **Grundschicht und Rand**: Grundschicht unter allen Farbschichten (Umriss minus Randbreite), eigener Rand füllt die Lücke; Stärke, Breite, Höhe und Farben einstellbar; Farben gestuft oder alle gleich hoch.
- Flache Grafiken werden erkannt und direkt sauber nachgezeichnet (Grundfarben inkl. kleiner wie Rot, Kantenglättung zählt nicht als Farbe).
