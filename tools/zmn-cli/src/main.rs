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
    let mut b3 = nz_core::relief::Build3d::default();
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
            "--grund" => {
                i += 1;
                b3.base_mm = args[i].replace(',', ".").parse().unwrap_or(1.6);
                b3.base = b3.base_mm > 0.0;
            }
            "--rand" => {
                i += 1;
                b3.rim_w_mm = args[i].replace(',', ".").parse().unwrap_or(1.0);
                b3.rim = b3.rim_w_mm > 0.0;
            }
            "--hoehe" => {
                i += 1;
                b3.uniform_mm = args[i].replace(',', ".").parse().unwrap_or(0.6);
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
        eprintln!("Aufruf: zmn <bild.png> [-o ausgabe.svg] [-k farben] [--ohne text,linien,foto,verlauf,hintergrund] [--druck breite_mm] [--farben n] [--min mm] [--grund mm] [--rand mm] [--hoehe mm]  (Ausgabe .svg .png .step .3mf)");
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
    b3.width_mm = if h.print { h.print_width_mm } else { b3.width_mm };
    b3.rim_h_mm = b3.base_mm;
    let stem = out.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let data = match ext.as_str() {
        "png" => d.build_png(&o),
        "step" | "stp" | "3mf" => {
            let t = std::time::Instant::now();
            let r = if ext == "3mf" {
                let (parts, _cols) = nz_core::relief::build_parts(&d, &o, &b3);
                eprintln!("  3D: {} Körper ({})", parts.len(), parts.iter().map(|p| p.name.clone()).collect::<Vec<_>>().join(", "));
                nz_core::export3mf::write_3mf(&parts, &[stem.clone()], &nz_core::export3mf::ExportOpts::default())
            } else {
                let bodies = nz_core::relief::bodies(&d, &o, &b3);
                eprintln!("  3D: {} Körper ({})", bodies.len(), bodies.iter().map(|b| b.name.clone()).collect::<Vec<_>>().join(", "));
                nz_core::relief::step_from_bodies(&bodies, b3.width_mm, d.src_w, d.src_h, &stem, &out.file_name().unwrap().to_string_lossy()).into_bytes()
            };
            eprintln!("  3D fertig in {:.1} s", t.elapsed().as_secs_f64());
            r
        }
        _ => d.build_svg(&o, false).into_bytes(),
    };
    std::fs::write(&out, data).expect("schreiben");
    eprintln!("-> {}", out.display());
}
