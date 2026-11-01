//! zmn – ZeichneMichNach auf der Kommandozeile.
//! zmn <bild.png> [-o ausgabe.svg] [-k farben] [--ohne text,linien,foto,verlauf,hintergrund]
#[path = "../../../app/src/fonts.rs"]
mod fonts;
#[path = "../../../app/src/ocr.rs"]
mod ocr;
#[path = "../../../app/src/pipeline.rs"]
mod pipeline;

use nz_core::{hand::HandOpts, vector};
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut input = None;
    let mut out = PathBuf::from("nachgezeichnet.svg");
    let mut k = 0usize;
    let mut h = HandOpts::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-o" => {
                i += 1;
                out = PathBuf::from(&args[i]);
            }
            "-k" => {
                i += 1;
                k = args[i].parse().unwrap_or(0);
            }
            "--druck" => {
                h.print = true;
                if let Some(v) = args.get(i + 1).and_then(|v| v.replace(',', ".").parse::<f64>().ok()) {
                    h.print_width_mm = v;
                    i += 1;
                }
            }
            "--farben" => {
                i += 1;
                h.print_colors = args[i].parse().unwrap_or(4);
            }
            "--min" => {
                i += 1;
                h.print_min_mm = args[i].replace(',', ".").parse().unwrap_or(0.8);
            }
            "--ohne" => {
                i += 1;
                for w in args[i].split(',') {
                    match w {
                        "text" => h.text = false,
                        "linien" => h.lines = false,
                        "foto" => h.photo = false,
                        "verlauf" => h.gradients = false,
                        "hintergrund" => h.bg_cut = false,
                        _ => {}
                    }
                }
            }
            a => input = Some(PathBuf::from(a)),
        }
        i += 1;
    }
    let Some(input) = input else {
        eprintln!("Aufruf: zmn <bild.png> [-o ausgabe.svg] [-k farben] [--ohne text,linien,foto,verlauf,hintergrund] [--druck breite_mm] [--farben n] [--min mm]");
        std::process::exit(2);
    };
    let (w, hh, img) = vector::decode_png(&std::fs::read(&input).expect("Bild lesen")).unwrap_or_else(|e| {
        eprintln!("FEHLER: {e}");
        std::process::exit(1)
    });
    let mut d = vector::Doc::new(w, hh, img);
    let o = vector::Opts::default();
    let info = pipeline::run(&mut d, Some(&input), &o, &h, k, &|s, _| eprintln!("  {s}"));
    for m in &info.matches {
        eprintln!("  Text „{}“ → {} {} ({:.0} %)", m.text, m.family, m.styles.join("/"), m.score * 100.0);
    }
    if let Some(n) = &info.note {
        eprintln!("  Hinweis: {n}");
    }
    let (p, n) = d.stats();
    eprintln!("{} Farben, {} Formen, {} Knoten, {} Verläufe, {} Linien, {} Texte, {:.2} s", d.layers.len(), p, n, d.hand.grads.len(), d.hand.lines.len(), d.hand.texts, info.secs);
    let ext = out.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let data = if ext == "png" { d.build_png(&o) } else { d.build_svg(&o, false).into_bytes() };
    std::fs::write(&out, data).expect("schreiben");
    eprintln!("-> {}", out.display());
}
