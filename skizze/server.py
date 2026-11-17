#!/usr/bin/env python3
"""Skizze → Zeichnung als eigener Dienst (ohne Home Assistant).

Liefert die Seite mit der unveränderten HA-Karte aus und ersetzt das, was sonst
Home Assistant erledigt: Gemini-Aufruf, Einstellungen, Bild-Upload, Speichern.
Nur Python-Standardbibliothek.
"""
import base64, json, os, re, threading, time, urllib.error, urllib.request, uuid
from email.parser import BytesParser
from email.policy import default as email_policy
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import quote, unquote, urlparse

HIER = Path(__file__).resolve().parent
DATEN = Path(os.environ.get("DATEN", "/daten"))
UPLOADS = DATEN / "uploads"
ZEICHNUNGEN = DATEN / "zeichnungen"
EINST = DATEN / "einstellungen.json"
SCHLUESSEL = DATEN / "schluessel.json"
PORT = int(os.environ.get("PORT", "8095"))
MODELLE = [m.strip() for m in os.environ.get("GEMINI_MODELLE", "gemini-3.5-flash,gemini-2.5-flash,gemini-flash-lite-latest").split(",") if m.strip()]
API = "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent"
SPERRE = threading.Lock()

for d in (UPLOADS, ZEICHNUNGEN):
    d.mkdir(parents=True, exist_ok=True)


def entity_id(modell):
    return "ai_task." + re.sub(r"[^a-z0-9]+", "_", modell.lower()).strip("_")


def anzeigename(modell):
    return " ".join(w if w[:1].isdigit() else w.capitalize() for w in modell.split("-"))


def schluessel():
    k = os.environ.get("GEMINI_API_KEY", "").strip()
    if k:
        return k
    try:
        return json.loads(SCHLUESSEL.read_text()).get("key", "").strip()
    except Exception:
        return ""


def karte_js():
    t = (HIER / "karte.js").read_text(encoding="utf-8")
    # Texte, die nur in Home Assistant stimmen
    t = t.replace("In Medien speichern", "Auf dem NUC speichern")
    t = t.replace("Gespeichert in <b>Medien → Meine Medien</b> als", "Gespeichert auf dem NUC unter <b>Gespeicherte Zeichnungen</b> als")
    t = t.replace("(für dein Benutzerkonto auf allen Geräten)", "(auf dem NUC, für alle Geräte)")
    t = t.replace("Keine KI-Aufgaben-Entität (ai_task) gefunden", "Kein Gemini-Schlüssel eingetragen")
    t = t.replace("Keine KI eingerichtet (ai_task-Entität fehlt).", "Kein Gemini-Schlüssel eingetragen – oben auf „Gemini-Schlüssel“ tippen.")
    return t.encode("utf-8")


def gemini(modell, anweisung, bild):
    key = schluessel()
    if not key:
        raise RuntimeError("Kein Gemini-Schlüssel eingetragen – oben auf „Gemini-Schlüssel“ tippen.")
    body = {
        "contents": [{"role": "user", "parts": [
            {"text": anweisung},
            {"inline_data": {"mime_type": "image/jpeg", "data": base64.b64encode(bild).decode()}},
        ]}],
        "generationConfig": {"responseMimeType": "application/json", "maxOutputTokens": 8192},
    }
    req = urllib.request.Request(API.format(modell), data=json.dumps(body).encode(), method="POST",
                                 headers={"Content-Type": "application/json", "x-goog-api-key": key})
    try:
        with urllib.request.urlopen(req, timeout=180) as r:
            antwort = json.loads(r.read())
    except urllib.error.HTTPError as e:
        text = e.read().decode("utf-8", "replace")
        try:
            msg = json.loads(text)["error"]["message"]
        except Exception:
            msg = text[:300]
        if e.code == 429:
            raise RuntimeError(f"Gratis-Kontingent für {modell} gerade aufgebraucht – kurz warten oder in den Einstellungen ein anderes Modell wählen. ({msg[:160]})")
        if e.code in (400, 403) and "API key" in msg:
            raise RuntimeError("Der Gemini-Schlüssel wird nicht angenommen – bitte neu eintragen.")
        raise RuntimeError(f"Gemini-Fehler {e.code}: {msg[:300]}")
    teile = (((antwort.get("candidates") or [{}])[0].get("content") or {}).get("parts")) or []
    text = "".join(p.get("text", "") for p in teile if not p.get("thought"))
    if not text.strip():
        grund = (antwort.get("candidates") or [{}])[0].get("finishReason", "")
        raise RuntimeError(f"Gemini hat nichts geliefert ({grund or 'leere Antwort'}).")
    try:
        return json.loads(text)
    except Exception:
        m = re.search(r"\{[\s\S]*\}", text)
        if not m:
            raise RuntimeError("Die KI hat kein lesbares Ergebnis geliefert: " + text[:200])
        return json.loads(m.group(0))


