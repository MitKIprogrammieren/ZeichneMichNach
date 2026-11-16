//! zmn-ki – Trainingsdaten für die Kanten-KI.
//!
//!   zmn-ki daten <ordner> [--anzahl 4000] [--groesse 192] [--start 0]
//!
//! Jedes Beispiel ist eine zufällige Cartoon-/Logo-Szene als SVG in zwei Fassungen:
//! – Eingabe: mit weicher Schattierung, Glanzlichtern, Schatten, Leuchten, Vignette,
//!   dazu Unschärfe, Verkleinerung, Rauschen und JPEG – so wie echte Bilder aussehen
//! – Lösung: dieselben Formen flach gefüllt, nur harte Kanten (Umrisse, Streifen,
//!   Zelltöne, Schrift, Linien)
//! Gespeichert werden ein Ausschnitt der Eingabe (`NNNNN_ein.png`) und die echten
//! Kanten der Lösung (`NNNNN_kante.png`, weiß = echte Kante).

use std::f64::consts::TAU;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::Arc;

struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407) | 1)
    }
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn f(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn r(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.f()
    }
    fn i(&mut self, a: usize, b: usize) -> usize {
        a + (self.next() % (b - a + 1) as u64) as usize
    }
    fn p(&mut self, prob: f64) -> bool {
        self.f() < prob
    }
}

fn hsl(h: f64, s: f64, l: f64) -> [u8; 3] {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = (h.rem_euclid(360.0)) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    [((r + m) * 255.0) as u8, ((g + m) * 255.0) as u8, ((b + m) * 255.0) as u8]
}

fn hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

/// Logo-/Cartoon-typische Farbe (satt, Gold, Rot, Schwarz, Weiß …)
fn farbe(r: &mut Rng) -> (f64, f64, f64) {
    match r.i(0, 9) {
        0 => (0.0, 0.0, r.r(0.02, 0.12)),        // Schwarz
        1 => (0.0, 0.0, r.r(0.88, 0.98)),        // Weiß
        2 => (r.r(38.0, 50.0), r.r(0.7, 1.0), r.r(0.45, 0.6)), // Gold
        3 => (r.r(-8.0, 8.0), r.r(0.7, 0.95), r.r(0.4, 0.55)),  // Rot
        _ => (r.r(0.0, 360.0), r.r(0.35, 1.0), r.r(0.2, 0.75)),
    }
}

