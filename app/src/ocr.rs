//! Texterkennung mit der Apple-Texterkennung (Vision), komplett lokal.
//! Ein kleiner Helfer `zmn-ocr` (Swift, liegt neben der App) liefert die Zeilen
//! als JSON: [{"text", "conf", "x", "y", "w", "h"}] mit Werten 0…1, y von oben.
//! Für Tests ohne Mac: Umgebungsvariable ZMN_OCR_JSON mit einer solchen Datei.

use nz_core::hand::TextLine;
use std::path::{Path, PathBuf};

fn helper() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    for p in [dir.join("zmn-ocr"), dir.join("../Resources/zmn-ocr")] {
        if p.exists() {
            return Some(p);
        }
    }
    None
}

/// Steht die Texterkennung zur Verfügung?
pub fn available() -> bool {
    std::env::var_os("ZMN_OCR_JSON").is_some() || helper().is_some()
}

/// Textzeilen im Bild finden (Rechtecke in Pixeln des Quellbilds).
pub fn recognize(image: &Path, w: usize, h: usize) -> Result<Vec<TextLine>, String> {
    let json = if let Some(f) = std::env::var_os("ZMN_OCR_JSON") {
        std::fs::read_to_string(f).map_err(|e| e.to_string())?
    } else {
        let hp = helper().ok_or("Texterkennung nicht installiert")?;
        let out = std::process::Command::new(hp).arg(image).output().map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(format!("Texterkennung fehlgeschlagen: {}", String::from_utf8_lossy(&out.stderr).trim()));
        }
        String::from_utf8_lossy(&out.stdout).to_string()
    };
    let v = nz_core::json::parse(json.trim()).map_err(|e| format!("Antwort der Texterkennung: {e}"))?;
    let mut lines = Vec::new();
    for it in v.arr() {
        let text = it.get("text").and_then(|t| t.str()).unwrap_or("").trim().to_string();
        let conf = it.get("conf").and_then(|c| c.num()).unwrap_or(1.0);
        let num = |k: &str| it.get(k).and_then(|x| x.num()).unwrap_or(0.0);
        if text.chars().filter(|c| c.is_alphanumeric()).count() == 0 || conf < 0.3 {
            continue;
        }
        let (x, y, bw, bh) = (num("x"), num("y"), num("w"), num("h"));
        // Werte 0…1 oder schon Pixel
        let (sx, sy) = if x + bw <= 1.01 && y + bh <= 1.01 { (w as f64, h as f64) } else { (1.0, 1.0) };
        lines.push(TextLine { text, bbox: [x * sx, y * sy, (x + bw) * sx, (y + bh) * sy] });
    }
    Ok(lines)
}
