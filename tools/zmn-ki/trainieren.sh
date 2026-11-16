#!/bin/bash
# Kanten-KI für ZeichneMichNach auf dem Mac trainieren (Grafikchip, Apple MPS).
#
#   curl -fsSL https://raw.githubusercontent.com/MitKIprogrammieren/ZeichneMichNach/web-container/tools/zmn-ki/trainieren.sh | bash
#
# Holt den Quellcode nach ~/ZeichneMichNach-KI, erzeugt Trainingsbilder, trainiert das
# kleine Netz (ca. 20–40 Minuten), prüft es und lädt die Gewichte auf GitHub hoch.
# Danach baut GitHub den Container neu – die KI läuft dann auch auf dem NUC.
set -euo pipefail

BRANCH="${ZMN_BRANCH:-web-container}"
REPO="https://github.com/MitKIprogrammieren/ZeichneMichNach.git"
ZIEL="$HOME/ZeichneMichNach-KI"
ANZAHL="${ANZAHL:-6000}"
EPOCHEN="${EPOCHEN:-30}"

schritt() { printf '\n\033[1;34m▶ %s\033[0m\n' "$1"; }
fehler() { printf '\n\033[1;31m✖ %s\033[0m\n' "$1" >&2; exit 1; }

[ "$(uname -s)" = "Darwin" ] || fehler "Dieses Skript ist für den Mac gedacht."
[ "$(uname -m)" = "arm64" ] || echo "Hinweis: kein Apple-Chip – das Training läuft auf dem Prozessor und dauert deutlich länger."

schritt "1/6 Werkzeuge prüfen"
if [ -f "$HOME/.cargo/env" ]; then . "$HOME/.cargo/env"; fi
command -v git >/dev/null || fehler "git fehlt – bitte zuerst scripts/mac-installieren.sh ausführen (installiert die Apple-Entwicklerwerkzeuge)."
if ! command -v cargo >/dev/null; then
  echo "Rust wird installiert …"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
  . "$HOME/.cargo/env"
fi
export PATH="$HOME/.local/bin:$PATH"
if ! command -v uv >/dev/null; then
  echo "uv (Python-Verwaltung) wird installiert …"
  curl -LsSf https://astral.sh/uv/install.sh | sh
  export PATH="$HOME/.local/bin:$PATH"
fi

schritt "2/6 Quellcode holen ($BRANCH)"
if [ -d "$ZIEL/.git" ]; then
  git -C "$ZIEL" fetch -q origin "$BRANCH"
  git -C "$ZIEL" checkout -q "$BRANCH"
  git -C "$ZIEL" reset -q --hard "origin/$BRANCH"
else
  git clone -q -b "$BRANCH" "$REPO" "$ZIEL"
fi
cd "$ZIEL/tools/zmn-ki"

schritt "3/6 Python mit PyTorch einrichten (einmalig, ca. 1 GB)"
[ -d .venv ] || uv venv -q --python 3.12 .venv
uv pip install -q --python .venv/bin/python torch numpy pillow

schritt "4/6 Trainingsbilder erzeugen ($ANZAHL Stück)"
cargo build -q --release
if [ "$(ls "$ZIEL/ki-daten" 2>/dev/null | grep -c '_ein.png' || true)" -lt "$ANZAHL" ]; then
  ./target/release/zmn-ki daten "$ZIEL/ki-daten" --anzahl "$ANZAHL"
else
  echo "Trainingsbilder sind schon da."
fi

schritt "5/6 Trainieren ($EPOCHEN Durchgänge)"
GEW="$ZIEL/core/ki/kanten.bin"
.venv/bin/python trainieren.py "$ZIEL/ki-daten" "$GEW" --schnelltest >/dev/null || fehler "Schnelltest fehlgeschlagen – bitte die Ausgabe an Claude schicken."
caffeinate -i .venv/bin/python trainieren.py "$ZIEL/ki-daten" "$GEW" --epochen "$EPOCHEN" | tee "$ZIEL/training.log"
echo "Abgleich mit der Rust-Rechnung …"
.venv/bin/python abgleich.py "$GEW" || fehler "Rust und PyTorch rechnen unterschiedlich – bitte Claude Bescheid geben."

schritt "6/6 Gewichte hochladen"
cd "$ZIEL"
cp training.log tools/zmn-ki/letztes-training.log
git add core/ki/kanten.bin tools/zmn-ki/letztes-training.log
if git diff --cached --quiet; then
  echo "Keine Änderung an den Gewichten."
else
  F1=$(grep -o 'bestes F1 [0-9.]* %' training.log | tail -1)
  git -c user.name=MitKIprogrammieren -c user.email=129672797+MitKIprogrammieren@users.noreply.github.com \
    commit -q -m "Kanten-KI trainiert ($F1)"
  git push -q origin "$BRANCH" || fehler "Hochladen fehlgeschlagen – ist GitHub angemeldet? (gh auth login)"
fi
printf '\n\033[1;32m✔ Fertig. %s – sag Claude Bescheid, dann wird die KI eingebaut und getestet.\033[0m\n' "$(grep -o 'bestes F1 [0-9.]* %' training.log | tail -1)"
