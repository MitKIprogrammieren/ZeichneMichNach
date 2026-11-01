//! Gesamter Ablauf „Nachzeichnen wie von Hand“ (von App und Kommandozeile genutzt).

use crate::{fonts, ocr};
use nz_core::hand::{self, HandOpts};
use nz_core::vector::{Doc, Opts};
use std::path::Path;

#[derive(Clone, Debug, Default)]
pub struct Info {
    pub matches: Vec<fonts::Match>,
    /// Hinweis, falls die Texterkennung nicht lief
    pub note: Option<String>,
    pub secs: f64,
}

/// Bild nachzeichnen. `path`: Bilddatei für die Texterkennung (None = ohne Text).
pub fn run(doc: &mut Doc, path: Option<&Path>, o: &Opts, h: &HandOpts, k: usize, progress: &dyn Fn(&str, f32)) -> Info {
    let t0 = std::time::Instant::now();
    let mut info = Info::default();
    let mut lines = Vec::new();
    if h.text {
        match path {
            Some(p) if ocr::available() => {
                progress("Text erkennen …", 0.05);
                // dem Helfer immer ein PNG geben – unabhängig von Dateiendung und Format
                let tmp = std::env::temp_dir().join(format!("zmn-ocr-{}.png", std::process::id()));
                let use_path = if std::fs::write(&tmp, nz_core::vector::encode_png(doc.src_w, doc.src_h, &doc.src)).is_ok() { tmp.clone() } else { p.to_path_buf() };
                let res = ocr::recognize(&use_path, doc.src_w, doc.src_h);
                let _ = std::fs::remove_file(&tmp);
                match res {
                    Ok(l) if l.is_empty() => info.note = Some("Texterkennung: kein Text im Bild gefunden".into()),
                    Ok(l) => lines = l,
                    Err(e) => info.note = Some(format!("Texterkennung fehlgeschlagen: {e}")),
                }
            }
            Some(_) => info.note = Some("Texterkennung nicht installiert (Helfer „zmn-ocr“ fehlt in der App) – Text wird als Fläche nachgezeichnet. Installationsskript erneut ausführen.".into()),
            None => info.note = Some("Beispielbild ohne Texterkennung".into()),
        }
    }
    progress("Bild verstehen …", 0.15);
    let boxes: Vec<[f64; 4]> = lines.iter().map(|l| l.bbox).collect();
    hand::prepare(doc, o, h, &boxes);
    let mut texts = Vec::new();
    if !lines.is_empty() {
        let ana: Vec<_> = lines.iter().filter_map(|l| hand::analyze_text(doc, l)).collect();
        if std::env::var_os("ZMN_DEBUG").is_some() {
            for a in &ana {
                eprintln!("    Analyse „{}“: {} Abschnitte {:?}", a.line.text, a.segs.len(), a.segs.iter().map(|s| s.chars.clone()).collect::<Vec<_>>());
            }
        }
        if !ana.is_empty() {
            let (outs, matches) = fonts::match_lines(&ana, &|f| progress("Passende Schrift suchen …", 0.25 + 0.45 * f));
            if matches.is_empty() {
                info.note = Some(format!(
                    "Text erkannt ({}), aber keine Schrift auf diesem Mac passt gut genug – Text wird als Fläche nachgezeichnet.",
                    lines.iter().map(|l| format!("„{}“", l.text)).collect::<Vec<_>>().join(", ")
                ));
            } else if matches.len() < lines.len() {
                let found: Vec<&str> = matches.iter().map(|m| m.text.as_str()).collect();
                let miss: Vec<String> = lines.iter().filter(|l| !found.contains(&l.text.as_str())).map(|l| format!("„{}“", l.text)).collect();
                info.note = Some(format!("Keine passende Schrift für {} – als Fläche nachgezeichnet.", miss.join(", ")));
            }
            texts = outs;
            info.matches = matches;
        }
    }
    progress("Nachzeichnen …", 0.75);
    hand::finish(doc, o, k, h, &texts);
    doc.hand.ocr = lines;
    info.secs = t0.elapsed().as_secs_f64();
    progress("Fertig", 1.0);
    info
}