fn shape_path(r: &mut Rng) -> String {
    let (cx, cy) = (r.r(40.0, 360.0), r.r(40.0, 360.0));
    let s = r.r(30.0, 170.0);
    match r.i(0, 5) {
        0 => {
            // Ellipse als Pfad
            let (rx, ry) = (s, s * r.r(0.5, 1.0));
            format!("M{:.1},{:.1} a{rx:.1},{ry:.1} 0 1,0 {:.1},0 a{rx:.1},{ry:.1} 0 1,0 {:.1},0 Z", cx - rx, cy, 2.0 * rx, -2.0 * rx)
        }
        1 => {
            // Stern
            let n = r.i(4, 8);
            let (r1, r2) = (s, s * r.r(0.35, 0.6));
            let rot = r.r(0.0, TAU);
            let mut d = String::new();
            for i in 0..n * 2 {
                let a = rot + TAU * i as f64 / (n * 2) as f64;
                let rr = if i % 2 == 0 { r1 } else { r2 };
                let _ = write!(d, "{}{:.1},{:.1} ", if i == 0 { "M" } else { "L" }, cx + rr * a.sin(), cy - rr * a.cos());
            }
            d + "Z"
        }
        2 => {
            // Vieleck (Kristall, Schild)
            let n = r.i(3, 7);
            let rot = r.r(0.0, TAU);
            let mut d = String::new();
            for i in 0..n {
                let a = rot + TAU * i as f64 / n as f64 + r.r(-0.25, 0.25);
                let rr = s * r.r(0.6, 1.1);
                let _ = write!(d, "{}{:.1},{:.1} ", if i == 0 { "M" } else { "L" }, cx + rr * a.cos(), cy + rr * a.sin());
            }
            d + "Z"
        }
        3 => {
            // abgerundetes Rechteck, gedreht
            let (w, h) = (s * r.r(0.6, 1.6), s * r.r(0.3, 1.0));
            let rad = r.r(0.0, h.min(w) * 0.45);
            let a = r.r(-0.6, 0.6);
            let (ca, sa) = (a.cos(), a.sin());
            let p = |x: f64, y: f64| (cx + x * ca - y * sa, cy + x * sa + y * ca);
            let (x0, y0, x1, y1) = (-w / 2.0, -h / 2.0, w / 2.0, h / 2.0);
            let pts = [
                p(x0 + rad, y0),
                p(x1 - rad, y0),
                p(x1, y0),
                p(x1, y0 + rad),
                p(x1, y1 - rad),
                p(x1, y1),
                p(x1 - rad, y1),
                p(x0 + rad, y1),
                p(x0, y1),
                p(x0, y1 - rad),
                p(x0, y0 + rad),
                p(x0, y0),
            ];
            format!(
                "M{:.1},{:.1} L{:.1},{:.1} Q{:.1},{:.1} {:.1},{:.1} L{:.1},{:.1} Q{:.1},{:.1} {:.1},{:.1} L{:.1},{:.1} Q{:.1},{:.1} {:.1},{:.1} L{:.1},{:.1} Q{:.1},{:.1} {:.1},{:.1} Z",
                pts[0].0, pts[0].1, pts[1].0, pts[1].1, pts[2].0, pts[2].1, pts[3].0, pts[3].1, pts[4].0, pts[4].1, pts[5].0, pts[5].1, pts[6].0, pts[6].1, pts[7].0, pts[7].1,
                pts[8].0, pts[8].1, pts[9].0, pts[9].1, pts[10].0, pts[10].1, pts[11].0, pts[11].1, pts[0].0, pts[0].1
            )
        }
        _ => {
            // weicher Klecks (Figur, Wolke, Hand)
            let n = r.i(5, 9);
            let pts: Vec<(f64, f64)> = (0..n)
                .map(|i| {
                    let a = TAU * i as f64 / n as f64;
                    let rr = s * r.r(0.6, 1.15);
                    (cx + rr * a.cos(), cy + rr * a.sin())
                })
                .collect();
            let mut d = format!("M{:.1},{:.1} ", pts[0].0, pts[0].1);
            for i in 0..n {
                let p0 = pts[(i + n - 1) % n];
                let p1 = pts[i];
                let p2 = pts[(i + 1) % n];
                let p3 = pts[(i + 2) % n];
                let _ = write!(
                    d,
                    "C{:.1},{:.1} {:.1},{:.1} {:.1},{:.1} ",
                    p1.0 + (p2.0 - p0.0) / 6.0,
                    p1.1 + (p2.1 - p0.1) / 6.0,
                    p2.0 - (p3.0 - p1.0) / 6.0,
                    p2.1 - (p3.1 - p1.1) / 6.0,
                    p2.0,
                    p2.1
                );
            }
            d + "Z"
        }
    }
}

const WORTE: &[&str] = &["AWARD", "KICK", "TEAM", "GOLD", "2026", "BANDE", "LIGA", "WIN", "CUP", "STAR"];
const SCHRIFTEN: &[&str] = &["DejaVu Sans", "DejaVu Serif", "Liberation Sans", "Poppins", "Open Sans", "Lato", "Roboto"];

