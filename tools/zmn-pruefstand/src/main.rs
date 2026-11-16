//! zmn-pruefstand – Testbilder mit bekanntem Original erzeugen, nachzeichnen und die Abweichung messen.
//!
//!   zmn-pruefstand <ordner> [--anzahl n] [--nur art,art] [--ohne-vtracer]
//!
//! Für jedes Testbild gibt es das Original (SVG) und drei Eingaben: sauber, klein (160 px) und
//! jpeg (unscharf, Rauschen, starke Kompression). Gemessen wird gegen das saubere Original:
//! Farbfehler, Fehlpixel, Kantenfehler, Knotenzahl und Zeit. Ergebnis: bericht.html und werte.csv.
#[path = "../../../app/src/fonts.rs"]
mod fonts;
#[path = "../../../app/src/ocr.rs"]
mod ocr;
#[path = "../../../app/src/pipeline.rs"]
mod pipeline;
mod gen;

use nz_core::hand::HandOpts;
use nz_core::vector::{self, Doc, Opts};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

const N: u32 = 400;

fn fontdb() -> Arc<resvg::usvg::fontdb::Database> {
    let mut db = resvg::usvg::fontdb::Database::new();
    db.load_system_fonts();
    db.set_sans_serif_family("DejaVu Sans");
    Arc::new(db)
}

