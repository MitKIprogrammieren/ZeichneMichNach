//! Testbilder mit bekanntem Original: zufällige Logos, Wappen, Schriftzüge, Linien und Verläufe
//! als SVG. Gleicher Startwert = gleiches Bild, damit Messungen vergleichbar bleiben.

use std::f64::consts::TAU;

pub struct Rng(u64);
impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407) | 1)
    }
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    pub fn f(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    pub fn r(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.f()
    }
    pub fn i(&mut self, a: usize, b: usize) -> usize {
        a + (self.next() % (b - a + 1) as u64) as usize
    }
    pub fn pick<'a, T>(&mut self, v: &'a [T]) -> &'a T {
        &v[self.i(0, v.len() - 1)]
    }
}

/// Typische Logo-/Wappenfarben (keine reinen Zufallsfarben – so sehen echte Logos aus).
const PAL: &[&str] = &[
    "#1d3a8a", "#c9a227", "#b91c1c", "#15803d", "#0f172a", "#ffffff", "#f97316", "#7c3aed", "#0891b2", "#e11d48", "#facc15", "#374151", "#a16207", "#0ea5e9",
];

fn colors(r: &mut Rng, n: usize) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    while out.len() < n {
        let c = *r.pick(PAL);
        if !out.contains(&c) {
            out.push(c);
        }
    }
    out
}

fn star(cx: f64, cy: f64, r1: f64, r2: f64, n: usize, rot: f64) -> String {
    let mut s = String::new();
    for i in 0..n * 2 {
        let a = rot + TAU * i as f64 / (n * 2) as f64;
        let r = if i % 2 == 0 { r1 } else { r2 };
        s += &format!("{}{:.2},{:.2} ", if i == 0 { "M" } else { "L" }, cx + r * a.sin(), cy - r * a.cos());
    }
    s + "Z"
}

fn blob(r: &mut Rng, cx: f64, cy: f64, rad: f64) -> String {
    let n = r.i(5, 8);
    let pts: Vec<(f64, f64)> = (0..n)
        .map(|i| {
            let a = TAU * i as f64 / n as f64;
            let rr = rad * r.r(0.65, 1.15);
            (cx + rr * a.cos(), cy + rr * a.sin())
        })
        .collect();
    // geschlossene Catmull-Rom-Kurve als kubische Béziers
    let mut s = format!("M{:.2},{:.2} ", pts[0].0, pts[0].1);
    for i in 0..n {
        let p0 = pts[(i + n - 1) % n];
        let p1 = pts[i];
        let p2 = pts[(i + 1) % n];
        let p3 = pts[(i + 2) % n];
        let c1 = (p1.0 + (p2.0 - p0.0) / 6.0, p1.1 + (p2.1 - p0.1) / 6.0);
        let c2 = (p2.0 - (p3.0 - p1.0) / 6.0, p2.1 - (p3.1 - p1.1) / 6.0);
        s += &format!("C{:.2},{:.2} {:.2},{:.2} {:.2},{:.2} ", c1.0, c1.1, c2.0, c2.1, p2.0, p2.1);
    }
    s + "Z"
}

const SHIELD: &str = "M200 30 L340 70 L330 220 Q315 330 200 375 Q85 330 70 220 L60 70 Z";

pub struct Case {
    pub name: String,
    pub kind: &'static str,
    /// Original (Soll-Ergebnis)
    pub svg: String,
    /// Eingabe, falls anders als das Original (z. B. mit Hintergrund zum Entfernen)
    pub input: Option<String>,
}

fn wrap(body: &str, bg: Option<&str>) -> String {
    let bgr = bg.map(|c| format!("<rect width=\"400\" height=\"400\" fill=\"{c}\"/>")).unwrap_or_default();
    format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"400\" height=\"400\" viewBox=\"0 0 400 400\">{bgr}{body}</svg>")
}

