# Changelog

## 1.0.0 – noch nicht veröffentlicht

### Neu
- **Browser-Version im Container** (`zmn-web`): gleiche Engine, Bedienung im Browser vom Mac oder iPhone, Original/SVG-Vergleich mit Schieberegler, Speichern als SVG, PNG, PDF, DXF, EPS, STEP und 3MF. Texterkennung mit Tesseract (Farbmasken je Farbe, damit farbige Schrift auf farbigem Grund gelesen wird). Image `ghcr.io/mitkiprogrammieren/zeichnemichnach-web`, baut automatisch.
- **Prüfstand** (`zmn-pruefstand`): Testbilder mit bekanntem Original, Messung von Farbfehler, Fehlpixeln, Kanten, Knoten, Zeit und zu schmalen Stellen; VTracer als Vergleich. Ergebnis: eigener Kern ist durchweg genauer als VTracer.
- **Dünne Linien entmischen**: Linien und Ränder aus Mischpixeln bekommen ihre echte Farbe (statt blass) und die richtige Breite (Deckung bleibt erhalten). Fehlpixel bei Linien 2,6 % → 2,0 %, bei kleinen Bildern 1,5 % → 1,0 %.
- **Hintergrund entfernen** auch bei Verlauf/Vignette (glattes Farbmodell aus dem Rand), optional mit Innenflächen.
- **Spitze Ecken abrunden** im Druckmodus (Radius einstellbar, Standard halbe Düse); Düse setzt schmalste Stelle und Eckenradius; Höhen und Rand werden an die Düse angepasst.
- **Bildart Illustration** (automatisch erkannt oder wählbar): gezeichnete Figuren mit Schattierung und Glanz werden zu vielen sauberen Farbflächen (wie vectorizer.ai) statt zu Logo-Flächen mit Linien. Beim Silvester-Award: Fehlpixel 20 % → 5,3 % (vectorizer.ai 7,2 %).
- Browser-Version: **Lupe** – Kreis folgt der Maus über dem Bild, Mausrad vergrößert (1,5–24×); im Vergleich zeigt sie je nach Seite Original oder SVG.
- Browser-Version: Ansicht **Vergleich / Vektor / Konturen**, Speichern **Allgemein / Gestapelt / Schneiden** (nur Konturen für Laser, Plotter, Gravur).
- **Schärfere Kanten bei Illustrationen**: Übergangspixel werden vor dem Nachzeichnen auf die nähere Kantenseite gesetzt (Kramer-Bruckner), keine Mischfarb-Säume mehr; Splitter werden stärker zusammengelegt. Silvester-Award: Kantenschärfe 0,92 → 1,04 (vectorizer.ai 1,12), Säume 6,9 % → 6,1 %, weiterhin weniger Fehlpixel als vectorizer.ai.
- **Kanten-KI** (eigenes kleines Netz, 67.000 Gewichte, ohne Fremdbibliothek im Kern): erkennt echte Kanten (Umrisse, Streifen, Schrift) und unterscheidet sie von Schattierung und Glanz. Trainiert nur auf selbst erzeugten Bildern (F1 96,2 %). Bei Illustrationen wird außerhalb echter Kanten geglättet und nur an Kanten geschärft – bei 10 Pokal-Bildern weniger Fehlpixel bei jedem Bild (z. B. 5,8 % → 5,3 %). Optional **Vereinfachen** (mittel/stark): Schattierungsstufen werden zu ruhigen Flächen.
- Prüfstand: **eigene Referenzbilder** (`referenzen/` mit Original und optional fremder SVG gleichen Namens) werden mitgemessen, inkl. Kantenschärfe; in der Browser-Version direkt unter ⚙︎ hochladbar – mehrere Dateien, ganze Ordner oder ZIP, einzeln oder alle löschbar.
- Texterkennung unter Linux: in unruhigen Bildern nur sehr sichere Zeilen (keine „Phantomtexte“ mehr in Formen).
- Kommandozeile: `--duese`, `--ecken`, `--innen`.

### Behoben
- Texterkennung im Container deutlich schneller: die Durchgänge je Farbe laufen parallel (Silvester-/Pokal-Bilder: 8,4 s → 6,0 s auf 2 Kernen, auf dem NUC mit 8 Kernen mehr). Gleichzeitige Anfragen kommen sich bei den Zwischendateien nicht mehr in die Quere.
- Hintergrund entfernen lief bei schwarzem (oder weißem) Hintergrund in fast gleichfarbige Teile des Motivs (schwarzer Controller, dunkler Sockel). Jetzt passt sich die Toleranz dem Rauschen des Hintergrunds an, und die Füllung stoppt an jedem Farbsprung. Der Rand wird entmischt (kein dunkler/heller Saum mehr) und geglättet.
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