/// SVG in RGBA rendern (auf Größe w×h eingepasst, transparent).
fn render(svg: &str, w: u32, h: u32, db: &Arc<resvg::usvg::fontdb::Database>) -> Result<Vec<u8>, String> {
    let mut opt = resvg::usvg::Options::default();
    opt.fontdb = db.clone();
    let tree = resvg::usvg::Tree::from_str(svg, &opt).map_err(|e| e.to_string())?;
    let mut pm = resvg::tiny_skia::Pixmap::new(w, h).ok_or("Pixmap")?;
    let s = tree.size();
    let t = resvg::tiny_skia::Transform::from_scale(w as f32 / s.width(), h as f32 / s.height());
    resvg::render(&tree, t, &mut pm.as_mut());
    // vormultipliziert → gerade
    let mut d = pm.take();
    for p in d.chunks_mut(4) {
        let a = p[3] as u32;
        if a > 0 && a < 255 {
            for c in 0..3 {
                p[c] = ((p[c] as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    Ok(d)
}

fn on_white(d: &[u8]) -> Vec<[f32; 3]> {
    d.chunks(4)
        .map(|p| {
            let a = p[3] as f32 / 255.0;
            [p[0] as f32 * a + 255.0 * (1.0 - a), p[1] as f32 * a + 255.0 * (1.0 - a), p[2] as f32 * a + 255.0 * (1.0 - a)]
        })
        .collect()
}

fn blur(d: &[u8], w: usize, h: usize) -> Vec<u8> {
    let k = [1.0f32, 4.0, 6.0, 4.0, 1.0];
    let mut t = vec![0f32; d.len()];
    let mut o = vec![0u8; d.len()];
    for y in 0..h {
        for x in 0..w {
            for c in 0..4 {
                let mut s = 0.0;
                for (i, kv) in k.iter().enumerate() {
                    let xx = (x as i32 + i as i32 - 2).clamp(0, w as i32 - 1) as usize;
                    s += kv * d[(y * w + xx) * 4 + c] as f32;
                }
                t[(y * w + x) * 4 + c] = s / 16.0;
            }
        }
    }
    for y in 0..h {
        for x in 0..w {
            for c in 0..4 {
                let mut s = 0.0;
                for (i, kv) in k.iter().enumerate() {
                    let yy = (y as i32 + i as i32 - 2).clamp(0, h as i32 - 1) as usize;
                    s += kv * t[(yy * w + x) * 4 + c];
                }
                o[(y * w + x) * 4 + c] = (s / 16.0).round() as u8;
            }
        }
    }
    o
}

/// Eingabe verschlechtern wie ein echtes Foto/Bildschirmfoto: Unschärfe, Rauschen, JPEG.
fn degrade_jpeg(d: &[u8], w: usize, h: usize, seed: u64) -> Vec<u8> {
    let white = on_white(d);
    let mut rgba: Vec<u8> = white.iter().flat_map(|p| [p[0] as u8, p[1] as u8, p[2] as u8, 255]).collect();
    rgba = blur(&rgba, w, h);
    let mut r = gen::Rng::new(seed);
    for p in rgba.chunks_mut(4) {
        for c in 0..3 {
            let n = (r.f() + r.f() + r.f() - 1.5) * 14.0;
            p[c] = (p[c] as f64 + n).clamp(0.0, 255.0) as u8;
        }
    }
    let rgb: Vec<u8> = rgba.chunks(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
    let mut buf = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 55).encode(&rgb, w as u32, h as u32, image::ExtendedColorType::Rgb8).unwrap();
    let img = image::load_from_memory(&buf).unwrap().to_rgba8();
    img.into_raw()
}

struct Res {
    svg: String,
    secs: f64,
}

fn run_kern(rgba: &[u8], w: usize, h: usize) -> Res {
    let t = Instant::now();
    let o = Opts::default();
    let mut d = Doc::new(w, h, rgba.to_vec());
    d.run(0, &o, 0);
    Res { svg: d.build_svg(&o, false), secs: t.elapsed().as_secs_f64() }
}

fn run_hand(rgba: &[u8], w: usize, h: usize, input: &Path, hh: HandOpts) -> Res {
    let t = Instant::now();
    let o = Opts::default();
    let mut d = Doc::new(w, h, rgba.to_vec());
    pipeline::run(&mut d, Some(input), &o, &hh, 0, &|_, _| {});
    Res { svg: d.build_svg(&o, false), secs: t.elapsed().as_secs_f64() }
}

fn run_vtracer(rgba: &[u8], w: usize, h: usize) -> Res {
    let t = Instant::now();
    let img = vtracer::ColorImage { pixels: rgba.to_vec(), width: w, height: h };
    let svg = match vtracer::convert(img, vtracer::Config::default()) {
        Ok(s) => s.to_string(),
        Err(e) => format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\"><!-- {e} --></svg>"),
    };
    Res { svg, secs: t.elapsed().as_secs_f64() }
}

#[derive(Clone, Default)]
struct Score {
    farbe: f64,  // mittlerer Farbfehler in %
    fehl: f64,   // Anteil deutlich falscher Pixel in %
    kante: f64,  // mittlerer Abstand Kante→Kante in Pixeln (beide Richtungen)
    knoten: usize,
    secs: f64,
    /// Anteil der Farbfläche schmaler als 0,8 mm (0,4er Düse) bei 50 mm Breite, in %
    schmal: f64,
}

fn edges(p: &[[f32; 3]], w: usize, h: usize) -> Vec<bool> {
    let mut e = vec![false; w * h];
    for y in 0..h - 1 {
        for x in 0..w - 1 {
            let a = p[y * w + x];
            let d = |b: [f32; 3]| (a[0] - b[0]).abs().max((a[1] - b[1]).abs()).max((a[2] - b[2]).abs());
            if d(p[y * w + x + 1]) > 60.0 || d(p[(y + 1) * w + x]) > 60.0 {
                e[y * w + x] = true;
            }
        }
    }
    e
}

/// Abstandsfeld (Chamfer 3-4) zu den markierten Pixeln.
fn dist(e: &[bool], w: usize, h: usize) -> Vec<f32> {
    let inf = 1e6f32;
    let mut d: Vec<f32> = e.iter().map(|&b| if b { 0.0 } else { inf }).collect();
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let mut v = d[i];
            if x > 0 {
                v = v.min(d[i - 1] + 1.0);
            }
            if y > 0 {
                v = v.min(d[i - w] + 1.0);
                if x > 0 {
                    v = v.min(d[i - w - 1] + 1.414);
                }
                if x + 1 < w {
                    v = v.min(d[i - w + 1] + 1.414);
                }
            }
            d[i] = v;
        }
    }
    for y in (0..h).rev() {
        for x in (0..w).rev() {
            let i = y * w + x;
            let mut v = d[i];
            if x + 1 < w {
                v = v.min(d[i + 1] + 1.0);
            }
            if y + 1 < h {
                v = v.min(d[i + w] + 1.0);
                if x + 1 < w {
                    v = v.min(d[i + w + 1] + 1.414);
                }
                if x > 0 {
                    v = v.min(d[i + w - 1] + 1.414);
                }
            }
            d[i] = v;
        }
    }
    d
}

fn knoten(svg: &str) -> usize {
    // Pfadbefehle zählen (jeder Befehl = ein Knoten), Kreise/Ellipsen/Rechtecke als 4
    let mut n = 0;
    let mut rest = svg;
    while let Some(i) = rest.find(" d=\"") {
        let s = &rest[i + 4..];
        let e = s.find('"').unwrap_or(s.len());
        let d = &s[..e];
        // jeder Buchstabe außer e/E (Exponent) beginnt einen Befehl; Wiederholungen ohne Buchstabe zählen über Zahlenpaare
        let mut cmd = ' ';
        let mut nums = 0usize;
        let per = |c: char| match c.to_ascii_uppercase() {
            'C' => 6,
            'S' | 'Q' => 4,
            'A' => 7,
            'H' | 'V' => 1,
            'Z' => 0,
            _ => 2,
        };
        let flush = |cmd: char, nums: usize, n: &mut usize| {
            let p = per(cmd);
            *n += if p == 0 { 0 } else { (nums / p).max(1) };
        };
        let mut tok = String::new();
        for ch in d.chars() {
            if ch.is_ascii_alphabetic() && ch != 'e' && ch != 'E' {
                if !tok.is_empty() {
                    nums += 1;
                    tok.clear();
                }
                if cmd != ' ' {
                    flush(cmd, nums, &mut n);
                }
                cmd = ch;
                nums = 0;
            } else if ch == ' ' || ch == ',' || (ch == '-' && !tok.is_empty() && !tok.ends_with('e')) {
                if !tok.is_empty() {
                    nums += 1;
                    tok.clear();
                }
                if ch == '-' {
                    tok.push(ch);
                }
            } else {
                tok.push(ch);
            }
        }
        if !tok.is_empty() {
            nums += 1;
        }
        if cmd != ' ' {
            flush(cmd, nums, &mut n);
        }
        rest = &s[e..];
    }
    n + 4 * (svg.matches("<circle").count() + svg.matches("<ellipse").count() + svg.matches("<rect").count())
}

fn score(truth: &[[f32; 3]], truth_e: &[f32], truth_edges: &[bool], out_svg: &str, db: &Arc<resvg::usvg::fontdb::Database>, secs: f64) -> (Score, Vec<u8>) {
    let n = N as usize;
    let rendered = render(out_svg, N, N, db).unwrap_or_else(|_| vec![0; n * n * 4]);
    let p = on_white(&rendered);
    let mut sum = 0.0;
    let mut bad = 0usize;
    for (a, b) in truth.iter().zip(&p) {
        let d = (a[0] - b[0]).abs() + (a[1] - b[1]).abs() + (a[2] - b[2]).abs();
        sum += d as f64 / (3.0 * 255.0);
        if (a[0] - b[0]).abs().max((a[1] - b[1]).abs()).max((a[2] - b[2]).abs()) > 48.0 {
            bad += 1;
        }
    }
    let oe = edges(&p, n, n);
    let od = dist(&oe, n, n);
    let (mut s1, mut c1, mut s2, mut c2) = (0.0f64, 0usize, 0.0f64, 0usize);
    for i in 0..n * n {
        if truth_edges[i] {
            s1 += od[i].min(20.0) as f64;
            c1 += 1;
        }
        if oe[i] {
            s2 += truth_e[i].min(20.0) as f64;
            c2 += 1;
        }
    }
    let kante = (s1 / c1.max(1) as f64 + s2 / c2.max(1) as f64) / 2.0;
    (
        Score { farbe: 100.0 * sum / (n * n) as f64, fehl: 100.0 * bad as f64 / (n * n) as f64, kante, knoten: knoten(out_svg), secs, schmal: schmal(out_svg, db, 0.8) },
        rendered,
    )
}

/// Druckbarkeit: Ergebnis auf 50 mm Breite (16 px/mm) rendern, je Farbe rund öffnen
/// (Radius = halbe Mindestbreite) – was dabei verschwindet, ist zu schmal oder zu spitz.
fn schmal(svg: &str, db: &Arc<resvg::usvg::fontdb::Database>, min_mm: f64) -> f64 {
    let n = 800usize;
    let Ok(img) = render(svg, n as u32, n as u32, db) else { return 0.0 };
    let r = (min_mm * 16.0 / 2.0) as i64;
    let mut cols: std::collections::HashMap<[u8; 3], Vec<usize>> = Default::default();
    for p in 0..n * n {
        if img[p * 4 + 3] == 255 {
            cols.entry([img[p * 4], img[p * 4 + 1], img[p * 4 + 2]]).or_default().push(p);
        }
    }
    let (mut tot, mut lost) = (0usize, 0usize);
    for (_, px) in cols {
        if px.len() < n * n / 2000 {
            continue;
        }
        let mut m = vec![false; n * n];
        for &p in &px {
            m[p] = true;
        }
        // Abstand nach innen → Erosion; dann Dilatation der Kerne
        let inv: Vec<bool> = m.iter().map(|&b| !b).collect();
        let din = dist(&inv, n, n);
        let core: Vec<bool> = din.iter().map(|&d| d > r as f32).collect();
        let dc = dist(&core, n, n);
        for &p in &px {
            tot += 1;
            if dc[p] > r as f32 + 1.0 {
                lost += 1;
            }
        }
    }
    100.0 * lost as f64 / tot.max(1) as f64
}

/// Eigene Referenzbilder: Original (PNG/JPG) + optional fremde SVG gleichen Namens
/// (z. B. von vectorizer.ai). Beide werden gegen das Original gemessen.
fn referenzen(dir: &Path, out: &Path, db: &Arc<resvg::usvg::fontdb::Database>) -> String {
    let mut rep = String::new();
    let Ok(rd) = std::fs::read_dir(dir) else { return rep };
    let mut files: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| matches!(p.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).as_deref(), Some("png" | "jpg" | "jpeg" | "webp"))).collect();
    files.sort();
    for f in files {
        let Ok(img) = image::open(&f) else { continue };
        let img = img.to_rgba8();
        let (w, h) = (img.width() as usize, img.height() as usize);
        let rgba = img.into_raw();
        let truth = on_white(&rgba);
        let stem = f.file_stem().unwrap().to_string_lossy().to_string();
        // freigestellte Bereiche der SVG auf der Randfarbe des Originals zeigen
        let bgc = {
            let mut c = [0f64; 3];
            let mut nn = 0.0;
            for x in 0..w {
                for y in [0, h - 1] {
                    for k in 0..3 {
                        c[k] += truth[y * w + x][k] as f64;
                    }
                    nn += 1.0;
                }
            }
            c.map(|v| (v / nn) as f32)
        };
        let measure = |svg: &str| -> (f64, f64, usize) {
            let r = render(svg, w as u32, h as u32, db).unwrap_or_else(|_| vec![0; w * h * 4]);
            let p: Vec<[f32; 3]> = r.chunks(4).map(|q| {
                let a = q[3] as f32 / 255.0;
                [0, 1, 2].map(|k| q[k] as f32 * a + bgc[k] * (1.0 - a))
            }).collect();
            let (mut sum, mut bad) = (0.0f64, 0usize);
            for (a, b) in truth.iter().zip(&p) {
                let d = (0..3).map(|k| (a[k] - b[k]).abs()).fold(0.0f32, f32::max);
                sum += (0..3).map(|k| (a[k] - b[k]).abs() as f64).sum::<f64>() / (3.0 * 255.0);
                if d > 48.0 {
                    bad += 1;
                }
            }
            (100.0 * sum / (w * h) as f64, 100.0 * bad as f64 / (w * h) as f64, knoten(svg))
        };
        let t = Instant::now();
        // Hintergrund bleibt – das Original hat ihn auch
        let r = run_hand(&rgba, w, h, &f, HandOpts { bg_cut: false, ..HandOpts::default() });
        let secs = t.elapsed().as_secs_f64();
        let _ = std::fs::write(out.join(format!("{stem}-zmn.svg")), &r.svg);
        let (fa, fe, kn) = measure(&r.svg);
        let _ = writeln!(rep, "{stem:20} ZeichneMichNach  farbe {fa:5.2}  fehl {fe:5.2}  knoten {kn:7}  zeit {secs:5.1}");
        let refp = f.with_extension("svg");
        if let Ok(rs) = std::fs::read_to_string(&refp) {
            let (fa, fe, kn) = measure(&rs);
            let _ = writeln!(rep, "{stem:20} Referenz-SVG     farbe {fa:5.2}  fehl {fe:5.2}  knoten {kn:7}");
        }
    }
    rep
}