const WORDS: &[&str] = &["KADER", "SCHICHT", "WERK", "NORD", "FC 1987", "ADLER", "MAKER", "LÖWE", "TEAM", "BERG"];
const FONTS: &[&str] = &["DejaVu Sans", "DejaVu Serif", "Liberation Sans", "Poppins"];

pub fn make(kind: &'static str, seed: u64) -> Case {
    let mut r = Rng::new(seed ^ (kind.len() as u64 * 7919));
    if kind == "freisteller" {
        // Motiv ohne Hintergrund; Eingabe mit Verlauf und Vignette dahinter
        let motiv = make(*r.pick(&["flach", "wappen", "text"]), seed + 100);
        let body = motiv.svg.replacen("<rect width=\"400\" height=\"400\" fill=", "<rect width=\"0\" height=\"0\" fill=", 1);
        let (a, e) = *r.pick(&[("#ffffff", "#d9dde3"), ("#f5f0e6", "#c9bfa8"), ("#e8f1fb", "#9fb8d6"), ("#ffffff", "#ffffff")]);
        let grad = format!(
            "<defs><radialGradient id=\"bgv\" cx=\"0.5\" cy=\"0.45\" r=\"0.75\"><stop offset=\"0\" stop-color=\"{a}\"/><stop offset=\"1\" stop-color=\"{e}\"/></radialGradient></defs><rect width=\"400\" height=\"400\" fill=\"url(#bgv)\"/>"
        );
        let inner = body.split_once('>').map(|x| x.1).unwrap_or("").trim_end_matches("</svg>").to_string();
        // Motiv etwas kleiner, damit ringsum Hintergrund bleibt
        let scaled = format!("<g transform=\"translate(40 40) scale(0.8)\">{inner}</g>");
        return Case { name: format!("{kind}-{seed:02}"), kind, svg: wrap(&scaled, None), input: Some(wrap(&format!("{grad}{scaled}"), None)) };
    }
    let svg = match kind {
        // flache Formen, 2–5 Farben
        "flach" => {
            let nc = r.i(3, 5);
            let c = colors(&mut r, nc);
            let mut b = String::new();
            for i in 1..c.len() {
                match r.i(0, 3) {
                    0 => b += &format!("<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"{:.1}\" fill=\"{}\"/>", r.r(120.0, 280.0), r.r(120.0, 280.0), r.r(40.0, 110.0), c[i]),
                    1 => b += &format!("<path d=\"{}\" fill=\"{}\"/>", star(r.r(150.0, 250.0), r.r(150.0, 250.0), r.r(70.0, 130.0), r.r(30.0, 60.0), r.i(4, 7), r.r(0.0, 1.0)), c[i]),
                    2 => b += &format!(
                        "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" rx=\"{:.1}\" transform=\"rotate({:.1} 200 200)\" fill=\"{}\"/>",
                        r.r(60.0, 160.0),
                        r.r(60.0, 160.0),
                        r.r(100.0, 200.0),
                        r.r(60.0, 180.0),
                        r.r(0.0, 30.0),
                        r.r(-30.0, 30.0),
                        c[i]
                    ),
                    _ => {
                        let (x, y, rr) = (r.r(150.0, 250.0), r.r(150.0, 250.0), r.r(60.0, 120.0));
                        b += &format!("<path d=\"{}\" fill=\"{}\"/>", blob(&mut r, x, y, rr), c[i])
                    }
                }
            }
            wrap(&b, Some(c[0]))
        }
        // Wappen: Schild mit Rand, Teilung, Figur
        "wappen" => {
            let c = colors(&mut r, 4);
            let sw = r.r(6.0, 16.0);
            let mut b = format!("<path d=\"{SHIELD}\" fill=\"{}\"/>", c[0]);
            b += &format!("<clipPath id=\"s\"><path d=\"{SHIELD}\"/></clipPath><g clip-path=\"url(#s)\">");
            match r.i(0, 2) {
                0 => b += &format!("<rect x=\"200\" y=\"0\" width=\"200\" height=\"400\" fill=\"{}\"/>", c[1]),
                1 => b += &format!("<path d=\"M0 0 L400 400 L400 0 Z\" fill=\"{}\"/>", c[1]),
                _ => b += &format!("<rect x=\"0\" y=\"170\" width=\"400\" height=\"{:.1}\" fill=\"{}\"/>", r.r(40.0, 90.0), c[1]),
            }
            b += "</g>";
            b += &format!("<path d=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"4\"/>", star(200.0, 200.0, r.r(55.0, 80.0), r.r(22.0, 35.0), r.i(5, 6), 0.0), c[2], c[3]);
            b += &format!("<path d=\"{SHIELD}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{sw:.1}\" stroke-linejoin=\"round\"/>", c[3]);
            wrap(&b, None)
        }
        // Schriftzug mit Form dahinter
        "text" => {
            let c = colors(&mut r, 3);
            let w = r.pick(WORDS);
            let f = r.pick(FONTS);
            let sz = (300.0 / w.chars().count() as f64 * 1.5).min(90.0);
            let mut b = format!("<circle cx=\"200\" cy=\"200\" r=\"{:.1}\" fill=\"{}\"/>", r.r(150.0, 185.0), c[1]);
            b += &format!(
                "<text x=\"200\" y=\"{:.1}\" font-family=\"{f}\" font-weight=\"{}\" font-size=\"{sz:.1}\" text-anchor=\"middle\" fill=\"{}\">{w}</text>",
                200.0 + sz * 0.35,
                if r.f() < 0.5 { "bold" } else { "normal" },
                c[2]
            );
            wrap(&b, Some(c[0]))
        }
        // feine Linien, Ringe, Kreuzungen
        "linien" => {
            let c = colors(&mut r, 2);
            let lw = r.r(2.0, 6.0);
            let mut b = format!("<g fill=\"none\" stroke=\"{}\" stroke-width=\"{lw:.1}\" stroke-linecap=\"round\">", c[1]);
            b += &format!("<circle cx=\"200\" cy=\"200\" r=\"{:.1}\"/>", r.r(130.0, 170.0));
            b += &format!("<circle cx=\"200\" cy=\"200\" r=\"{:.1}\"/>", r.r(60.0, 100.0));
            for _ in 0..r.i(3, 6) {
                b += &format!("<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\"/>", r.r(30.0, 370.0), r.r(30.0, 370.0), r.r(30.0, 370.0), r.r(30.0, 370.0));
            }
            b += &format!("<path d=\"M60 {:.1} Q200 {:.1} 340 {:.1}\"/>", r.r(250.0, 330.0), r.r(150.0, 380.0), r.r(250.0, 330.0));
            b += "</g>";
            wrap(&b, Some(c[0]))
        }
        // Verlauf (Gold/Metall) in einer Form
        _ => {
            let c = colors(&mut r, 2);
            let (a, e) = *r.pick(&[("#fde68a", "#a16207"), ("#e5e7eb", "#4b5563"), ("#93c5fd", "#1e3a8a"), ("#fca5a5", "#7f1d1d")]);
            let mut b = format!("<defs><linearGradient id=\"g\" x1=\"0\" y1=\"0\" x2=\"{:.2}\" y2=\"1\"><stop offset=\"0\" stop-color=\"{a}\"/><stop offset=\"1\" stop-color=\"{e}\"/></linearGradient></defs>", r.r(0.0, 1.0));
            b += &format!("<path d=\"{}\" fill=\"url(#g)\"/>", star(200.0, 200.0, r.r(140.0, 175.0), r.r(80.0, 120.0), r.i(5, 8), r.r(0.0, 1.0)));
            b += &format!("<circle cx=\"200\" cy=\"200\" r=\"{:.1}\" fill=\"{}\"/>", r.r(35.0, 60.0), c[1]);
            wrap(&b, Some(c[0]))
        }
    };
    Case { name: format!("{kind}-{seed:02}"), kind, svg, input: None }
}

pub const KINDS: &[&str] = &["flach", "wappen", "text", "linien", "verlauf", "freisteller"];