/// Eine Szene: (Eingabe-SVG, Lösungs-SVG).
fn szene(seed: u64) -> (String, String) {
    let mut r = Rng::new(seed);
    let mut ein = String::new();
    let mut los = String::new();
    let mut defs = String::new();
    let mut id = 0usize;
    let mut nid = |p: &str| {
        id += 1;
        format!("{p}{id}")
    };
    // Hintergrund: Lösung flach, Eingabe mit Vignette oder Verlauf
    let (bh, bs, bl) = farbe(&mut r);
    let bg = hsl(bh, bs, bl);
    let _ = write!(los, "<rect width=\"400\" height=\"400\" fill=\"{}\"/>", hex(bg));
    if r.p(0.7) {
        let g = nid("g");
        let dark = hsl(bh, bs, (bl * r.r(0.4, 0.85)).max(0.0));
        let _ = write!(defs, "<radialGradient id=\"{g}\" cx=\"{:.2}\" cy=\"{:.2}\" r=\"{:.2}\"><stop offset=\"0\" stop-color=\"{}\"/><stop offset=\"1\" stop-color=\"{}\"/></radialGradient>", r.r(0.3, 0.7), r.r(0.3, 0.7), r.r(0.5, 0.9), hex(bg), hex(dark));
        let _ = write!(ein, "<rect width=\"400\" height=\"400\" fill=\"url(#{g})\"/>");
    } else {
        let _ = write!(ein, "<rect width=\"400\" height=\"400\" fill=\"{}\"/>", hex(bg));
    }
    for _ in 0..r.i(3, 9) {
        let d = shape_path(&mut r);
        let (h, s, l) = farbe(&mut r);
        let c = hsl(h, s, l);
        let clip = nid("c");
        let _ = write!(defs, "<clipPath id=\"{clip}\"><path d=\"{d}\"/></clipPath>");
        // Leuchten hinter der Form (nur Eingabe, keine Kante)
        if r.p(0.25) {
            let f = nid("f");
            let _ = write!(defs, "<filter id=\"{f}\" x=\"-50%\" y=\"-50%\" width=\"200%\" height=\"200%\"><feGaussianBlur stdDeviation=\"{:.1}\"/></filter>", r.r(4.0, 14.0));
            let glow = hsl(h + r.r(-20.0, 20.0), 1.0, r.r(0.5, 0.75));
            let _ = write!(ein, "<path d=\"{d}\" fill=\"{}\" opacity=\"{:.2}\" filter=\"url(#{f})\"/>", hex(glow), r.r(0.4, 0.9));
        }
        // Füllung: Lösung flach, Eingabe weich schattiert
        let _ = write!(los, "<path d=\"{d}\" fill=\"{}\"/>", hex(c));
        if r.p(0.75) {
            let g = nid("g");
            let hell = hsl(h, s, (l + r.r(0.08, 0.3)).min(0.97));
            let dunkel = hsl(h, s, (l - r.r(0.08, 0.3)).max(0.02));
            if r.p(0.5) {
                let _ = write!(defs, "<linearGradient id=\"{g}\" x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\"><stop offset=\"0\" stop-color=\"{}\"/><stop offset=\"{:.2}\" stop-color=\"{}\"/><stop offset=\"1\" stop-color=\"{}\"/></linearGradient>", r.f(), r.f(), r.f(), r.f(), hex(hell), r.r(0.3, 0.7), hex(c), hex(dunkel));
            } else {
                let _ = write!(defs, "<radialGradient id=\"{g}\" cx=\"{:.2}\" cy=\"{:.2}\" r=\"{:.2}\"><stop offset=\"0\" stop-color=\"{}\"/><stop offset=\"1\" stop-color=\"{}\"/></radialGradient>", r.r(0.2, 0.6), r.r(0.2, 0.6), r.r(0.5, 1.0), hex(hell), hex(dunkel));
            }
            let _ = write!(ein, "<path d=\"{d}\" fill=\"url(#{g})\"/>");
        } else {
            let _ = write!(ein, "<path d=\"{d}\" fill=\"{}\"/>", hex(c));
        }
        // harte Streifen in der Form (echte Kanten, in beiden)
        if r.p(0.3) {
            let (h2, s2, l2) = farbe(&mut r);
            let c2 = hex(hsl(h2, s2, l2));
            let (w, gap, a) = (r.r(8.0, 30.0), r.r(8.0, 40.0), r.r(-60.0, 60.0));
            let mut st = String::new();
            let mut x = -400.0;
            while x < 800.0 {
                let _ = write!(st, "<rect x=\"{x:.1}\" y=\"-400\" width=\"{w:.1}\" height=\"1200\"/>");
                x += w + gap;
            }
            let g = format!("<g clip-path=\"url(#{clip})\"><g fill=\"{c2}\" transform=\"rotate({a:.1} 200 200)\">{st}</g></g>");
            ein += &g;
            los += &g;
        }
        // Zellschattierung: harte dunklere Hälfte (echte Kante)
        if r.p(0.3) {
            let dunkel = hex(hsl(h, s, (l - r.r(0.12, 0.3)).max(0.02)));
            let off = (r.r(-40.0, 40.0), r.r(10.0, 50.0));
            let g = format!("<g clip-path=\"url(#{clip})\"><path d=\"{d}\" fill=\"{dunkel}\" transform=\"translate({:.1} {:.1})\"/></g>", off.0, off.1);
            ein += &g;
            los += &g;
        }
        // weicher Schatten innen (nur Eingabe)
        if r.p(0.35) {
            let f = nid("f");
            let _ = write!(defs, "<filter id=\"{f}\" x=\"-50%\" y=\"-50%\" width=\"200%\" height=\"200%\"><feGaussianBlur stdDeviation=\"{:.1}\"/></filter>", r.r(5.0, 18.0));
            let _ = write!(ein, "<g clip-path=\"url(#{clip})\"><path d=\"{d}\" fill=\"#000\" opacity=\"{:.2}\" transform=\"translate({:.1} {:.1})\" filter=\"url(#{f})\"/></g>", r.r(0.2, 0.5), r.r(-20.0, 20.0), r.r(10.0, 40.0));
        }
        // Glanzlicht (nur Eingabe)
        if r.p(0.45) {
            let f = nid("f");
            let _ = write!(defs, "<filter id=\"{f}\" x=\"-50%\" y=\"-50%\" width=\"200%\" height=\"200%\"><feGaussianBlur stdDeviation=\"{:.1}\"/></filter>", r.r(2.0, 10.0));
            let _ = write!(
                ein,
                "<g clip-path=\"url(#{clip})\"><ellipse cx=\"{:.1}\" cy=\"{:.1}\" rx=\"{:.1}\" ry=\"{:.1}\" fill=\"#fff\" opacity=\"{:.2}\" filter=\"url(#{f})\"/></g>",
                r.r(40.0, 360.0),
                r.r(40.0, 360.0),
                r.r(8.0, 60.0),
                r.r(4.0, 30.0),
                r.r(0.3, 0.85)
            );
        }
        // Umriss (echte Kante, in beiden)
        if r.p(0.45) {
            let (h3, s3, l3) = if r.p(0.6) { (0.0, 0.0, r.r(0.0, 0.12)) } else { farbe(&mut r) };
            let k = format!("<path d=\"{d}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{:.1}\" stroke-linejoin=\"round\"/>", hex(hsl(h3, s3, l3)), r.r(1.5, 9.0));
            ein += &k;
            los += &k;
        }
    }
    // feine harte Linien
    for _ in 0..r.i(0, 3) {
        let (h, s, l) = farbe(&mut r);
        let k = format!(
            "<path d=\"M{:.1},{:.1} Q{:.1},{:.1} {:.1},{:.1}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{:.1}\" stroke-linecap=\"round\"/>",
            r.r(0.0, 400.0),
            r.r(0.0, 400.0),
            r.r(0.0, 400.0),
            r.r(0.0, 400.0),
            r.r(0.0, 400.0),
            r.r(0.0, 400.0),
            hex(hsl(h, s, l)),
            r.r(1.0, 4.0)
        );
        ein += &k;
        los += &k;
    }
    // Schrift
    if r.p(0.35) {
        let (h, s, l) = farbe(&mut r);
        let w = WORTE[r.i(0, WORTE.len() - 1)];
        let f = SCHRIFTEN[r.i(0, SCHRIFTEN.len() - 1)];
        let k = format!(
            "<text x=\"{:.1}\" y=\"{:.1}\" font-family=\"{f}\" font-weight=\"{}\" font-size=\"{:.1}\" fill=\"{}\">{w}</text>",
            r.r(0.0, 200.0),
            r.r(60.0, 380.0),
            if r.p(0.6) { "bold" } else { "normal" },
            r.r(18.0, 80.0),
            hex(hsl(h, s, l))
        );
        ein += &k;
        los += &k;
    }
    let wrap = |b: &str| format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"400\" height=\"400\" viewBox=\"0 0 400 400\"><defs>{defs}</defs>{b}</svg>");
    (wrap(&ein), wrap(&los))
}

