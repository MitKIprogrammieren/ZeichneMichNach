# Changelog

## 1.0.0 – noch nicht veröffentlicht

### Neu
- Eigene App **ZeichneMichNach** im Mesh2STEP-Look, dunkles Design als Standard, hell/dunkel/System in den Einstellungen.
- **Nachzeichnen wie von Hand**: Hintergrund außen freistellen, Text erkennen und durch passende Schrift ersetzen (Apple-Texterkennung, alle Schriften des Macs, Schnitt je Buchstabe), feine Linien als Striche (über Kreuzungen verbunden, Kreise ergänzt), Fotobereiche beruhigen, Farbverläufe.
- Tafel „Erkannter Text“ mit gewählter Schrift und Übereinstimmung.
- Kommandozeile `zmn`.
- Installationsskript `scripts/mac-installieren.sh`.
- **Für den 3D-Druck**: Druckbreite, Anzahl Farben (Zahl eintippen), Düse, schmalste Stelle; flache Farben ohne Verläufe, dünne Striche und Schrift werden verstärkt statt gelöscht, Schattierungen derselben Farbe zusammengelegt, Fotobereiche als eine Farbe oder ruhige Stufen, eigene Filamentfarben (AMS).
- **STEP speichern** mit echten Flächen und Farben, ein Körper je Farbe; dazu 3MF mit demselben Aufbau.
- **Grundschicht und Rand**: Grundschicht unter allen Farbschichten (Umriss minus Randbreite), eigener Rand füllt die Lücke; Stärke, Breite, Höhe und Farben einstellbar; Farben gestuft oder alle gleich hoch.
- Flache Grafiken werden erkannt und direkt sauber nachgezeichnet (Grundfarben inkl. kleiner wie Rot, Kantenglättung zählt nicht als Farbe).