def sicherer_name(n):
    n = Path(n or "").name
    n = re.sub(r"[^\wäöüÄÖÜß,.\- ]+", "_", n).strip(" .") or "datei"
    return n[:120]


class Handler(BaseHTTPRequestHandler):
    server_version = "SkizzeZeichnung/1"

    def log_message(self, fmt, *args):
        if "/api/" in (args[0] if args else ""):
            super().log_message(fmt, *args)

    def senden(self, code, daten, typ="application/json; charset=utf-8", extra=None):
        if isinstance(daten, (dict, list)):
            daten = json.dumps(daten, ensure_ascii=False).encode()
        elif isinstance(daten, str):
            daten = daten.encode()
        self.send_response(code)
        self.send_header("Content-Type", typ)
        self.send_header("Content-Length", str(len(daten)))
        self.send_header("Cache-Control", "no-store")
        for k, v in (extra or {}).items():
            self.send_header(k, v)
        self.end_headers()
        if self.command != "HEAD":
            self.wfile.write(daten)

    def fehler(self, code, text):
        self.senden(code, {"fehler": text})

    def koerper(self):
        n = int(self.headers.get("Content-Length") or 0)
        if n > 40 * 1024 * 1024:
            raise ValueError("Datei zu groß")
        return self.rfile.read(n) if n else b""

    def do_HEAD(self):
        self.do_GET()

    def do_GET(self):
        pfad = urlparse(self.path).path
        if pfad in ("/", "/index.html"):
            return self.senden(200, (HIER / "index.html").read_bytes(), "text/html; charset=utf-8")
        if pfad == "/karte.js":
            return self.senden(200, karte_js(), "text/javascript; charset=utf-8")
        if pfad == "/icon.svg":
            return self.senden(200, (HIER / "icon.svg").read_bytes(), "image/svg+xml")
        if pfad == "/api/status":
            return self.senden(200, {"schluessel": bool(schluessel()), "aus_umgebung": bool(os.environ.get("GEMINI_API_KEY")),
                                     "modelle": [{"id": entity_id(m), "modell": m, "name": anzeigename(m)} for m in MODELLE]})
        if pfad == "/api/einstellungen":
            try:
                return self.senden(200, {"value": json.loads(EINST.read_text())})
            except Exception:
                return self.senden(200, {"value": None})
        if pfad == "/api/zeichnungen":
            dateien = sorted(ZEICHNUNGEN.iterdir(), key=lambda p: p.stat().st_mtime, reverse=True)
            return self.senden(200, [{"name": p.name, "groesse": p.stat().st_size, "zeit": int(p.stat().st_mtime)} for p in dateien if p.is_file()])
        if pfad.startswith("/zeichnungen/"):
            p = ZEICHNUNGEN / sicherer_name(unquote(pfad[len("/zeichnungen/"):]))
            if not p.is_file():
                return self.fehler(404, "nicht gefunden")
            typ = "image/png" if p.suffix.lower() == ".png" else "image/svg+xml" if p.suffix.lower() == ".svg" else "application/octet-stream"
            return self.senden(200, p.read_bytes(), typ, {"Content-Disposition": f"inline; filename*=UTF-8''{quote(p.name)}"})
        if pfad == "/gesund":
            return self.senden(200, {"ok": True})
        self.fehler(404, "nicht gefunden")

    def do_POST(self):
        pfad = urlparse(self.path).path
        try:
            if pfad == "/api/einstellungen":
                wert = json.loads(self.koerper() or b"null")
                with SPERRE:
                    EINST.write_text(json.dumps(wert, ensure_ascii=False, indent=1))
                return self.senden(200, {"ok": True})
            if pfad == "/api/schluessel":
                if os.environ.get("GEMINI_API_KEY"):
                    return self.fehler(400, "Der Schlüssel ist fest im Container eingetragen und kann hier nicht geändert werden.")
                key = (json.loads(self.koerper() or b"{}").get("key") or "").strip()
                with SPERRE:
                    if key:
                        SCHLUESSEL.write_text(json.dumps({"key": key}))
                        os.chmod(SCHLUESSEL, 0o600)
                    elif SCHLUESSEL.exists():
                        SCHLUESSEL.unlink()
                return self.senden(200, {"ok": True, "schluessel": bool(key)})
            if pfad == "/api/hochladen":
                typ = self.headers.get("Content-Type", "")
                msg = BytesParser(policy=email_policy).parsebytes(b"Content-Type: " + typ.encode() + b"\r\n\r\n" + self.koerper())
                for teil in msg.iter_parts():
                    if teil.get_param("name", header="content-disposition") == "file":
                        name = sicherer_name(teil.get_filename() or "bild.jpg")
                        daten = teil.get_payload(decode=True) or b""
                        if name.startswith("skizze_"):
                            ziel = UPLOADS / f"{uuid.uuid4().hex}.jpg"
                            ziel.write_bytes(daten)
                            return self.senden(200, {"media_content_id": "upload/" + ziel.name})
                        ziel = ZEICHNUNGEN / name
                        if ziel.exists():
                            ziel = ZEICHNUNGEN / f"{ziel.stem}_{time.strftime('%Y%m%d-%H%M%S')}{ziel.suffix}"
                        ziel.write_bytes(daten)
                        return self.senden(200, {"media_content_id": "zeichnungen/" + ziel.name})
                return self.fehler(400, "Keine Datei erhalten")
            if pfad == "/api/entfernen":
                mid = json.loads(self.koerper() or b"{}").get("media_content_id", "")
                if mid.startswith("upload/"):
                    (UPLOADS / sicherer_name(mid[7:])).unlink(missing_ok=True)
                return self.senden(200, {"ok": True})
            if pfad == "/api/zeichnung_loeschen":
                name = json.loads(self.koerper() or b"{}").get("name", "")
                (ZEICHNUNGEN / sicherer_name(name)).unlink(missing_ok=True)
                return self.senden(200, {"ok": True})
            if pfad == "/api/lesen":
                d = json.loads(self.koerper() or b"{}")
                ent = d.get("entity_id", "")
                modell = next((m for m in MODELLE if entity_id(m) == ent), MODELLE[0])
                mid = d.get("media_content_id", "")
                bild_pfad = UPLOADS / sicherer_name(mid[7:]) if mid.startswith("upload/") else None
                if not bild_pfad or not bild_pfad.is_file():
                    return self.fehler(400, "Bild nicht gefunden – bitte nochmal auswählen.")
                t0 = time.time()
                daten = gemini(modell, d.get("instructions", ""), bild_pfad.read_bytes())
                print(f"Skizze gelesen mit {modell} in {time.time() - t0:.1f} s", flush=True)
                return self.senden(200, {"response": {"data": daten}})
            self.fehler(404, "nicht gefunden")
        except RuntimeError as e:
            self.fehler(502, str(e))
        except Exception as e:
            self.fehler(500, f"{type(e).__name__}: {e}")


def aufraeumen():
    while True:
        grenze = time.time() - 3600
        for p in UPLOADS.glob("*.jpg"):
            try:
                if p.stat().st_mtime < grenze:
                    p.unlink()
            except Exception:
                pass
        time.sleep(600)


if __name__ == "__main__":
    threading.Thread(target=aufraeumen, daemon=True).start()
    print(f"Skizze → Zeichnung läuft auf Port {PORT} · Modelle: {', '.join(MODELLE)}", flush=True)
    ThreadingHTTPServer(("0.0.0.0", PORT), Handler).serve_forever()