fn render(svg: &str, n: u32, db: &Arc<resvg::usvg::fontdb::Database>) -> Option<Vec<u8>> {
    let mut opt = resvg::usvg::Options::default();
    opt.fontdb = db.clone();
    let tree = resvg::usvg::Tree::from_str(svg, &opt).ok()?;
    let mut pm = resvg::tiny_skia::Pixmap::new(n, n)?;
    let s = tree.size();
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(n as f32 / s.width(), n as f32 / s.height()), &mut pm.as_mut());
    // Hintergrund ist immer deckend – Vormultiplikation spielt keine Rolle
    Some(pm.take().chunks(4).flat_map(|p| [p[0], p[1], p[2]]).collect())
}

fn blur(d: &mut [u8], w: usize, h: usize, sigma: f64) {
    if sigma < 0.2 {
        return;
    }
    let rad = (sigma * 2.5).ceil() as i64;
    let k: Vec<f64> = (-rad..=rad).map(|i| (-(i * i) as f64 / (2.0 * sigma * sigma)).exp()).collect();
    let ks: f64 = k.iter().sum();
    let src: Vec<f64> = d.iter().map(|&v| v as f64).collect();
    let mut t = vec![0f64; d.len()];
    for y in 0..h {
        for x in 0..w {
            for c in 0..3 {
                let mut s = 0.0;
                for (i, kv) in k.iter().enumerate() {
                    let xx = (x as i64 + i as i64 - rad).clamp(0, w as i64 - 1) as usize;
                    s += kv * src[(y * w + xx) * 3 + c];
                }
                t[(y * w + x) * 3 + c] = s / ks;
            }
        }
    }
    for y in 0..h {
        for x in 0..w {
            for c in 0..3 {
                let mut s = 0.0;
                for (i, kv) in k.iter().enumerate() {
                    let yy = (y as i64 + i as i64 - rad).clamp(0, h as i64 - 1) as usize;
                    s += kv * t[(yy * w + x) * 3 + c];
                }
                d[(y * w + x) * 3 + c] = (s / ks).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(|s| s.as_str()) != Some("daten") || args.len() < 2 {
        eprintln!("Aufruf: zmn-ki daten <ordner> [--anzahl 4000] [--groesse 192] [--start 0]");
        std::process::exit(2);
    }
    let dir = PathBuf::from(&args[1]);
    let mut anzahl = 4000u64;
    let mut gr = 192usize;
    let mut start = 0u64;
    let mut i = 2;
    while i + 1 < args.len() {
        match args[i].as_str() {
            "--anzahl" => anzahl = args[i + 1].parse().unwrap_or(anzahl),
            "--groesse" => gr = args[i + 1].parse().unwrap_or(gr),
            "--start" => start = args[i + 1].parse().unwrap_or(start),
            _ => {}
        }
        i += 2;
    }
    std::fs::create_dir_all(&dir).expect("Ordner anlegen");
    let mut db = resvg::usvg::fontdb::Database::new();
    db.load_system_fonts();
    let db = Arc::new(db);
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let next = Arc::new(std::sync::atomic::AtomicU64::new(start));
    let ende = start + anzahl;
    std::thread::scope(|sc| {
        for _ in 0..threads {
            let (next, db, dir) = (next.clone(), db.clone(), dir.clone());
            sc.spawn(move || loop {
                let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if k >= ende {
                    break;
                }
                let mut r = Rng::new(k * 7919 + 13);
                let (ein, los) = szene(k);
                // Größe der ganzen Szene, daraus ein Ausschnitt
                let n = r.i(gr, gr * 3) as u32;
                let (Some(mut a), Some(b)) = (render(&ein, n, &db), render(&los, n, &db)) else { continue };
                let nn = n as usize;
                // Verkleinern und wieder vergrößern (kleine Vorlagen, Bildschirmfotos)
                if r.p(0.4) {
                    let f = r.r(1.3, 3.0);
                    let m = ((nn as f64 / f) as u32).max(16);
                    let im = image::RgbImage::from_raw(n, n, a).unwrap();
                    let small = image::imageops::resize(&im, m, m, image::imageops::FilterType::Triangle);
                    a = image::imageops::resize(&small, n, n, if r.p(0.5) { image::imageops::FilterType::Triangle } else { image::imageops::FilterType::CatmullRom }).into_raw();
                }
                if r.p(0.5) {
                    blur(&mut a, nn, nn, r.r(0.3, 1.4));
                }
                if r.p(0.6) {
                    let s = r.r(1.0, 9.0);
                    for v in a.iter_mut() {
                        let z = (r.f() + r.f() + r.f() - 1.5) * s;
                        *v = (*v as f64 + z).clamp(0.0, 255.0) as u8;
                    }
                }
                if r.p(0.6) {
                    let mut buf = Vec::new();
                    let q = r.i(35, 92) as u8;
                    if image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, q).encode(&a, n, n, image::ExtendedColorType::Rgb8).is_ok() {
                        if let Ok(im) = image::load_from_memory(&buf) {
                            a = im.to_rgb8().into_raw();
                        }
                    }
                }
                // echte Kanten der Lösung: Farbsprung zum Nachbarn
                let mut kante = vec![0u8; nn * nn];
                let px = |p: usize| [b[p * 3] as i32, b[p * 3 + 1] as i32, b[p * 3 + 2] as i32];
                let diff = |p: usize, q: usize| {
                    let (u, v) = (px(p), px(q));
                    (u[0] - v[0]).abs().max((u[1] - v[1]).abs()).max((u[2] - v[2]).abs())
                };
                for y in 0..nn {
                    for x in 0..nn {
                        let p = y * nn + x;
                        if (x + 1 < nn && diff(p, p + 1) > 40) || (y + 1 < nn && diff(p, p + nn) > 40) {
                            kante[p] = 255;
                        }
                    }
                }
                // Kanten, die in der Eingabe gar nicht zu sehen sind (dunkel auf dunkel durch
                // Schattierung), nicht lernen – sonst rät das Netz Kanten ins Leere
                let lum = |p: usize| 0.299 * a[p * 3] as f64 + 0.587 * a[p * 3 + 1] as f64 + 0.114 * a[p * 3 + 2] as f64;
                for y in 0..nn {
                    for x in 0..nn {
                        let p = y * nn + x;
                        if kante[p] == 0 {
                            continue;
                        }
                        let (mut lo, mut hi) = (255.0f64, 0.0f64);
                        for yy in y.saturating_sub(2)..(y + 3).min(nn) {
                            for xx in x.saturating_sub(2)..(x + 3).min(nn) {
                                let v = lum(yy * nn + xx);
                                lo = lo.min(v);
                                hi = hi.max(v);
                            }
                        }
                        if hi - lo < 14.0 {
                            kante[p] = 0;
                        }
                    }
                }
                // Ausschnitt
                let (ox, oy) = (r.i(0, nn - gr), r.i(0, nn - gr));
                let mut ca = Vec::with_capacity(gr * gr * 3);
                let mut ck = Vec::with_capacity(gr * gr);
                for y in oy..oy + gr {
                    ca.extend_from_slice(&a[(y * nn + ox) * 3..(y * nn + ox + gr) * 3]);
                    ck.extend_from_slice(&kante[y * nn + ox..y * nn + ox + gr]);
                }
                let _ = image::RgbImage::from_raw(gr as u32, gr as u32, ca).unwrap().save(dir.join(format!("{k:05}_ein.png")));
                let _ = image::GrayImage::from_raw(gr as u32, gr as u32, ck).unwrap().save(dir.join(format!("{k:05}_kante.png")));
                if k % 500 == 0 {
                    eprintln!("  {k} / {ende}");
                }
            });
        }
    });
    eprintln!("Fertig: {anzahl} Beispiele in {}", dir.display());
}
