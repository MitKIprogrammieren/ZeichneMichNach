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
                match ocr::recognize(p, doc.src_w, doc.src_h) {
                    Ok(l) => lines = l,
                    Err(e) => info.note = Some(e),
                }
            }
            _ => info.note = Some("Texterkennung gibt es nur auf dem Mac".into()),
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