fn png_b64(d: &[u8], w: usize, h: usize) -> String {
    let png = vector::encode_png(w, h, d);
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(png.len() * 4 / 3 + 4);
    for c in png.chunks(3) {
        let b = [c[0], *c.get(1).unwrap_or(&0), *c.get(2).unwrap_or(&0)];
        let v = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        s.push(T[(v >> 18) as usize & 63] as char);
        s.push(T[(v >> 12) as usize & 63] as char);
        s.push(if c.len() > 1 { T[(v >> 6) as usize & 63] as char } else { '=' });
        s.push(if c.len() > 2 { T[v as usize & 63] as char } else { '=' });
    }
    s
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut dir = PathBuf::from("pruefstand");
    let mut count = 4u64;
    let mut kinds: Vec<&'static str> = gen::KINDS.to_vec();
    let mut vt = true;
    let mut engines_only: Option<Vec<String>> = None;
    let mut refdir: Option<PathBuf> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--anzahl" => {
                i += 1;
                count = args[i].parse().unwrap_or(4);
            }
            "--nur" => {
                i += 1;
                let want: Vec<&str> = args[i].split(',').collect();
                kinds.retain(|k| want.contains(k));
            }
            "--engines" => {
                i += 1;
                engines_only = Some(args[i].split(',').map(|s| s.to_string()).collect());
            }
            "--ohne-vtracer" => vt = false,
            "--referenzen" => {
                i += 1;
                refdir = Some(PathBuf::from(&args[i]));
            }
            a => dir = PathBuf::from(a),
        }
        i += 1;
    }
    std::fs::create_dir_all(dir.join("eingaben")).unwrap();
    std::fs::create_dir_all(dir.join("ergebnisse")).unwrap();
    let db = fontdb();
    let n = N as usize;
    let mut engines: Vec<&str> = vec!["kern", "hand", "druck04", "druck02"];
    if vt {
        engines.push("vtracer");
    }
    if let Some(e) = &engines_only {
        engines.retain(|x| e.iter().any(|y| y == x));
    }
    let mut csv = String::from("bild;art;eingabe;engine;farbfehler_%;fehlpixel_%;kante_px;knoten;sekunden;zu_schmal_%\n");
    let mut html = String::from("<!DOCTYPE html><html lang=de><meta charset=utf-8><title>Prüfstand</title><style>body{font:13px system-ui;background:#26282c;color:#e6e7ea;margin:20px}table{border-collapse:collapse}td,th{padding:4px 8px;border-bottom:1px solid #46484e;text-align:right}th{color:#a2a5ac}td:first-child,th:first-child{text-align:left}img{width:140px;height:140px;background:#fff;border-radius:6px}.b{color:#4ade80}.row{display:flex;gap:8px;align-items:flex-start;margin:10px 0}.c{text-align:center;font-size:11px;color:#a2a5ac}</style><h1>Prüfstand ZeichneMichNach</h1>");
    let mut sums: std::collections::BTreeMap<(String, String), (Score, usize)> = Default::default();
    let mut rows = String::new();
    for kind in &kinds {
        for seed in 1..=count {
            let case = gen::make(kind, seed);
            std::fs::write(dir.join("eingaben").join(format!("{}.svg", case.name)), &case.svg).unwrap();
            let truth_rgba = match render(&case.svg, N, N, &db) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("{}: {e}", case.name);
                    continue;
                }
            };
            let truth = on_white(&truth_rgba);
            let te = edges(&truth, n, n);
            let td = dist(&te, n, n);
            let isvg = case.input.clone().unwrap_or_else(|| case.svg.clone());
            let full = render(&isvg, N, N, &db).unwrap();
            let small = render(&isvg, 160, 160, &db).unwrap();
            let inputs: Vec<(&str, Vec<u8>, usize)> = vec![("sauber", full.clone(), n), ("klein", small, 160), ("jpeg", degrade_jpeg(&full, n, n, seed), n)];
            for (iname, rgba, sz) in &inputs {
                let ip = dir.join("eingaben").join(format!("{}-{iname}.png", case.name));
                std::fs::write(&ip, vector::encode_png(*sz, *sz, rgba)).unwrap();
                let _ = write!(rows, "<div class=row><div class=c><img src=\"data:image/png;base64,{}\"><br>{} · {iname}</div>", png_b64(rgba, *sz, *sz), case.name);
                for e in &engines {
                    // Hintergrund nur bei den Freisteller-Bildern entfernen – sonst hat das Original ihn auch
                    let base = HandOpts { bg_cut: case.input.is_some(), ..HandOpts::default() };
                    let druck = |nz: f64| HandOpts { print: true, print_width_mm: 50.0, print_min_mm: 2.0 * nz, print_round_mm: nz / 2.0, print_colors: 6, ..base.clone() };
                    let r = match *e {
                        "kern" => run_kern(rgba, *sz, *sz),
                        "hand" => run_hand(rgba, *sz, *sz, &ip, base.clone()),
                        "druck04" => run_hand(rgba, *sz, *sz, &ip, druck(0.4)),
                        "druck02" => run_hand(rgba, *sz, *sz, &ip, druck(0.2)),
                        _ => run_vtracer(rgba, *sz, *sz),
                    };
                    std::fs::write(dir.join("ergebnisse").join(format!("{}-{iname}-{e}.svg", case.name)), &r.svg).unwrap();
                    let (s, img) = score(&truth, &td, &te, &r.svg, &db, r.secs);
                    let _ = writeln!(csv, "{};{};{iname};{e};{:.3};{:.3};{:.3};{};{:.3};{:.3}", case.name, kind, s.farbe, s.fehl, s.kante, s.knoten, s.secs, s.schmal);
                    let _ = write!(rows, "<div class=c><img src=\"data:image/png;base64,{}\"><br>{e}: {:.2} % · {:.2} px · {} Kn. · schmal {:.1} %</div>", png_b64(&img, n, n), s.fehl, s.kante, s.knoten, s.schmal);
                    for key in [(kind.to_string(), e.to_string()), (format!("~{iname}"), e.to_string()), ("Gesamt".to_string(), e.to_string())] {
                        let a = sums.entry(key).or_default();
                        a.0.farbe += s.farbe;
                        a.0.fehl += s.fehl;
                        a.0.kante += s.kante;
                        a.0.knoten += s.knoten;
                        a.0.secs += s.secs;
                        a.0.schmal += s.schmal;
                        a.1 += 1;
                    }
                    eprintln!("{} {iname:6} {e:8} fehl {:6.2} %  kante {:5.2} px  {:5} Knoten  {:5.2} s  schmal {:5.2} %", case.name, s.fehl, s.kante, s.knoten, s.secs, s.schmal);
                }
                rows += "</div>";
            }
        }
    }
    html += "<h2>Mittelwerte (kleiner ist besser)</h2><table><tr><th>Gruppe</th><th>Engine</th><th>Farbfehler %</th><th>Fehlpixel %</th><th>Kante px</th><th>Knoten</th><th>Sekunden</th><th>zu schmal % (0,4)</th></tr>";
    let mut summary = String::new();
    for ((g, e), (s, c)) in &sums {
        let c = *c as f64;
        let _ = write!(html, "<tr><td>{}</td><td>{e}</td><td>{:.2}</td><td>{:.2}</td><td>{:.2}</td><td>{:.0}</td><td>{:.2}</td><td>{:.2}</td></tr>", g.trim_start_matches('~'), s.farbe / c, s.fehl / c, s.kante / c, s.knoten as f64 / c, s.secs / c, s.schmal / c);
        let _ = writeln!(summary, "{:11} {:8} farbe {:5.2}  fehl {:5.2}  kante {:5.2}  knoten {:6.0}  zeit {:5.2}  schmal {:5.2}", g.trim_start_matches('~'), e, s.farbe / c, s.fehl / c, s.kante / c, s.knoten as f64 / c, s.secs / c, s.schmal / c);
    }
    html += "</table><h2>Einzelbilder</h2>";
    html += &rows;
    std::fs::write(dir.join("werte.csv"), csv).unwrap();
    if let Some(rd) = refdir.or_else(|| Some(dir.join("referenzen")).filter(|p| p.is_dir())) {
        let rep = referenzen(&rd, &dir.join("ergebnisse"), &db);
        if !rep.is_empty() {
            summary += "\nEigene Referenzbilder:\n";
            summary += &rep;
            html = html.replacen("<h2>Einzelbilder</h2>", &format!("<h2>Eigene Referenzbilder</h2><pre>{}</pre><h2>Einzelbilder</h2>", rep), 1);
        }
    }
    std::fs::write(dir.join("bericht.html"), html).unwrap();
    std::fs::write(dir.join("zusammenfassung.txt"), &summary).unwrap();
    println!("{summary}");
    println!("-> {}", dir.join("bericht.html").display());
}
