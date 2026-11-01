#!/usr/bin/env bash
# ZeichneMichNach auf dem Mac bauen und als „ZeichneMichNach.app“ in den Programme-Ordner legen.
# Beim ersten Mal 5–10 Minuten, Updates schneller.
#
#   bash mac-installieren.sh
#
# Beim ersten Mal öffnet sich der Browser für die GitHub-Anmeldung (kein Passwort im Terminal).
# Der Code liegt danach in ~/ZeichneMichNach-Quellcode. Erneut ausführen = neueste Version holen.
set -euo pipefail

REPO="https://github.com/MitKIprogrammieren/ZeichneMichNach.git"
BRANCH="main"
SRC="$HOME/ZeichneMichNach-Quellcode"
DEST="/Applications/ZeichneMichNach.app"

schritt() { printf '\n\033[1;34m==> %s\033[0m\n' "$*"; }
fehler() { printf '\n\033[1;31mFehler: %s\033[0m\n' "$*" >&2; exit 1; }

[ "$(uname -s)" = Darwin ] || fehler "Dieses Skript ist nur für macOS."

github_anmeldung() {
  if ! command -v gh >/dev/null 2>&1; then
    if command -v brew >/dev/null 2>&1; then
      brew install gh
    else
      local v tmp a
      v=$(curl -fsSLI -o /dev/null -w '%{url_effective}' https://github.com/cli/cli/releases/latest 2>/dev/null | sed -n 's|.*/tag/v||p' || true)
      [ -n "$v" ] || v=2.103.2
      if [ "$(uname -m)" = arm64 ]; then a=arm64; else a=amd64; fi
      tmp=$(mktemp -d)
      curl -fsSL -o "$tmp/gh.zip" "https://github.com/cli/cli/releases/download/v$v/gh_${v}_macOS_${a}.zip" ||
        fehler "GitHub CLI $v konnte nicht geladen werden. Bitte von https://cli.github.com installieren und das Skript erneut starten."
      ditto -x -k "$tmp/gh.zip" "$tmp"
      mkdir -p "$HOME/.local/bin"
      cp "$tmp"/gh_*/bin/gh "$HOME/.local/bin/gh"
      rm -rf "$tmp"
    fi
    export PATH="$HOME/.local/bin:$PATH"
  fi
  if ! gh auth status -h github.com >/dev/null 2>&1; then
    echo "Einmalige Anmeldung bei GitHub: Es öffnet sich der Browser, dort den angezeigten Code eingeben."
    gh auth login -h github.com --web --git-protocol https
  fi
  gh auth setup-git -h github.com
}

# 1. Apple-Entwicklerwerkzeuge (Compiler, Swift, git, iconutil)
schritt "Entwicklerwerkzeuge prüfen"
if ! xcode-select -p >/dev/null 2>&1; then
  xcode-select --install || true
  fehler "Die Apple-Entwicklerwerkzeuge werden gerade installiert (Fenster beachten). Danach dieses Skript noch einmal starten."
fi

# 2. Rust
schritt "Rust prüfen"
if [ -f "$HOME/.cargo/env" ]; then . "$HOME/.cargo/env"; fi
if ! command -v cargo >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
  . "$HOME/.cargo/env"
fi
rustup update stable >/dev/null 2>&1 || true

# 3. Code holen oder aktualisieren
schritt "Code holen"
github_anmeldung
if [ -d "$SRC/.git" ]; then
  git -C "$SRC" fetch --depth 1 origin "$BRANCH"
  git -C "$SRC" checkout -q -B "$BRANCH" FETCH_HEAD
else
  git clone --depth 1 -b "$BRANCH" "$REPO" "$SRC"
fi
cd "$SRC"
echo "Stand: $(git log -1 --format='%h – %s')"

# 4. Bauen: App, Texterkennung (Swift) und Kommandozeilenprogramm
schritt "Bauen (beim ersten Mal 5–10 Minuten)"
export MACOSX_DEPLOYMENT_TARGET=12.0
cargo build --release --manifest-path app/Cargo.toml
cargo build --release --manifest-path tools/zmn-cli/Cargo.toml

schritt "App-Paket zusammensetzen"
WORK="$SRC/dist"
APP="$WORK/ZeichneMichNach.app"
rm -rf "$WORK" && mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp app/target/release/ZeichneMichNach "$APP/Contents/MacOS/ZeichneMichNach"
if swiftc -O -target "$(uname -m)-apple-macos12.0" packaging/ocr/main.swift -o "$APP/Contents/MacOS/zmn-ocr"; then
  echo "Texterkennung eingebaut"
else
  echo "Hinweis: Texterkennung konnte nicht gebaut werden – die App läuft ohne „Text erkennen“."
fi
cp tools/zmn-cli/target/release/zmn "$WORK/zmn"

cp packaging/Info.plist "$APP/Contents/Info.plist"
V=$(grep -m1 '^version' app/Cargo.toml | cut -d'"' -f2)
sed -i '' "s/<string>0\.0\.0<\/string>/<string>$V<\/string>/g" "$APP/Contents/Info.plist"
plutil -lint "$APP/Contents/Info.plist" >/dev/null

ICONSET="$WORK/AppIcon.iconset"
mkdir -p "$ICONSET"
for s in 16 32 64 128 256 512; do
  sips -z "$s" "$s" packaging/icon.png --out "$ICONSET/icon_${s}x${s}.png" >/dev/null
  d=$((s * 2))
  sips -z "$d" "$d" packaging/icon.png --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns"
rm -rf "$ICONSET"

# Ad-hoc-Signatur (Apple Silicon startet keinen unsignierten Code)
codesign --force --deep -s - "$APP"
codesign -s - --force "$WORK/zmn"

schritt "Installieren nach $DEST"
if [ -w /Applications ]; then SUDO=""; else SUDO="sudo"; fi
osascript -e 'tell application "ZeichneMichNach" to quit' >/dev/null 2>&1 || true
$SUDO rm -rf "$DEST"
$SUDO ditto "$APP" "$DEST"
$SUDO xattr -dr com.apple.quarantine "$DEST" 2>/dev/null || true
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$DEST" 2>/dev/null || true

schritt "Fertig – ZeichneMichNach $V"
echo "„ZeichneMichNach.app“ liegt im Programme-Ordner."
echo "Kommandozeile: $WORK/zmn bild.png -o ergebnis.svg"
open "$DEST" || true
