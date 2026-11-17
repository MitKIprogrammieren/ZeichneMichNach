# Skizze → Zeichnung (Container)

Foto einer Handskizze → Gemini liest die Maße → Fertigungszeichnung (A4, SVG/PNG/PDF) für Flach-, Dreh-, Biege- und Frästeile.

Dieselbe Karte wie in Home Assistant (`karte.js`, `custom:skizze-zeichnung-card`), hier ohne Home Assistant:
`server.py` (nur Python-Standardbibliothek) ruft Gemini direkt auf und speichert Einstellungen und Zeichnungen unter `/daten`.

- Port 8095, Daten-Ordner `/daten`
- Gemini-Schlüssel in der Seite eintragen (wird in `/daten/schluessel.json` abgelegt) oder per Umgebungsvariable `GEMINI_API_KEY`
- Modelle über `GEMINI_MODELLE` (Komma-Liste, erstes = Standard), Standard: `gemini-3.5-flash,gemini-2.5-flash,gemini-flash-lite-latest`

Image: `ghcr.io/mitkiprogrammieren/skizze-zeichnung:latest`
