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

/// Textzeilen im Bild finden. Unter Linux (Tesseract) wird das Bild zusätzlich je Farbe
/// in eine Schwarz-Weiß-Maske zerlegt – Tesseract liest farbige Schrift auf farbigem
/// Grund schlecht, eine saubere Maske dagegen sehr gut.
pub fn recognize_rgba(image: &Path, w: usize, h: usize, rgba: &[u8]) -> Result<Vec<TextLine>, String> {
    if !cfg!(target_os = "linux") || std::env::var_os("ZMN_OCR_JSON").is_some() {
        return recognize(image, w, h);
    }
    // Tesseract rät in unruhigen Bildern (Fotos, Illustrationen) gern Text in Formen hinein –
    // dort nur sehr sichere Zeilen nehmen
    let busy = !nz_core::hand::is_flat(rgba, w, h);
    let min_conf = if busy { 0.85 } else { 0.6 };
    // Masken je Farbe vorbereiten, dann alle Durchgänge (ganzes Bild + Masken) parallel –
    // Tesseract rechnet je Aufruf nur auf einem Kern
    static ZAEHLER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let lauf = ZAEHLER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut bilder: Vec<(std::path::PathBuf, bool)> = vec![(image.to_path_buf(), false)];
    let q = nz_core::vector::quantize(rgba, w, h);
    if !q.pal.is_empty() {
        // ein paar Farben mehr als automatisch – Schrift mit wenig Kontrast soll nicht
        // mit ihrem Untergrund verschmelzen
        let k = (q.auto + 2).clamp(3, 8).min(q.pal.len());
        for (gi, g) in q.presets[k].iter().enumerate() {
            let mut img = vec![255u8; w * h * 4];
            let mut cnt = 0usize;
            for p in 0..w * h {
                let l = q.labels[p];
                if l >= 0 && g.contains(&(l as usize)) && rgba[p * 4 + 3] >= 128 {
                    img[p * 4] = 0;
                    img[p * 4 + 1] = 0;
                    img[p * 4 + 2] = 0;
                    cnt += 1;
                }
            }
            // sehr große Flächen (Hintergrund) und Krümel überspringen
            if cnt < 50 || cnt > w * h * 6 / 10 {
                continue;
            }
            let tmp = std::env::temp_dir().join(format!("zmn-ocr-{}-{lauf}-{gi}.png", std::process::id()));
            if std::fs::write(&tmp, nz_core::vector::encode_png(w, h, &img)).is_ok() {
                bilder.push((tmp, true));
            }
        }
    }
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2).max(1);
    let naechstes = std::sync::atomic::AtomicUsize::new(0);
    let all = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|sc| {
        for _ in 0..threads.min(bilder.len()) {
            sc.spawn(|| loop {
                let i = naechstes.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let Some((pfad, temp)) = bilder.get(i) else { break };
                if let Ok(lines) = recognize_min(pfad, w, h, min_conf) {
                    all.lock().unwrap().extend(lines);
                }
                if *temp {
                    let _ = std::fs::remove_file(pfad);
                }
            });
        }
    });
    let mut all = all.into_inner().unwrap();
    // Tesseract liest in Formen gern einzelne Zeichen hinein („m @“ in einem Stern):
    // nur Zeilen mit mindestens zwei Buchstaben/Ziffern und überwiegend Schrift behalten
    all.retain(|l| {
        let t: Vec<char> = l.text.chars().filter(|c| !c.is_whitespace()).collect();
        let an = t.iter().filter(|c| c.is_alphanumeric()).count();
        an >= if busy { 4 } else { 2 } && an * 10 >= t.len() * 7
    });
    // doppelte Funde (gleiche Stelle) zusammenlegen – der längere Text gewinnt
    let mut out: Vec<TextLine> = Vec::new();
    for l in all {
        let over = |a: &[f64; 4], b: &[f64; 4]| {
            let ix = (a[2].min(b[2]) - a[0].max(b[0])).max(0.0);
            let iy = (a[3].min(b[3]) - a[1].max(b[1])).max(0.0);
            let ar = ((a[2] - a[0]) * (a[3] - a[1])).min((b[2] - b[0]) * (b[3] - b[1])).max(1.0);
            ix * iy / ar > 0.5
        };
        if let Some(o) = out.iter_mut().find(|o| over(&o.bbox, &l.bbox)) {
            if l.text.chars().filter(|c| c.is_alphanumeric()).count() > o.text.chars().filter(|c| c.is_alphanumeric()).count() {
                *o = l;
            }
        } else {
            out.push(l);
        }
    }
    Ok(out)
}

/// Textzeilen im Bild finden (Rechtecke in Pixeln des Quellbilds).
pub fn recognize(image: &Path, w: usize, h: usize) -> Result<Vec<TextLine>, String> {
    recognize_min(image, w, h, 0.3)
}

/// Wie [`recognize`], aber nur Zeilen ab dieser Sicherheit (0…1).
fn recognize_min(image: &Path, w: usize, h: usize, min_conf: f64) -> Result<Vec<TextLine>, String> {
    let json = if let Some(f) = std::env::var_os("ZMN_OCR_JSON") {
        std::fs::read_to_string(f).map_err(|e| e.to_string())?
    } else {
        let hp = helper().ok_or("Texterkennung nicht installiert")?;
        let out = std::process::Command::new(hp).arg(image).output().map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(format!("Code {} – {}", out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stderr).trim()));
        }
        String::from_utf8_lossy(&out.stdout).to_string()
    };
    let v = nz_core::json::parse(json.trim()).map_err(|e| format!("Antwort der Texterkennung: {e}"))?;
    let mut lines = Vec::new();
    for it in v.arr() {
        let text = it.get("text").and_then(|t| t.str()).unwrap_or("").trim().to_string();
        let conf = it.get("conf").and_then(|c| c.num()).unwrap_or(1.0);
        let num = |k: &str| it.get(k).and_then(|x| x.num()).unwrap_or(0.0);
        if text.chars().filter(|c| c.is_alphanumeric()).count() == 0 || conf < min_conf {
            continue;
        }
        let (x, y, bw, bh) = (num("x"), num("y"), num("w"), num("h"));
        // Werte 0…1 oder schon Pixel
        let (sx, sy) = if x + bw <= 1.01 && y + bh <= 1.01 { (w as f64, h as f64) } else { (1.0, 1.0) };
        lines.push(TextLine { text, bbox: [x * sx, y * sy, (x + bw) * sx, (y + bh) * sy] });
    }
    Ok(lines)
}
