#!/usr/bin/env bash
# Lädt die Schriften aus liste.txt aus github.com/google/fonts (alle unter SIL Open Font License).
# Nur die benötigten Ordner werden geholt (sparse checkout), kein API-Schlüssel nötig.
# Aufruf: tools/schriften/laden.sh <zielordner>
set -euo pipefail
hier="$(cd "$(dirname "$0")" && pwd)"
ziel="$(mkdir -p "${1:-$hier/../../packaging/web/schriften}" && cd "${1:-$hier/../../packaging/web/schriften}" && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
git clone -q --depth 1 --filter=blob:none --sparse https://github.com/google/fonts.git "$tmp/gf"
mapfile -t fams < <(grep -v '^#' "$hier/liste.txt" | sed 's/[[:space:]]//g' | grep .)
muster=()
for f in "${fams[@]}"; do for d in ofl apache ufl; do muster+=("/$d/$f/*.ttf" "/$d/$f/*.txt"); done; done
(cd "$tmp/gf" && git sparse-checkout set --no-cone "${muster[@]}")
ok=0; fehlt=()
for f in "${fams[@]}"; do
  q=""; for d in ofl apache ufl; do ls "$tmp/gf/$d/$f/"*.ttf >/dev/null 2>&1 && { q="$tmp/gf/$d/$f"; break; }; done
  if [ -n "$q" ]; then
    mkdir -p "$ziel/$f"; cp "$q/"*.ttf "$ziel/$f/"; cp "$q/"*.txt "$ziel/$f/" 2>/dev/null || true
    ok=$((ok+1))
  else fehlt+=("$f"); fi
done
echo "Schriften: $ok Familien → $ziel ($(du -sh "$ziel" | cut -f1))"
[ ${#fehlt[@]} -gt 0 ] && echo "nicht gefunden: ${fehlt[*]}"
exit 0
