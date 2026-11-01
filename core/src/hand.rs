//! „Nachzeichnen wie von Hand“: versteht ein Bild ein Stück weit, bevor es
//! nachgezeichnet wird – Hintergrund außen freistellen, feine Linien als Striche,
//! Fotobereiche beruhigen, Text durch echte Schrift ersetzen und Farbverläufe
//! erkennen. Alles lokal, ohne Internet.
//!
//! Ablauf: [`prepare`] (Analysebild, Freistellen, Linien, Fotobereiche) →
//! [`analyze_text`] je Textzeile (die App sucht dazu passende Schriften) →
//! [`finish`] (Farben, Pfade, Text, Verläufe).

use crate::vector::{self, fill_evenodd, flatten, Cmd, Doc, Layer, Opts, Pt, Shape};

/// Schalter des Bereichs „Nachzeichnen wie von Hand“.
#[derive(Clone, Debug, PartialEq)]
pub struct HandOpts {
    pub bg_cut: bool,
    /// Farbabstand für das Freistellen (0…100)
    pub bg_tol: f64,
    pub lines: bool,
    pub photo: bool,
    pub gradients: bool,
    pub text: bool,
    /// Für den 3D-Druck: nur flache Farben, Mindestbreite, keine Spitzen
    pub print: bool,
    /// Druckbreite des ganzen Bildes in mm
    pub print_width_mm: f64,
    /// schmalste erlaubte Stelle in mm (z. B. 2 Linienbreiten)
    pub print_min_mm: f64,
    /// höchstens so viele Farben (AMS)
    pub print_colors: usize,
}

impl Default for HandOpts {
    fn default() -> Self {
        HandOpts {
            bg_cut: true,
            bg_tol: 28.0,
            lines: true,
            photo: true,
            gradients: true,
            text: true,
            print: false,
            print_width_mm: 100.0,
            print_min_mm: 0.8,
            print_colors: 4,
        }
    }
}

/// Farbverlauf einer Teilform (Koordinaten im Quellbild).
#[derive(Clone, Debug)]
pub struct Grad {
    pub layer: usize,
    pub sub: usize,
    pub p1: Pt,
    pub p2: Pt,
    pub stops: Vec<(f64, [u8; 3])>,
}

/// Erkannte feine Linie (Koordinaten im Quellbild).
#[derive(Clone, Debug)]
pub struct LineOut {
    pub pts: Vec<Pt>,
    pub closed: bool,
    pub width: f64,
    pub color: [u8; 3],
}

/// Ergebnis der Zusatzschritte, wird an [`Doc`] gehängt.
#[derive(Clone, Debug, Default)]
pub struct HandResult {
    pub grads: Vec<Grad>,
    pub lines: Vec<LineOut>,
    pub texts: usize,
    /// Anteil der Fotobereiche am Bild (0…1)
    pub photo: f64,
    /// Analysebild vor dem Übermalen von Text und Linien (für Verläufe)
    pub orig: Vec<u8>,
    /// Pixel, die zu Text oder Linien gehören und übermalt werden
    pub erase: Vec<bool>,
    /// erkannte Textzeilen und eingesetzte Schrift (für Neuberechnungen)
    pub ocr: Vec<TextLine>,
    pub saved: Vec<TextOut>,
    /// tatsächlich verwendete Farbanzahl
    pub k: usize,
    /// für den 3D-Druck aufbereitet (keine Spaltstriche, nur Flächen)
    pub print: bool,
}

// ---------------------------------------------------------------------------
// kleine Helfer

fn lum(d: &[u8], p: usize) -> f64 {
    let j = p * 4;
    0.299 * d[j] as f64 + 0.587 * d[j + 1] as f64 + 0.114 * d[j + 2] as f64
}

fn cdist(a: [f64; 3], b: [f64; 3]) -> f64 {
    let (dr, dg, db) = (a[0] - b[0], a[1] - b[1], a[2] - b[2]);
    let rm = (a[0] + b[0]) / 2.0;
    ((2.0 + rm / 256.0) * dr * dr + 4.0 * dg * dg + (2.0 + (255.0 - rm) / 256.0) * db * db).sqrt() / 3.0
}

fn px(d: &[u8], p: usize) -> [f64; 3] {
    let j = p * 4;
    [d[j] as f64, d[j + 1] as f64, d[j + 2] as f64]
}

/// Minimum/Maximum in einem Quadrat (trennbar, Radius r).
fn morph(v: &[f32], w: usize, h: usize, r: usize, max: bool) -> Vec<f32> {
    let pick = |a: f32, b: f32| if max { a.max(b) } else { a.min(b) };
    let mut tmp = vec![0f32; w * h];
    for y in 0..h {
        let row = &v[y * w..(y + 1) * w];
        for x in 0..w {
            let (a, b) = (x.saturating_sub(r), (x + r).min(w - 1));
            let mut m = row[a];
            for &q in &row[a + 1..=b] {
                m = pick(m, q);
            }
            tmp[y * w + x] = m;
        }
    }
    let mut out = vec![0f32; w * h];
    for x in 0..w {
        for y in 0..h {
            let (a, b) = (y.saturating_sub(r), (y + r).min(h - 1));
            let mut m = tmp[a * w + x];
            for yy in a + 1..=b {
                m = pick(m, tmp[yy * w + x]);
            }
            out[y * w + x] = m;
        }
    }
    out
}

fn morph_mask(m: &[bool], w: usize, h: usize, r: usize, dilate: bool) -> Vec<bool> {
    let f: Vec<f32> = m.iter().map(|&b| if b { 1.0 } else { 0.0 }).collect();
    morph(&f, w, h, r, dilate).into_iter().map(|v| v > 0.5).collect()
}

/// Übermalte Pixel aus den Nachbarn auffüllen (von außen nach innen).
fn inpaint(d: &mut [u8], w: usize, h: usize, mask: &[bool]) {
    let mut todo: Vec<bool> = mask.to_vec();
    let mut left = todo.iter().filter(|&&b| b).count();
    let mut guard = 0;
    while left > 0 && guard < 400 {
        guard += 1;
        let mut fill: Vec<(usize, [u8; 4])> = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let p = y * w + x;
                if !todo[p] {
                    continue;
                }
                let mut acc = [0u32; 4];
                let mut n = 0u32;
                for (dx, dy) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1), (-1, -1), (1, 1), (-1, 1), (1, -1)] {
                    let (xx, yy) = (x as i64 + dx, y as i64 + dy);
                    if xx < 0 || yy < 0 || xx >= w as i64 || yy >= h as i64 {
                        continue;
                    }
                    let q = yy as usize * w + xx as usize;
                    if todo[q] {
                        continue;
                    }
                    for k in 0..4 {
                        acc[k] += d[q * 4 + k] as u32;
                    }
                    n += 1;
                }
                if n >= 2 || (n == 1 && guard > 3) {
                    fill.push((p, [(acc[0] / n) as u8, (acc[1] / n) as u8, (acc[2] / n) as u8, (acc[3] / n) as u8]));
                }
            }
        }
        if fill.is_empty() {
            break;
        }
        for (p, c) in fill {
            d[p * 4..p * 4 + 4].copy_from_slice(&c);
            todo[p] = false;
            left -= 1;
        }
    }
}

// ---------------------------------------------------------------------------
// 1. Hintergrund außen freistellen

/// Vom Bildrand aus alles mit randähnlicher Farbe transparent machen.
/// Rückgabe: Anteil der freigestellten Pixel.
pub fn cut_background(d: &mut [u8], w: usize, h: usize, tol: f64) -> f64 {
    let n = w * h;
    if n == 0 {
        return 0.0;
    }
    // Randfarbe: Median der Randpixel je Kanal
    let mut border: Vec<usize> = Vec::new();
    for x in 0..w {
        border.push(x);
        border.push((h - 1) * w + x);
    }
    for y in 0..h {
        border.push(y * w);
        border.push(y * w + w - 1);
    }
    let opaque: Vec<usize> = border.iter().copied().filter(|&p| d[p * 4 + 3] >= 128).collect();
    if opaque.len() * 2 < border.len() {
        return 0.0; // schon weitgehend transparent
    }
    let mut ch: [Vec<u8>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    for &p in &opaque {
        for k in 0..3 {
            ch[k].push(d[p * 4 + k]);
        }
    }
    let mut bg = [0f64; 3];
    for k in 0..3 {
        ch[k].sort();
        bg[k] = ch[k][ch[k].len() / 2] as f64;
    }
    // nur freistellen, wenn der Rand wirklich einfarbig ist
    let uniform = opaque.iter().filter(|&&p| cdist(px(d, p), bg) < tol).count();
    if uniform * 10 < opaque.len() * 8 {
        return 0.0;
    }
    let mut seen = vec![false; n];
    let mut stack: Vec<usize> = opaque.into_iter().filter(|&p| cdist(px(d, p), bg) < tol).collect();
    for &p in &stack {
        seen[p] = true;
    }
    while let Some(p) = stack.pop() {
        let (x, y) = (p % w, p / w);
        let nb = [
            if x > 0 { Some(p - 1) } else { None },
            if x + 1 < w { Some(p + 1) } else { None },
            if y > 0 { Some(p - w) } else { None },
            if y + 1 < h { Some(p + w) } else { None },
        ];
        for q in nb.into_iter().flatten() {
            if !seen[q] && (d[q * 4 + 3] < 128 || cdist(px(d, q), bg) < tol) {
                seen[q] = true;
                stack.push(q);
            }
        }
    }
    // Saum: halbtransparente Kantenpixel zwischen Hintergrund und Motiv mitnehmen,
    // wenn sie deutlich näher an der Randfarbe liegen
    let grown = morph_mask(&seen, w, h, 1, true);
    let mut cnt = 0usize;
    for p in 0..n {
        let cut = seen[p] || (grown[p] && cdist(px(d, p), bg) < tol * 1.6);
        if cut {
            d[p * 4 + 3] = 0;
            cnt += 1;
        }
    }
    cnt as f64 / n as f64
}

// ---------------------------------------------------------------------------
// 2. Feine Linien

fn thin(m: &mut [bool], w: usize, h: usize) {
    // Zhang-Suen-Verdünnung
    let at = |m: &[bool], x: i64, y: i64| -> bool { x >= 0 && y >= 0 && x < w as i64 && y < h as i64 && m[y as usize * w + x as usize] };
    loop {
        let mut changed = false;
        for step in 0..2 {
            let mut del = Vec::new();
            for y in 0..h as i64 {
                for x in 0..w as i64 {
                    if !m[y as usize * w + x as usize] {
                        continue;
                    }
                    let p = [at(m, x, y - 1), at(m, x + 1, y - 1), at(m, x + 1, y), at(m, x + 1, y + 1), at(m, x, y + 1), at(m, x - 1, y + 1), at(m, x - 1, y), at(m, x - 1, y - 1)];
                    let b = p.iter().filter(|&&v| v).count();
                    if !(2..=6).contains(&b) {
                        continue;
                    }
                    let a = (0..8).filter(|&i| !p[i] && p[(i + 1) % 8]).count();
                    if a != 1 {
                        continue;
                    }
                    let (c1, c2) = if step == 0 { (p[0] && p[2] && p[4], p[2] && p[4] && p[6]) } else { (p[0] && p[2] && p[6], p[0] && p[4] && p[6]) };
                    if c1 || c2 {
                        continue;
                    }
                    del.push(y as usize * w + x as usize);
                }
            }
            if !del.is_empty() {
                changed = true;
            }
            for p in del {
                m[p] = false;
            }
        }
        if !changed {
            break;
        }
    }
}

const N8: [(i64, i64); 8] = [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)];

/// Skelett in Ketten zwischen End- und Verzweigungspunkten zerlegen.
/// Knoten über die Kreuzungszahl (Übergänge im 8er-Ring), damit Treppen in
/// schrägen Linien nicht als Verzweigung gelten.
fn node_kind(sk: &[bool], w: usize, h: usize, p: usize) -> u8 {
    let (x, y) = ((p % w) as i64, (p / w) as i64);
    let ring: Vec<bool> = N8.iter().map(|(dx, dy)| {
        let (xx, yy) = (x + dx, y + dy);
        xx >= 0 && yy >= 0 && xx < w as i64 && yy < h as i64 && sk[yy as usize * w + xx as usize]
    }).collect();
    let t = (0..8).filter(|&i| !ring[i] && ring[(i + 1) % 8]).count();
    let cnt = ring.iter().filter(|&&b| b).count();
    if cnt == 0 {
        3 // Einzelpunkt
    } else if t <= 1 {
        1 // Ende
    } else if t >= 3 {
        2 // Verzweigung
    } else {
        0 // Linie
    }
}

fn chains(sk: &[bool], w: usize, h: usize) -> Vec<Vec<usize>> {
    let nbs = |p: usize| -> Vec<usize> {
        let (x, y) = ((p % w) as i64, (p / w) as i64);
        // erst waagrecht/senkrecht, dann schräg
        [(1i64, 0i64), (0, 1), (-1, 0), (0, -1), (1, 1), (-1, 1), (-1, -1), (1, -1)]
            .iter()
            .filter_map(|(dx, dy)| {
                let (xx, yy) = (x + dx, y + dy);
                if xx >= 0 && yy >= 0 && xx < w as i64 && yy < h as i64 && sk[yy as usize * w + xx as usize] {
                    Some(yy as usize * w + xx as usize)
                } else {
                    None
                }
            })
            .collect()
    };
    let kind: Vec<u8> = (0..w * h).map(|p| if sk[p] { node_kind(sk, w, h, p) } else { 255 }).collect();
    let is_node = |p: usize| kind[p] == 1 || kind[p] == 2;
    let mut used = vec![false; w * h];
    let mut out = Vec::new();
    let walk = |start: usize, first: usize, used: &mut Vec<bool>| -> Vec<usize> {
        let mut ch = vec![start, first];
        let mut cur = first;
        while !is_node(cur) {
            used[cur] = true;
            let nx = nbs(cur).into_iter().find(|&q| !used[q] && q != start || (is_node(q) && q != ch[ch.len() - 2] && ch.len() > 3 && q != start));
            match nx {
                Some(q) => {
                    ch.push(q);
                    cur = q;
                }
                None => break,
            }
        }
        ch
    };
    for s in 0..w * h {
        if !sk[s] || !is_node(s) {
            continue;
        }
        used[s] = true;
        for first in nbs(s) {
            if used[first] && !is_node(first) {
                continue;
            }
            if is_node(first) {
                continue; // Knoten direkt neben Knoten: keine Kette
            }
            let ch = walk(s, first, &mut used);
            if ch.len() >= 3 {
                out.push(ch);
            }
        }
    }
    // Ringe ohne Knoten
    for s in 0..w * h {
        if !sk[s] || used[s] || is_node(s) {
            continue;
        }
        let mut ch = vec![s];
        used[s] = true;
        let mut cur = s;
        while let Some(q) = nbs(cur).into_iter().find(|&q| !used[q] && !is_node(q)) {
            used[q] = true;
            ch.push(q);
            cur = q;
        }
        if ch.len() >= 8 {
            ch.push(ch[0]);
            out.push(ch);
        }
    }
    out
}

/// Kurze Seitenäste (Sporne) vom Skelett entfernen, damit Linien nicht zerfallen.
fn prune(sk: &mut [bool], w: usize, h: usize, spur: usize) {
    for _ in 0..3 {
        let mut removed = false;
        for ch in chains(sk, w, h) {
            if ch.len() > spur || ch.first() == ch.last() {
                continue;
            }
            let (a, b) = (ch[0], ch[ch.len() - 1]);
            let (da, db) = (node_kind(sk, w, h, a), node_kind(sk, w, h, b));
            // nur Äste, die an einer Verzweigung hängen und frei enden
            let (end_free, junction) = (da == 1 || db == 1, da == 2 || db == 2);
            if end_free && junction {
                for &p in &ch {
                    if node_kind(sk, w, h, p) != 2 {
                        sk[p] = false;
                    }
                }
                removed = true;
            } else if da == 1 && db == 1 && ch.len() < spur / 2 {
                for &p in &ch {
                    sk[p] = false;
                }
                removed = true;
            }
        }
        if !removed {
            break;
        }
        thin(sk, w, h);
    }
}

fn seg_dist(p: Pt, a: Pt, b: Pt) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dy * dy;
    if l2 < 1e-12 {
        return ((p[0] - a[0]).powi(2) + (p[1] - a[1]).powi(2)).sqrt();
    }
    let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0);
    ((p[0] - a[0] - t * dx).powi(2) + (p[1] - a[1] - t * dy).powi(2)).sqrt()
}

#[allow(dead_code)]
fn rdp(a: &[Pt], eps: f64) -> Vec<Pt> {
    if a.len() < 3 {
        return a.to_vec();
    }
    let (mut idx, mut dm) = (0, 0.0);
    for i in 1..a.len() - 1 {
        let d = seg_dist(a[i], a[0], a[a.len() - 1]);
        if d > dm {
            dm = d;
            idx = i;
        }
    }
    if dm > eps {
        let mut l = rdp(&a[..=idx], eps);
        let r = rdp(&a[idx..], eps);
        l.pop();
        l.extend(r);
        l
    } else {
        vec![a[0], a[a.len() - 1]]
    }
}

/// Kreis durch Punkte (algebraisch). Rückgabe: (Mitte, Radius, mittlerer Fehler).
fn fit_circle(p: &[Pt]) -> Option<(Pt, f64, f64)> {
    if p.len() < 6 {
        return None;
    }
    let n = p.len() as f64;
    let (mx, my) = (p.iter().map(|q| q[0]).sum::<f64>() / n, p.iter().map(|q| q[1]).sum::<f64>() / n);
    let (mut suu, mut svv, mut suv, mut suuu, mut svvv, mut suvv, mut svuu) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for q in p {
        let (u, v) = (q[0] - mx, q[1] - my);
        suu += u * u;
        svv += v * v;
        suv += u * v;
        suuu += u * u * u;
        svvv += v * v * v;
        suvv += u * v * v;
        svuu += v * u * u;
    }
    let det = suu * svv - suv * suv;
    if det.abs() < 1e-9 {
        return None;
    }
    let (b1, b2) = (0.5 * (suuu + suvv), 0.5 * (svvv + svuu));
    let uc = (b1 * svv - b2 * suv) / det;
    let vc = (suu * b2 - suv * b1) / det;
    let r = (uc * uc + vc * vc + (suu + svv) / n).sqrt();
    let c = [uc + mx, vc + my];
    let err = p.iter().map(|q| (((q[0] - c[0]).powi(2) + (q[1] - c[1]).powi(2)).sqrt() - r).abs()).sum::<f64>() / n;
    Some((c, r, err))
}

/// Gerades Stück oder Kreisbogen aus dem Skelett.
struct Piece {
    pts: Vec<Pt>,
    len: f64,
    color: [f64; 3],
    width: f64,
}

/// Gerade durch Punkte (Hauptachse). Rückgabe: Anfang, Ende (Projektion), mittlerer Abstand.
fn fit_seg(p: &[Pt]) -> Option<(Pt, Pt, f64)> {
    if p.len() < 2 {
        return None;
    }
    let n = p.len() as f64;
    let (mx, my) = (p.iter().map(|q| q[0]).sum::<f64>() / n, p.iter().map(|q| q[1]).sum::<f64>() / n);
    let (mut sxx, mut syy, mut sxy) = (0.0, 0.0, 0.0);
    for q in p {
        let (u, v) = (q[0] - mx, q[1] - my);
        sxx += u * u;
        syy += v * v;
        sxy += u * v;
    }
    let ang = 0.5 * (2.0 * sxy).atan2(sxx - syy);
    let d = [ang.cos(), ang.sin()];
    let (mut t0, mut t1, mut err) = (f64::MAX, f64::MIN, 0.0);
    for q in p {
        let (u, v) = (q[0] - mx, q[1] - my);
        let t = u * d[0] + v * d[1];
        t0 = t0.min(t);
        t1 = t1.max(t);
        err += (u * -d[1] + v * d[0]).abs();
    }
    Some(([mx + d[0] * t0, my + d[1] * t0], [mx + d[0] * t1, my + d[1] * t1], err / n))
}

/// Feine helle oder dunkle Linien finden. Stücke derselben Geraden werden über
/// Kreuzungen hinweg verbunden, Kreisbögen zu Kreisen ergänzt.
/// Rückgabe: Linien (Analysepixel) und Maske zum Übermalen.
pub fn find_lines(d: &[u8], w: usize, h: usize, sc: f64, keep_out: &[bool]) -> (Vec<LineOut>, Vec<bool>) {
    let n = w * h;
    let mut lines = Vec::new();
    let mut erase = vec![false; n];
    if n == 0 {
        return (lines, erase);
    }
    let l: Vec<f32> = (0..n).map(|p| if d[p * 4 + 3] < 128 { 0.0 } else { lum(d, p) as f32 }).collect();
    let r = ((3.0 * sc).round() as usize).clamp(2, 6);
    let open = morph(&morph(&l, w, h, r, false), w, h, r, true);
    let close = morph(&morph(&l, w, h, r, true), w, h, r, false);
    let minlen = (w.min(h) as f64 * 0.05).max(30.0);
    let s = sc.max(0.7);
    let mut all_raw = vec![false; n];
    for bright in [true, false] {
        let mut m: Vec<bool> = (0..n)
            .map(|p| {
                if keep_out.get(p).copied().unwrap_or(false) || d[p * 4 + 3] < 128 {
                    return false;
                }
                let v = if bright { l[p] - open[p] } else { close[p] - l[p] };
                v > 26.0
            })
            .collect();
        let raw = m.clone();
        thin(&mut m, w, h);
        prune(&mut m, w, h, (9.0 * s) as usize);
        // Breite und Farbe je Kette
        let mut segs: Vec<Piece> = Vec::new();
        let mut arcs: Vec<Piece> = Vec::new();
        for ch in chains(&m, w, h) {
            let pts: Vec<Pt> = ch.iter().map(|&p| [(p % w) as f64 + 0.5, (p / w) as f64 + 0.5]).collect();
            let len: f64 = pts.windows(2).map(|q| ((q[1][0] - q[0][0]).powi(2) + (q[1][1] - q[0][1]).powi(2)).sqrt()).sum();
            if len < 10.0 * s {
                continue;
            }
            let rr = r as i64 + 1;
            let mut near = std::collections::HashSet::new();
            let mut col = [0f64; 3];
            for &p in &ch {
                // Farbe der Linie: hellster (bzw. dunkelster) Punkt im Umfeld –
                // der Mittelwert wäre bei dünnen Linien mit dem Untergrund vermischt
                let (x0, y0) = ((p % w) as i64, (p / w) as i64);
                let mut pick = p;
                for dy in -1i64..=1 {
                    for dx in -1i64..=1 {
                        let (xx, yy) = (x0 + dx, y0 + dy);
                        if xx < 0 || yy < 0 || xx >= w as i64 || yy >= h as i64 {
                            continue;
                        }
                        let q = yy as usize * w + xx as usize;
                        if (bright && lum(d, q) > lum(d, pick)) || (!bright && lum(d, q) < lum(d, pick)) {
                            pick = q;
                        }
                    }
                }
                let c = px(d, pick);
                for k in 0..3 {
                    col[k] += c[k] / ch.len() as f64;
                }
                let (x, y) = ((p % w) as i64, (p / w) as i64);
                for dy in -rr..=rr {
                    for dx in -rr..=rr {
                        let (xx, yy) = (x + dx, y + dy);
                        if xx >= 0 && yy >= 0 && xx < w as i64 && yy < h as i64 && raw[yy as usize * w + xx as usize] {
                            near.insert(yy as usize * w + xx as usize);
                        }
                    }
                }
            }
            let width = (near.len() as f64 / len.max(1.0)).clamp(1.0, 2.0 * r as f64);
            // Kette gierig in Kreisbögen und gerade Stücke zerlegen (eine Kette kann
            // über eine Kreuzung hinweg von einem Kreis in eine Gerade übergehen)
            let np = pts.len();
            let rmax = 0.5 * w.min(h) as f64;
            let rmin = w.min(h) as f64 * 0.02;
            let dbg = std::env::var_os("ZMN_DEBUG_LINES").is_some();
            let mut i = 0;
            while i + 3 < np {
                let alen = |e: usize| pts[i..e].windows(2).map(|q| ((q[1][0] - q[0][0]).powi(2) + (q[1][1] - q[0][1]).powi(2)).sqrt()).sum::<f64>();
                // längster Bogen ab i
                let mut arc_end = i;
                let mut j = i + 8;
                while j <= np {
                    let part = &pts[i..j];
                    let ok = fit_circle(part).map(|c| c.2 < 0.9 * s && c.1 > rmin && c.1 < rmax).unwrap_or(false);
                    let bowed = {
                        let (a0, a1) = (part[0], part[part.len() - 1]);
                        part.iter().map(|q| seg_dist(*q, a0, a1)).fold(0.0, f64::max) > 2.5 * s
                    };
                    if ok && bowed {
                        arc_end = j;
                    } else if !ok && (arc_end > i || j > i + 40) {
                        break;
                    }
                    j += 3;
                }
                // längstes gerades Stück ab i
                let mut seg_end = i;
                let mut j2 = i + 4;
                while j2 <= np {
                    if fit_seg(&pts[i..j2]).map(|f| f.2 < 0.8 * s).unwrap_or(false) {
                        seg_end = j2;
                        j2 += 2;
                    } else {
                        break;
                    }
                }
                if arc_end > seg_end + 4 && alen(arc_end) >= 18.0 * s {
                    if dbg {
                        eprintln!("  bogen {:?}", fit_circle(&pts[i..arc_end]).map(|c| (c.0[0].round(), c.0[1].round(), c.1.round())));
                    }
                    arcs.push(Piece { pts: pts[i..arc_end].to_vec(), len: alen(arc_end), color: col, width });
                    i = arc_end - 1;
                } else if seg_end > i + 3 {
                    let l2 = alen(seg_end);
                    if l2 >= 10.0 * s {
                        segs.push(Piece { pts: pts[i..seg_end].to_vec(), len: l2, color: col, width });
                    }
                    i = seg_end - 1;
                } else {
                    i += 2;
                }
            }
        }
        // gerade Stücke auf derselben Geraden zusammenfassen
        let mut used = vec![false; segs.len()];
        let fits: Vec<Option<(Pt, Pt, f64)>> = segs.iter().map(|sg| fit_seg(&sg.pts)).collect();
        for i in 0..segs.len() {
            if used[i] || fits[i].is_none() {
                continue;
            }
            used[i] = true;
            let mut group = vec![i];
            let mut grew = true;
            while grew {
                grew = false;
                let mut pts: Vec<Pt> = group.iter().flat_map(|&g| segs[g].pts.iter().copied()).collect();
                let (a, b, _) = fit_seg(&pts).unwrap();
                let dl = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt().max(1e-6);
                let dir = [(b[0] - a[0]) / dl, (b[1] - a[1]) / dl];
                for j in 0..segs.len() {
                    if used[j] {
                        continue;
                    }
                    let Some((c, e, _)) = fits[j] else { continue };
                    let el = ((e[0] - c[0]).powi(2) + (e[1] - c[1]).powi(2)).sqrt().max(1e-6);
                    let dj = [(e[0] - c[0]) / el, (e[1] - c[1]) / el];
                    let cosang = (dir[0] * dj[0] + dir[1] * dj[1]).abs();
                    if cosang < (3.0f64).to_radians().cos() {
                        continue;
                    }
                    let off = |q: Pt| ((q[0] - a[0]) * -dir[1] + (q[1] - a[1]) * dir[0]).abs();
                    if off(c) > 2.2 * s || off(e) > 2.2 * s {
                        continue;
                    }
                    // Lücke entlang der Geraden
                    let t = |q: Pt| (q[0] - a[0]) * dir[0] + (q[1] - a[1]) * dir[1];
                    let (tc, te) = (t(c).min(t(e)), t(c).max(t(e)));
                    let gap = if te < 0.0 { -te } else if tc > dl { tc - dl } else { 0.0 };
                    if gap > 70.0 * s {
                        continue;
                    }
                    used[j] = true;
                    group.push(j);
                    pts.extend(segs[j].pts.iter().copied());
                    grew = true;
                }
            }
            let pts: Vec<Pt> = group.iter().flat_map(|&g| segs[g].pts.iter().copied()).collect();
            let total: f64 = group.iter().map(|&g| segs[g].len).sum();
            if total < minlen {
                continue;
            }
            let (a, b, err) = fit_seg(&pts).unwrap();
            if err > 1.4 * s {
                continue;
            }
            let k = group.len() as f64;
            let width = group.iter().map(|&g| segs[g].width).sum::<f64>() / k;
            let col = [0, 1, 2].map(|c| group.iter().map(|&g| segs[g].color[c]).sum::<f64>() / k);
            lines.push(LineOut { pts: vec![a, b], closed: false, width, color: col.map(|v| v.round() as u8) });
        }
        // Kreisbögen mit gleichem Kreis zusammenfassen
        let circs: Vec<Option<(Pt, f64, f64)>> = arcs.iter().map(|a| fit_circle(&a.pts)).collect();
        let mut used = vec![false; arcs.len()];
        for i in 0..arcs.len() {
            let Some((c0, r0, _)) = circs[i] else { continue };
            if used[i] {
                continue;
            }
            used[i] = true;
            let mut group = vec![i];
            for j in i + 1..arcs.len() {
                if let Some((c1, r1, _)) = circs[j] {
                    if !used[j] && ((c1[0] - c0[0]).powi(2) + (c1[1] - c0[1]).powi(2)).sqrt() < 0.12 * r0 + 2.0 && (r1 - r0).abs() < 0.08 * r0 + 1.5 {
                        used[j] = true;
                        group.push(j);
                    }
                }
            }
            let pts: Vec<Pt> = group.iter().flat_map(|&g| arcs[g].pts.iter().copied()).collect();
            let Some((c, rad, err)) = fit_circle(&pts) else { continue };
            if err > 1.2 * s {
                continue;
            }
            // abgedeckte Winkel (in 10°-Schritten)
            let mut bins = [false; 36];
            for q in &pts {
                let a = (q[1] - c[1]).atan2(q[0] - c[0]);
                let b = (((a + std::f64::consts::PI) / std::f64::consts::TAU * 36.0) as usize).min(35);
                bins[b] = true;
            }
            let cover = bins.iter().filter(|&&b| b).count();
            let total: f64 = group.iter().map(|&g| arcs[g].len).sum();
            if total < minlen {
                continue;
            }
            let k = group.len() as f64;
            let width = group.iter().map(|&g| arcs[g].width).sum::<f64>() / k;
            let col = [0, 1, 2].map(|cc| group.iter().map(|&g| arcs[g].color[cc]).sum::<f64>() / k).map(|v| v.round() as u8);
            if cover >= 22 {
                // ganzer Kreis
                let pts: Vec<Pt> = (0..96).map(|kk| {
                    let t = kk as f64 / 96.0 * std::f64::consts::TAU;
                    [c[0] + rad * t.cos(), c[1] + rad * t.sin()]
                }).collect();
                lines.push(LineOut { pts, closed: true, width, color: col });
            } else {
                // Bogen über die abgedeckten Winkel (größter zusammenhängender Bereich)
                let mut best = (0usize, 0usize);
                for st in 0..36 {
                    if !bins[st] || bins[(st + 35) % 36] {
                        continue;
                    }
                    let mut len = 0;
                    while len < 36 && bins[(st + len) % 36] {
                        len += 1;
                    }
                    if len > best.1 {
                        best = (st, len);
                    }
                }
                if best.1 == 0 {
                    continue;
                }
                let a0 = -std::f64::consts::PI + best.0 as f64 / 36.0 * std::f64::consts::TAU;
                let a1 = a0 + best.1 as f64 / 36.0 * std::f64::consts::TAU;
                let steps = (best.1 * 3).max(6);
                let pts: Vec<Pt> = (0..=steps).map(|kk| {
                    let t = a0 + (a1 - a0) * kk as f64 / steps as f64;
                    [c[0] + rad * t.cos(), c[1] + rad * t.sin()]
                }).collect();
                lines.push(LineOut { pts, closed: false, width, color: col });
            }
        }
        for p in 0..n {
            if raw[p] {
                all_raw[p] = true;
            }
        }
    }
    // übermalen: Linienpixel in der Nähe der gefundenen Linien
    let mut cov = vec![0f32; n];
    for ln in &lines {
        stroke_line(&mut cov, w, h, &ln.pts, ln.closed, ln.width + 4.0 * s, 1);
    }
    let grown = morph_mask(&all_raw, w, h, 1, true);
    for p in 0..n {
        erase[p] = cov[p] > 0.0 && grown[p];
    }
    (lines, erase)
}

// ---------------------------------------------------------------------------
// 3. Fotobereiche beruhigen

/// Unruhige, großflächige Bereiche (Fotos, Muster) finden und kantenerhaltend glätten.
/// Rückgabe: Anteil der Fotobereiche.
pub fn calm_photo(d: &mut Vec<u8>, w: usize, h: usize, sc: f64) -> f64 {
    let n = w * h;
    if n == 0 {
        return 0.0;
    }
    let l: Vec<f64> = (0..n).map(|p| lum(d, p)).collect();
    // Kantenpixel
    let mut e = vec![0u32; n];
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let p = y * w + x;
            let g = (l[p + 1] - l[p - 1]).abs() + (l[p + w] - l[p - w]).abs();
            if g > 10.0 && d[p * 4 + 3] >= 128 {
                e[p] = 1;
            }
        }
    }
    // Summentabelle
    let mut s = vec![0u32; (w + 1) * (h + 1)];
    for y in 0..h {
        let mut row = 0u32;
        for x in 0..w {
            row += e[y * w + x];
            s[(y + 1) * (w + 1) + x + 1] = s[y * (w + 1) + x + 1] + row;
        }
    }
    let r = ((10.0 * sc).round() as usize).max(4);
    let mut busy = vec![false; n];
    for y in 0..h {
        for x in 0..w {
            let (x0, y0, x1, y1) = (x.saturating_sub(r), y.saturating_sub(r), (x + r + 1).min(w), (y + r + 1).min(h));
            let cnt = s[y1 * (w + 1) + x1] + s[y0 * (w + 1) + x0] - s[y0 * (w + 1) + x1] - s[y1 * (w + 1) + x0];
            let area = ((x1 - x0) * (y1 - y0)) as f64;
            busy[y * w + x] = cnt as f64 / area > 0.38 && d[(y * w + x) * 4 + 3] >= 128;
        }
    }
    // schmale Bereiche (Schrift) herausnehmen: Öffnen
    let k = ((14.0 * sc).round() as usize).max(5);
    let busy = morph_mask(&morph_mask(&busy, w, h, k, false), w, h, k + 2, true);
    let frac = busy.iter().filter(|&&b| b).count() as f64 / n as f64;
    if frac < 0.01 {
        return 0.0;
    }
    // innerhalb der Fotobereiche stark weichzeichnen (nur Fotopixel mitteln,
    // damit nichts vom Motiv hineinblutet) – es bleiben weiche Verläufe
    let rb = ((9.0 * sc).round() as usize).max(4);
    let mut cur = d.clone();
    let wts: Vec<f32> = busy.iter().map(|&b| if b { 1.0 } else { 0.0 }).collect();
    let mut acc: Vec<[f32; 4]> = (0..n).map(|p| {
        let k = wts[p];
        [cur[p * 4] as f32 * k, cur[p * 4 + 1] as f32 * k, cur[p * 4 + 2] as f32 * k, k]
    }).collect();
    for _ in 0..3 {
        acc = box_blur4(&acc, w, h, rb);
    }
    for p in 0..n {
        if busy[p] && acc[p][3] > 1e-4 {
            let b = [0, 1, 2].map(|k| acc[p][k] / acc[p][3]);
            // starke Kontraste (Schrift, Lichter) bleiben, Muster und Rauschen gehen
            let lb = 0.299 * b[0] + 0.587 * b[1] + 0.114 * b[2];
            if (l[p] as f32 - lb).abs() > 48.0 {
                continue;
            }
            for k in 0..3 {
                cur[p * 4 + k] = b[k].round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    *d = cur;
    frac
}

/// Kastenfilter (trennbar, Summen über gleitendes Fenster).
fn box_blur4(v: &[[f32; 4]], w: usize, h: usize, r: usize) -> Vec<[f32; 4]> {
    let mut tmp = vec![[0f32; 4]; w * h];
    for y in 0..h {
        let mut s = [0f32; 4];
        for x in 0..=r.min(w - 1) {
            for k in 0..4 {
                s[k] += v[y * w + x][k];
            }
        }
        for x in 0..w {
            tmp[y * w + x] = s;
            if x + r + 1 < w {
                for k in 0..4 {
                    s[k] += v[y * w + x + r + 1][k];
                }
            }
            if x >= r {
                for k in 0..4 {
                    s[k] -= v[y * w + x - r][k];
                }
            }
        }
    }
    let mut out = vec![[0f32; 4]; w * h];
    for x in 0..w {
        let mut s = [0f32; 4];
        for y in 0..=r.min(h - 1) {
            for k in 0..4 {
                s[k] += tmp[y * w + x][k];
            }
        }
        for y in 0..h {
            out[y * w + x] = s;
            if y + r + 1 < h {
                for k in 0..4 {
                    s[k] += tmp[(y + r + 1) * w + x][k];
                }
            }
            if y >= r {
                for k in 0..4 {
                    s[k] -= tmp[(y - r) * w + x][k];
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// 4. Text

/// Eine erkannte Textzeile (Rechteck im Quellbild: x0, y0, x1, y1).
#[derive(Clone, Debug)]
pub struct TextLine {
    pub text: String,
    pub bbox: [f64; 4],
}

/// Ein Zeichen (oder mehrere zusammenhängende) mit seiner Tintenmaske.
#[derive(Clone, Debug)]
pub struct TextSeg {
    pub chars: String,
    /// Tintenrechteck im Quellbild
    pub ink: [f64; 4],
    pub mw: usize,
    pub mh: usize,
    pub mask: Vec<bool>,
}

#[derive(Clone, Debug)]
pub struct TextAnalysis {
    pub line: TextLine,
    pub fg: [u8; 3],
    pub bg: [u8; 3],
    /// gefunden: ein Abschnitt je sichtbarem Zeichen; sonst die ganze Zeile als ein Abschnitt
    pub segs: Vec<TextSeg>,
    pub per_char: bool,
    /// Zeichen der Zeile (ohne Leerzeichen)
    pub chars: Vec<char>,
    /// Tintenmaske der ganzen Zeile (Analysepixel) mit Lage im Analysebild
    pub lmask: Vec<bool>,
    pub lw: usize,
    pub lh: usize,
    pub lx: usize,
    pub ly: usize,
    pub sc: f64,
    /// feine Zerlegung in Spaltenbereiche (Bausteine; ein Zeichen kann mehrere umfassen)
    pub atoms: Vec<(usize, usize)>,
    /// Maske der ganzen Zeile (Analysepixel, global)
    pub erase: Vec<usize>,
}

/// Fertige Schrift einer Zeile (Umrisse im Quellbild).
#[derive(Clone, Debug)]
pub struct TextOut {
    pub line: TextLine,
    pub color: [u8; 3],
    pub shapes: Vec<Shape>,
    pub erase: Vec<usize>,
}

/// Tintenpixel einer Textzeile bestimmen und in Zeichen zerlegen.
pub fn analyze_text(doc: &Doc, line: &TextLine) -> Option<TextAnalysis> {
    let (w, h, sc) = (doc.w, doc.h, doc.sc);
    let d = &doc.data;
    let pad = 3.0;
    let x0 = ((line.bbox[0] * sc - pad).floor().max(0.0)) as usize;
    let y0 = ((line.bbox[1] * sc - pad).floor().max(0.0)) as usize;
    let x1 = ((line.bbox[2] * sc + pad).ceil() as usize).min(w);
    let y1 = ((line.bbox[3] * sc + pad).ceil() as usize).min(h);
    if x1 <= x0 + 2 || y1 <= y0 + 2 {
        return None;
    }
    let (bw, bh) = (x1 - x0, y1 - y0);
    let idx = |x: usize, y: usize| (y0 + y) * w + x0 + x;
    // zwei Farben (dunkel/hell) per k-Means
    let mut c = [[0f64; 3], [255.0; 3]];
    {
        let mut lo = (f64::MAX, [0f64; 3]);
        let mut hi = (f64::MIN, [0f64; 3]);
        for y in 0..bh {
            for x in 0..bw {
                let p = idx(x, y);
                let v = lum(d, p);
                if v < lo.0 {
                    lo = (v, px(d, p));
                }
                if v > hi.0 {
                    hi = (v, px(d, p));
                }
            }
        }
        c[0] = lo.1;
        c[1] = hi.1;
    }
    let mut lab = vec![0u8; bw * bh];
    for _ in 0..8 {
        let mut acc = [[0f64; 4]; 2];
        for y in 0..bh {
            for x in 0..bw {
                let p = idx(x, y);
                let q = px(d, p);
                let k = if cdist(q, c[0]) <= cdist(q, c[1]) { 0 } else { 1 };
                lab[y * bw + x] = k as u8;
                for j in 0..3 {
                    acc[k][j] += q[j];
                }
                acc[k][3] += 1.0;
            }
        }
        for k in 0..2 {
            if acc[k][3] > 0.0 {
                c[k] = [acc[k][0] / acc[k][3], acc[k][1] / acc[k][3], acc[k][2] / acc[k][3]];
            }
        }
    }
    if cdist(c[0], c[1]) < 20.0 {
        return None; // kein Kontrast
    }
    // Textklasse: die mit weniger Randpixeln
    let mut border = [0usize; 2];
    for x in 0..bw {
        border[lab[x] as usize] += 1;
        border[lab[(bh - 1) * bw + x] as usize] += 1;
    }
    for y in 0..bh {
        border[lab[y * bw] as usize] += 1;
        border[lab[y * bw + bw - 1] as usize] += 1;
    }
    let tk = if border[0] <= border[1] { 0u8 } else { 1u8 };
    let mut m: Vec<bool> = lab.iter().map(|&v| v == tk).collect();
    // Splitter entfernen: winzige Teile, große Randflächen und kleine Randstücke
    // außerhalb des Schriftbands (z. B. Kanten von Nachbarformen in der Box)
    {
        let mut seen = vec![false; bw * bh];
        let mut comps: Vec<(Vec<usize>, usize, usize, bool)> = Vec::new(); // Pixel, oben, unten, berührt Rand
        for s in 0..bw * bh {
            if !m[s] || seen[s] {
                continue;
            }
            let mut comp = vec![s];
            seen[s] = true;
            let mut i = 0;
            while i < comp.len() {
                let p = comp[i];
                i += 1;
                let (x, y) = (p % bw, p / bw);
                let nb = [if x > 0 { Some(p - 1) } else { None }, if x + 1 < bw { Some(p + 1) } else { None }, if y > 0 { Some(p - bw) } else { None }, if y + 1 < bh { Some(p + bw) } else { None }];
                for q in nb.into_iter().flatten() {
                    if m[q] && !seen[q] {
                        seen[q] = true;
                        comp.push(q);
                    }
                }
            }
            let touches = comp.iter().any(|&p| p % bw == 0 || p % bw == bw - 1 || p / bw == 0 || p / bw == bh - 1);
            let top = comp.iter().map(|&p| p / bw).min().unwrap_or(0);
            let bot = comp.iter().map(|&p| p / bw).max().unwrap_or(0) + 1;
            comps.push((comp, top, bot, touches));
        }
        // Schriftband aus den großen Teilen (Höhe ≥ 45 % der höchsten)
        let hmax = comps.iter().map(|c| c.2 - c.1).max().unwrap_or(0);
        let big: Vec<&(Vec<usize>, usize, usize, bool)> = comps.iter().filter(|c| (c.2 - c.1) * 100 >= hmax * 45 && !c.3).collect();
        let (bt, bb) = if big.is_empty() { (0, bh) } else { (big.iter().map(|c| c.1).min().unwrap(), big.iter().map(|c| c.2).max().unwrap()) };
        let band = (bb - bt).max(1);
        for (comp, top, bot, touches) in comps {
            let h = bot - top;
            let small = h * 2 < band;
            let outside = bot <= bt + band / 5 || top + band / 5 >= bb;
            let drop = comp.len() < 6 || (touches && comp.len() > bw * bh / 4) || (touches && small && outside) || (touches && h * 4 < band);
            if drop {
                for p in comp {
                    m[p] = false;
                }
            }
        }
    }
    if let Some(dir) = std::env::var_os("ZMN_DEBUG_DIR") {
        let mut pgm = format!("P5 {bw} {bh} 255\n").into_bytes();
        pgm.extend(m.iter().map(|&b| if b { 255u8 } else { 0 }));
        let name: String = line.text.chars().filter(|c| c.is_alphanumeric()).collect();
        let _ = std::fs::write(std::path::Path::new(&dir).join(format!("maske-{name}.pgm")), pgm);
    }
    // Spaltenprojektion
    let cols: Vec<bool> = (0..bw).map(|x| (0..bh).any(|y| m[y * bw + x])).collect();
    let mut runs: Vec<(usize, usize)> = Vec::new();
    let mut x = 0;
    while x < bw {
        if cols[x] {
            let s = x;
            while x < bw && cols[x] {
                x += 1;
            }
            runs.push((s, x));
        } else {
            x += 1;
        }
    }
    if runs.is_empty() {
        return None;
    }
    let chars: Vec<char> = line.text.chars().filter(|c| !c.is_whitespace()).collect();
    // feine Zerlegung: an dünnen Stellen (lokale Minima der Spaltenfüllung) trennen.
    // Das zerschneidet auch mal ein O – die Schriftsuche setzt die Teile wieder zusammen.
    let colcnt: Vec<usize> = (0..bw).map(|x| (0..bh).filter(|&y| m[y * bw + x]).count()).collect();
    let line_h = {
        let rows: Vec<usize> = (0..bh).filter(|&y| (0..bw).any(|x| m[y * bw + x])).collect();
        if rows.is_empty() { bh } else { rows[rows.len() - 1] - rows[0] + 1 }
    };
    let mut atoms: Vec<(usize, usize)> = Vec::new();
    let minw = (line_h / 7).max(2);
    for &(a, b) in &runs {
        let mut start = a;
        let mut x = a + minw;
        while x + minw < b {
            let c = colcnt[x];
            let (wa, wb) = (x.saturating_sub(minw).max(a), (x + minw + 1).min(b));
            let wmin = colcnt[wa..wb].iter().copied().min().unwrap_or(c);
            let wmax = colcnt[wa..wb].iter().copied().max().unwrap_or(c);
            // nur echte Täler: tiefste Stelle im Umfeld, deutlich unter den Nachbarn
            if c == wmin && (c as f64) <= line_h as f64 * 0.2 && (c as f64) * 1.8 <= wmax as f64 {
                atoms.push((start, x));
                start = x;
                x += minw;
            } else {
                x += 1;
            }
        }
        atoms.push((start, b));
    }
    // zu viele Abschnitte: kleinste Lücken schließen
    while runs.len() > chars.len() && runs.len() > 1 {
        let (i, _) = runs.windows(2).enumerate().map(|(i, r)| (i, r[1].0 - r[0].1)).min_by_key(|x| x.1).unwrap();
        runs[i].1 = runs[i + 1].1;
        runs.remove(i + 1);
    }
    // Zeichen auf die Abschnitte verteilen (berührende Buchstaben bilden einen
    // gemeinsamen Abschnitt): dynamische Zuordnung nach geschätzter Breite
    let est = |c: char| -> f64 {
        match c {
            'I' | 'i' | 'l' | '1' | '.' | ',' | ':' | ';' | '!' | '|' | '\'' | 'j' => 0.3,
            'M' | 'W' | 'm' | 'w' | '@' => 0.9,
            'r' | 't' | 'f' | 'J' => 0.45,
            _ => 0.62,
        }
    };
    let nr = runs.len();
    let nc = chars.len();
    let tot_r: f64 = runs.iter().map(|r| (r.1 - r.0) as f64).sum();
    let tot_c: f64 = chars.iter().map(|&c| est(c)).sum();
    let mut groups: Vec<(usize, usize)> = Vec::new(); // Zeichenbereich je Abschnitt
    if nc >= nr && nr > 0 && tot_r > 0.0 && tot_c > 0.0 {
        let inf = f64::MAX / 4.0;
        let mut dp = vec![vec![inf; nr + 1]; nc + 1];
        let mut from = vec![vec![0usize; nr + 1]; nc + 1];
        dp[0][0] = 0.0;
        for j in 1..=nr {
            let rw = (runs[j - 1].1 - runs[j - 1].0) as f64 / tot_r;
            for i in j..=nc {
                let mut cw = 0.0;
                for a in (j - 1..i).rev() {
                    cw += est(chars[a]) / tot_c;
                    if dp[a][j - 1] >= inf {
                        continue;
                    }
                    let c = dp[a][j - 1] + (rw - cw).powi(2) / rw.max(0.01);
                    if c < dp[i][j] {
                        dp[i][j] = c;
                        from[i][j] = a;
                    }
                }
            }
        }
        let mut i = nc;
        for j in (1..=nr).rev() {
            let a = from[i][j];
            groups.push((a, i));
            i = a;
        }
        groups.reverse();
    } else {
        runs = vec![(runs[0].0, runs[runs.len() - 1].1)];
        groups.push((0, nc));
    }
    let per_char = groups.iter().all(|g| g.1 - g.0 == 1);
    let cut = |a: usize, b: usize| -> Option<TextSeg> {
        let (mut ya, mut yb) = (bh, 0);
        for y in 0..bh {
            if (a..b).any(|x| m[y * bw + x]) {
                ya = ya.min(y);
                yb = yb.max(y + 1);
            }
        }
        if yb <= ya {
            return None;
        }
        let (mw, mh) = (b - a, yb - ya);
        let mut mask = vec![false; mw * mh];
        for y in 0..mh {
            for x in 0..mw {
                mask[y * mw + x] = m[(ya + y) * bw + a + x];
            }
        }
        let ink = [(x0 + a) as f64 / sc, (y0 + ya) as f64 / sc, (x0 + b) as f64 / sc, (y0 + yb) as f64 / sc];
        Some(TextSeg { chars: String::new(), ink, mw, mh, mask })
    };
    let mut segs = Vec::new();
    for (ri, &(a, b)) in runs.iter().enumerate() {
        if let Some(mut sg) = cut(a, b) {
            let g = groups[ri];
            sg.chars = chars[g.0..g.1].iter().collect();
            segs.push(sg);
        }
    }
    // zu übermalende Pixel: Tinte etwas erweitert
    let grown = morph_mask(&m, bw, bh, 2, true);
    let erase: Vec<usize> = (0..bw * bh).filter(|&p| grown[p]).map(|p| idx(p % bw, p / bw)).collect();
    let tc = c[tk as usize];
    let bc = c[1 - tk as usize];
    Some(TextAnalysis {
        line: line.clone(),
        fg: [tc[0] as u8, tc[1] as u8, tc[2] as u8],
        bg: [bc[0] as u8, bc[1] as u8, bc[2] as u8],
        segs,
        per_char,
        chars,
        lmask: m.clone(),
        lw: bw,
        lh: bh,
        lx: x0,
        ly: y0,
        sc,
        atoms,
        erase,
    })
}

/// Polygone (in Maskenpixeln) rastern und mit der Maske vergleichen.
/// Wert 0…1: F-Maß aus Genauigkeit und Abdeckung mit 1 Pixel Toleranz –
/// so werden auch dünne Schriften fair bewertet.
pub fn mask_iou(mask: &[bool], mw: usize, mh: usize, polys: &[Vec<Pt>]) -> f64 {
    let mut cov = vec![0f32; mw * mh];
    fill_evenodd(&mut cov, mw, mh, polys, 2);
    let g: Vec<bool> = cov.iter().map(|&v| v >= 0.5).collect();
    let dil = |m: &[bool]| -> Vec<bool> {
        let mut o = m.to_vec();
        for y in 0..mh {
            for x in 0..mw {
                if m[y * mw + x] {
                    continue;
                }
                let mut hit = false;
                for (dx, dy) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)] {
                    let (xx, yy) = (x as i64 + dx, y as i64 + dy);
                    if xx >= 0 && yy >= 0 && xx < mw as i64 && yy < mh as i64 && m[yy as usize * mw + xx as usize] {
                        hit = true;
                        break;
                    }
                }
                o[y * mw + x] = hit;
            }
        }
        o
    };
    let (dm, dg) = (dil(mask), dil(&g));
    let (mut gp, mut gin, mut mp, mut min_) = (0usize, 0usize, 0usize, 0usize);
    for p in 0..mw * mh {
        if g[p] {
            gp += 1;
            if dm[p] {
                gin += 1;
            }
        }
        if mask[p] {
            mp += 1;
            if dg[p] {
                min_ += 1;
            }
        }
    }
    if gp == 0 || mp == 0 {
        return 0.0;
    }
    let (pr, rc) = (gin as f64 / gp as f64, min_ as f64 / mp as f64);
    if pr + rc <= 0.0 {
        0.0
    } else {
        // etwas strenger als das reine F-Maß, damit Unterschiede deutlicher werden
        (2.0 * pr * rc / (pr + rc)).powi(2)
    }
}

// ---------------------------------------------------------------------------
// Ablauf

/// Stufe A: Analysebild, Freistellen, feine Linien.
/// `text_boxes`: Rechtecke erkannter Texte (Quellbild), dort keine Linien suchen.
/// Danach kann die App für jede Textzeile [`analyze_text`] aufrufen.
pub fn prepare(doc: &mut Doc, o: &Opts, h: &HandOpts, text_boxes: &[[f64; 4]]) {
    doc.rasterize(o);
    let (w, hh, sc) = (doc.w, doc.h, doc.sc);
    let mut res = HandResult::default();
    if h.bg_cut {
        cut_background(&mut doc.data, w, hh, h.bg_tol);
    }
    let mut keep = vec![false; w * hh];
    for b in text_boxes {
        let (x0, y0) = (((b[0] * sc) as usize).saturating_sub(4), ((b[1] * sc) as usize).saturating_sub(4));
        let (x1, y1) = (((b[2] * sc) as usize + 4).min(w), ((b[3] * sc) as usize + 4).min(hh));
        for y in y0..y1 {
            for x in x0..x1 {
                keep[y * w + x] = true;
            }
        }
    }
    res.erase = vec![false; w * hh];
    if h.lines {
        let (lines, erase) = find_lines(&doc.data, w, hh, sc, &keep);
        res.erase = erase;
        res.lines = lines
            .into_iter()
            .map(|mut l| {
                for p in l.pts.iter_mut() {
                    *p = [p[0] / sc, p[1] / sc];
                }
                l.width /= sc;
                l
            })
            .collect();
    }
    doc.hand = res;
}

/// Stufe B: Text und Linien übermalen, Fotobereiche beruhigen, Farben, Pfade,
/// Text einsetzen, Verläufe. `k` = Farbanzahl (0 = automatisch, etwas reichlicher).
pub fn finish(doc: &mut Doc, o: &Opts, k: usize, h: &HandOpts, texts: &[TextOut]) {
    let (w, hh, sc) = (doc.w, doc.h, doc.sc);
    let n = w * hh;
    let mut erase = std::mem::take(&mut doc.hand.erase);
    if erase.len() != n {
        erase = vec![false; n];
    }
    let mut text_px = vec![false; n];
    for t in texts {
        for &p in &t.erase {
            if p < n {
                erase[p] = true;
                text_px[p] = true;
            }
        }
    }
    for p in 0..n {
        if doc.data[p * 4 + 3] < 128 {
            erase[p] = false;
        }
    }
    let before = doc.data.clone();
    inpaint(&mut doc.data, w, hh, &erase);
    if h.photo {
        doc.hand.photo = calm_photo(&mut doc.data, w, hh, sc);
        // feine Muster (Waben, Raster) auch in ruhigen Flächen glätten,
        // damit z. B. eine Goldfläche nicht in Farbstreifen zerfällt
        let r = ((3.0 * sc).round() as usize).clamp(2, 5);
        let f = vector::bilateral(&doc.data, w, hh, r, 30.0);
        doc.data = vector::bilateral(&f, w, hh, r, 30.0);
    }
    // Vorlage für Verläufe: geglättetes Bild, unter dem Text aber die echte Schriftfarbe
    let mut orig = doc.data.clone();
    for p in 0..n {
        if text_px[p] {
            orig[p * 4..p * 4 + 4].copy_from_slice(&before[p * 4..p * 4 + 4]);
        }
    }
    doc.hand.orig = orig;
    doc.q = vector::quantize(&doc.data, w, hh);
    set_colors(doc, k, h.gradients || h.print);
    doc.hand.saved = texts.to_vec();
    map_and_trace(doc, o, h);
}

/// Zuordnung, ggf. Druckaufbereitung, Pfade, dann Text und Verläufe.
fn map_and_trace(doc: &mut Doc, o: &Opts, h: &HandOpts) {
    doc.hand.print = h.print;
    if h.print {
        limit_colors(doc, h.print_colors);
    }
    doc.apply_map(o);
    if h.print {
        print_fix(doc, h);
    }
    doc.vectorize(o);
    if h.print {
        doc.hand.texts = doc.hand.saved.iter().filter(|t| !t.shapes.is_empty()).count();
        doc.hand.grads.clear();
    } else {
        add_texts_and_gradients(doc, h);
    }
}

/// Farbanzahl auf höchstens n begrenzen: ähnlichste Gruppen zusammenlegen
/// (kleine Gruppen zuerst, gewichtet nach Fläche).
fn limit_colors(doc: &mut Doc, n: usize) {
    let n = n.max(1);
    loop {
        let live: Vec<usize> = (0..doc.groups.len()).filter(|&i| !doc.groups[i].transp).collect();
        if live.len() <= n {
            break;
        }
        let mut best: Option<(f64, usize, usize)> = None;
        for (ia, &a) in live.iter().enumerate() {
            for &b in &live[ia + 1..] {
                let (ca, cb) = (doc.gcol(a).map(|v| v as f64), doc.gcol(b).map(|v| v as f64));
                let wsmall = doc.group_weight(&doc.groups[a]).min(doc.group_weight(&doc.groups[b])) as f64;
                let cost = cdist(ca, cb) * (wsmall + 1.0).sqrt();
                if best.map(|x| cost < x.0).unwrap_or(true) {
                    best = Some((cost, a, b));
                }
            }
        }
        let Some((_, a, b)) = best else { break };
        let src = std::mem::take(&mut doc.groups[b].src);
        doc.groups[a].src.extend(src);
        doc.groups.remove(b);
    }
    doc.k = doc.groups.len();
}

/// Abstand zum nächsten Pixel außerhalb der Maske (Fasen-Abstand 3/4, in Pixeln).
pub(crate) fn dist_in(m: &[bool], w: usize, h: usize) -> Vec<f32> {
    let big = 1e9f32;
    let mut d: Vec<f32> = m.iter().map(|&b| if b { big } else { 0.0 }).collect();
    for y in 0..h {
        for x in 0..w {
            let p = y * w + x;
            if d[p] == 0.0 {
                continue;
            }
            let mut v = d[p];
            if x == 0 || y == 0 {
                v = v.min(1.0);
            }
            if x > 0 {
                v = v.min(d[p - 1] + 1.0);
            }
            if y > 0 {
                v = v.min(d[p - w] + 1.0);
                if x > 0 {
                    v = v.min(d[p - w - 1] + 1.4142);
                }
                if x + 1 < w {
                    v = v.min(d[p - w + 1] + 1.4142);
                }
            }
            d[p] = v;
        }
    }
    for y in (0..h).rev() {
        for x in (0..w).rev() {
            let p = y * w + x;
            if d[p] == 0.0 {
                continue;
            }
            let mut v = d[p];
            if x + 1 == w || y + 1 == h {
                v = v.min(1.0);
            }
            if x + 1 < w {
                v = v.min(d[p + 1] + 1.0);
            }
            if y + 1 < h {
                v = v.min(d[p + w] + 1.0);
                if x + 1 < w {
                    v = v.min(d[p + w + 1] + 1.4142);
                }
                if x > 0 {
                    v = v.min(d[p + w - 1] + 1.4142);
                }
            }
            d[p] = v;
        }
    }
    d
}

/// Öffnen mit rundem Werkzeug: entfernt alles schmaler als 2r und rundet Spitzen.
fn open_round(m: &[bool], w: usize, h: usize, r: f32) -> Vec<bool> {
    let d = dist_in(m, w, h);
    let core: Vec<bool> = d.iter().map(|&v| v > r).collect();
    let inv: Vec<bool> = core.iter().map(|&b| !b).collect();
    let dc = dist_in(&inv, w, h);
    (0..w * h).map(|p| m[p] && (core[p] || dc[p] <= r + 0.5)).collect()
}

fn dilate_round(m: &[bool], w: usize, h: usize, r: f32) -> Vec<bool> {
    let inv: Vec<bool> = m.iter().map(|&b| !b).collect();
    let d = dist_in(&inv, w, h);
    (0..w * h).map(|p| m[p] || d[p] <= r).collect()
}

/// Aufbereitung für den 3D-Druck: Linien und Text als Flächen mit
/// Mindeststärke einbrennen, dann jede Farbe rund öffnen – schmale Streifen,
/// Splitter und Spitzen verschwinden, die Lücken füllt die Nachbarfarbe.
fn print_fix(doc: &mut Doc, h: &HandOpts) {
    let (w, hh, sc) = (doc.w, doc.h, doc.sc);
    let n = w * hh;
    if doc.g.len() != n || n == 0 {
        return;
    }
    // Analysepixel je mm
    let ppm = w as f64 / h.print_width_mm.max(1.0);
    let rmin = (h.print_min_mm * ppm / 2.0).max(0.6) as f32;
    // Linien einbrennen (mindestens so breit wie erlaubt)
    let lines = std::mem::take(&mut doc.hand.lines);
    // eingebrannte Linien und Schrift haben Vorrang vor dem Verbreitern anderer Flächen
    let mut protect = vec![false; n];
    if std::env::var_os("ZMN_DEBUG").is_some() {
        eprintln!("    Druck: {} Linien einbrennen, rmin {:.1} px", lines.len(), rmin);
    }
    for ln in &lines {
        let pts: Vec<Pt> = ln.pts.iter().map(|q| [q[0] * sc, q[1] * sc]).collect();
        let mut cov = vec![0f32; n];
        stroke_line(&mut cov, w, hh, &pts, ln.closed, (ln.width * sc).max(2.0 * rmin as f64 + 1.0), 1);
        // Untergrund der Linie (häufigste Farbe im Saum) – die Linie darf nicht
        // dieselbe Farbe bekommen, sonst verschwindet sie
        let mut ring = vec![0f32; n];
        stroke_line(&mut ring, w, hh, &pts, ln.closed, (ln.width * sc).max(2.0 * rmin as f64 + 1.0) + 6.0, 1);
        let mut under: std::collections::HashMap<i16, usize> = std::collections::HashMap::new();
        for p in 0..n {
            if ring[p] >= 0.5 && cov[p] < 0.5 {
                *under.entry(doc.g[p]).or_insert(0) += 1;
            }
        }
        let bg = under.into_iter().max_by_key(|x| x.1).map(|x| x.0).unwrap_or(-1);
        let mut gi = nearest_group_ex(doc, ln.color, false) as i16;
        if gi == bg {
            // zweitnächste Farbe nehmen
            let cf = ln.color.map(|v| v as f64);
            let mut best = (f64::MAX, gi);
            for g in 0..doc.groups.len() {
                if g as i16 == bg || doc.groups[g].transp {
                    continue;
                }
                let dd = cdist(cf, doc.gcol(g).map(|v| v as f64));
                if dd < best.0 {
                    best = (dd, g as i16);
                }
            }
            gi = best.1;
        }
        if std::env::var_os("ZMN_DEBUG").is_some() {
            eprintln!("      Linie {} -> Gruppe {} {} bei {:?} breite {:.1}", vector::hex(ln.color), gi, vector::hex(doc.gcol(gi as usize)), ln.pts.first().map(|p| (p[0].round(), p[1].round())), ln.width);
        }
        for p in 0..n {
            if cov[p] >= 0.5 && doc.g[p] >= 0 {
                doc.g[p] = gi;
                protect[p] = true;
            }
        }
    }
    // Text einbrennen, dünne Striche verstärken
    let saved = doc.hand.saved.clone();
    for t in &saved {
        if t.shapes.is_empty() {
            continue;
        }
        let polys: Vec<Vec<Pt>> = t.shapes.iter().flat_map(|sp| flatten(sp, 0.4)).map(|p| p.into_iter().map(|q| [q[0] * sc, q[1] * sc]).collect()).collect();
        let mut cov = vec![0f32; n];
        fill_evenodd(&mut cov, w, hh, &polys, 2);
        let m2: Vec<bool> = cov.iter().map(|&v| v >= 0.5).collect();
        let gi = nearest_group_ex(doc, t.color, false) as i16;
        for p in 0..n {
            if m2[p] && doc.g[p] >= 0 {
                doc.g[p] = gi;
                protect[p] = true;
            }
        }
    }
    let dbg = std::env::var_os("ZMN_DEBUG").is_some();
    let count = |g: &[i16]| { let mut c = std::collections::BTreeMap::new(); for &v in g { *c.entry(v).or_insert(0usize) += 1; } c };
    if dbg { eprintln!("    nach Einbrennen: {:?}", count(&doc.g)); }
    // jede Farbe (auch transparent) rund öffnen
    for _pass in 0..2 {
        let mut labels: Vec<i16> = doc.g.clone();
        labels.sort();
        labels.dedup();
        let mut orphan = vec![false; n];
        // lange, schmale Teile (Linien, dünne Schrift, Rahmen) werden verbreitert
        // statt entfernt; kurze Splitter und Spitzen gehen an die Nachbarfarbe
        let mut thicken: Vec<(Vec<usize>, i16, f32)> = Vec::new();
        for &lab in &labels {
            let m: Vec<bool> = doc.g.iter().map(|&v| v == lab).collect();
            let keep = open_round(&m, w, hh, rmin);
            let rem: Vec<bool> = (0..n).map(|p| m[p] && !keep[p]).collect();
            if !rem.iter().any(|&b| b) {
                continue;
            }
            let din = dist_in(&m, w, hh);
            let mut seen = vec![false; n];
            for st in 0..n {
                if !rem[st] || seen[st] {
                    continue;
                }
                let mut comp = vec![st];
                seen[st] = true;
                let mut i = 0;
                while i < comp.len() {
                    let p = comp[i];
                    i += 1;
                    let (x, y) = (p % w, p / w);
                    for (dx, dy) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1), (-1, -1), (1, 1), (-1, 1), (1, -1)] {
                        let (xx, yy) = (x as i64 + dx, y as i64 + dy);
                        if xx < 0 || yy < 0 || xx >= w as i64 || yy >= hh as i64 {
                            continue;
                        }
                        let q = yy as usize * w + xx as usize;
                        if rem[q] && !seen[q] {
                            seen[q] = true;
                            comp.push(q);
                        }
                    }
                }
                let half = comp.iter().map(|&p| din[p]).fold(0.0f32, f32::max).max(0.5);
                let len = comp.len() as f32 / (2.0 * half);
                // durchsichtige Teile werden nie verbreitert (nur Löcher füllen)
                if lab >= 0 && len >= 6.0 * rmin {
                    thicken.push((comp.clone(), lab, (rmin - half + 0.7).max(0.0)));
                }
                for p in comp {
                    orphan[p] = true;
                }
            }
        }
        if !orphan.iter().any(|&b| b) {
            break;
        }
        // Waisen von außen nach innen mit der häufigsten Nachbarfarbe füllen
        let mut guard = 0;
        while orphan.iter().any(|&b| b) && guard < 200 {
            guard += 1;
            let mut fill = Vec::new();
            for y in 0..hh {
                for x in 0..w {
                    let p = y * w + x;
                    if !orphan[p] {
                        continue;
                    }
                    let mut cnt: Vec<(i16, u8)> = Vec::new();
                    for (dx, dy) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1), (-1, -1), (1, 1), (-1, 1), (1, -1)] {
                        let (xx, yy) = (x as i64 + dx, y as i64 + dy);
                        if xx < 0 || yy < 0 || xx >= w as i64 || yy >= hh as i64 {
                            continue;
                        }
                        let q = yy as usize * w + xx as usize;
                        if orphan[q] {
                            continue;
                        }
                        match cnt.iter_mut().find(|c| c.0 == doc.g[q]) {
                            Some(c) => c.1 += 1,
                            None => cnt.push((doc.g[q], 1)),
                        }
                    }
                    if let Some(&(v, _)) = cnt.iter().max_by_key(|c| c.1) {
                        fill.push((p, v));
                    }
                }
            }
            if fill.is_empty() {
                break;
            }
            for (p, v) in fill {
                doc.g[p] = v;
                orphan[p] = false;
            }
        }
        if dbg { eprintln!("    nach Öffnen: {:?}, {} verbreitert", count(&doc.g), thicken.len()); }
        // verbreiterte Teile wieder einsetzen
        for (comp, lab, grow) in thicken {
            let mut m = vec![false; n];
            for p in comp {
                m[p] = true;
            }
            let m = dilate_round(&m, w, hh, grow);
            for p in 0..n {
                if m[p] && doc.g[p] >= 0 && (!protect[p] || doc.g[p] == lab) {
                    doc.g[p] = lab;
                }
            }
        }
    }
    doc.hand.lines.clear();
    if let Some(dir) = std::env::var_os("ZMN_DEBUG_DIR") {
        let mut pgm = format!("P5 {w} {hh} 255\n").into_bytes();
        pgm.extend(doc.g.iter().map(|&v| if v < 0 { 0u8 } else { 60 + (v as u8).wrapping_mul(60) }));
        let _ = std::fs::write(std::path::Path::new(&dir).join("druck-g.pgm"), pgm);
    }
}

/// Farbanzahl setzen (0 = automatisch, für Logos etwas reichlicher).
fn set_colors(doc: &mut Doc, k: usize, merge: bool) {
    let kk = if k == 0 { (doc.q.auto + 4).min(doc.q.pal.len()).max(1) } else { k };
    doc.set_preset(kk);
    // mit Verläufen: nahe Schattierungen derselben Farbe zusammenlegen – der
    // Verlauf zeichnet die Schattierung dann weich nach statt in Streifen
    if merge && k == 0 {
        loop {
            let cols: Vec<[f64; 3]> = (0..doc.groups.len()).map(|i| doc.gcol(i).map(|v| v as f64)).collect();
            let mut best: Option<(f64, usize, usize)> = None;
            for a in 0..cols.len() {
                for b in a + 1..cols.len() {
                    let dd = cdist(cols[a], cols[b]);
                    if dd < 36.0 && best.map(|x| dd < x.0).unwrap_or(true) {
                        best = Some((dd, a, b));
                    }
                }
            }
            let Some((_, a, b)) = best else { break };
            let src = std::mem::take(&mut doc.groups[b].src);
            doc.groups[a].src.extend(src);
            doc.groups.remove(b);
        }
        doc.k = doc.groups.len();
    }
    doc.hand.k = doc.k;
}

fn add_texts_and_gradients(doc: &mut Doc, h: &HandOpts) {
    let texts = doc.hand.saved.clone();
    let mut ntext = 0;
    for t in &texts {
        if t.shapes.is_empty() {
            continue;
        }
        let gi = nearest_group(doc, t.color);
        doc.layers.push(Layer { idx: gi, subs: t.shapes.clone() });
        ntext += 1;
    }
    doc.hand.texts = ntext;
    doc.hand.grads.clear();
    if h.gradients {
        find_gradients(doc);
    }
}

/// Nach einer Änderung in der Oberfläche neu berechnen (Stufe wie bei
/// [`Doc::run`]: 0 = alles, 1 = Farben, 2 = Zuordnung, 3 = Pfade).
/// Texterkennung und Schriftwahl werden wiederverwendet.
pub fn refresh(doc: &mut Doc, o: &Opts, h: &HandOpts, k: usize, stage: u8) {
    if stage == 0 {
        let ocr = std::mem::take(&mut doc.hand.ocr);
        let saved = std::mem::take(&mut doc.hand.saved);
        let boxes: Vec<[f64; 4]> = ocr.iter().map(|l| l.bbox).collect();
        prepare(doc, o, h, &boxes);
        let texts: Vec<TextOut> = if h.text {
            saved
                .into_iter()
                .map(|mut t| {
                    t.erase = analyze_text(doc, &t.line).map(|a| a.erase).unwrap_or_default();
                    t
                })
                .collect()
        } else {
            Vec::new()
        };
        finish(doc, o, k, h, &texts);
        doc.hand.ocr = ocr;
        return;
    }
    if stage <= 1 {
        doc.q = vector::quantize(&doc.data, doc.w, doc.h);
        set_colors(doc, k, h.gradients || h.print);
    }
    if stage <= 2 || h.print {
        // Druckaufbereitung braucht die Zuordnung (und die ursprünglichen Linien)
        if h.print && doc.hand.lines.is_empty() && stage > 0 {
            // Linien wurden beim letzten Mal eingebrannt – bleiben über die Zuordnung erhalten
        }
        map_and_trace(doc, o, h);
        return;
    }
    doc.vectorize(o);
    add_texts_and_gradients(doc, h);
}

/// Passende Farbgruppe für eine Farbe (oder neue Gruppe mit fester Farbe).
fn nearest_group(doc: &mut Doc, c: [u8; 3]) -> usize {
    nearest_group_ex(doc, c, true)
}

fn nearest_group_ex(doc: &mut Doc, c: [u8; 3], allow_new: bool) -> usize {
    let cf = [c[0] as f64, c[1] as f64, c[2] as f64];
    let mut best = (f64::MAX, 0usize);
    for gi in 0..doc.groups.len() {
        if doc.groups[gi].transp {
            continue;
        }
        let g = doc.gcol(gi);
        let dd = cdist(cf, [g[0] as f64, g[1] as f64, g[2] as f64]);
        if dd < best.0 {
            best = (dd, gi);
        }
    }
    if best.0 < 14.0 || !allow_new {
        return best.1;
    }
    doc.groups.push(vector::Group { src: Vec::new(), over: Some(c), transp: false });
    doc.groups.len() - 1
}

/// Für jede größere Teilform prüfen, ob ein Farbverlauf sie besser beschreibt als eine Farbe.
pub fn find_gradients(doc: &mut Doc) {
    let (w, h, sc) = (doc.w, doc.h, doc.sc);
    let img: &[u8] = if doc.hand.orig.len() == w * h * 4 { &doc.hand.orig } else { &doc.data };
    let min_px = ((w * h) as f64 * 0.0004).max(120.0) as usize;
    let mut out = Vec::new();
    for (li, l) in doc.layers.iter().enumerate() {
        for (si, sp) in l.subs.iter().enumerate() {
            let polys: Vec<Vec<Pt>> = flatten(sp, 0.5).into_iter().map(|p| p.into_iter().map(|q| [q[0] * sc, q[1] * sc]).collect()).collect();
            let (mut bx0, mut by0, mut bx1, mut by1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
            for p in polys.iter().flatten() {
                bx0 = bx0.min(p[0]);
                by0 = by0.min(p[1]);
                bx1 = bx1.max(p[0]);
                by1 = by1.max(p[1]);
            }
            if bx1 - bx0 < 6.0 || by1 - by0 < 6.0 {
                continue;
            }
            let (ox, oy) = (bx0.floor().max(0.0) as usize, by0.floor().max(0.0) as usize);
            let (ex, ey) = ((bx1.ceil() as usize + 1).min(w), (by1.ceil() as usize + 1).min(h));
            if ex <= ox || ey <= oy {
                continue;
            }
            let (cw, ch) = (ex - ox, ey - oy);
            let local: Vec<Vec<Pt>> = polys.iter().map(|p| p.iter().map(|q| [q[0] - ox as f64, q[1] - oy as f64]).collect()).collect();
            let mut cov = vec![0f32; cw * ch];
            fill_evenodd(&mut cov, cw, ch, &local, 1);
            // Pixel sammeln, Kantensaum auslassen
            let mut pts: Vec<(f64, f64, [f64; 3])> = Vec::new();
            for y in 1..ch.saturating_sub(1) {
                for x in 1..cw.saturating_sub(1) {
                    let q = y * cw + x;
                    if cov[q] < 0.5 || cov[q - 1] < 0.5 || cov[q + 1] < 0.5 || cov[q - cw] < 0.5 || cov[q + cw] < 0.5 {
                        continue;
                    }
                    let p = (oy + y) * w + ox + x;
                    if img[p * 4 + 3] < 128 {
                        continue;
                    }
                    pts.push(((ox + x) as f64, (oy + y) as f64, px(img, p)));
                }
            }
            if pts.len() < min_px {
                continue;
            }
            if let Some(g) = fit_gradient(&pts) {
                out.push(Grad { layer: li, sub: si, p1: [g.0[0] / sc, g.0[1] / sc], p2: [g.1[0] / sc, g.1[1] / sc], stops: g.2 });
            }
        }
    }
    doc.hand.grads = out;
}

type GradFit = (Pt, Pt, Vec<(f64, [u8; 3])>);

fn fit_gradient(pts: &[(f64, f64, [f64; 3])]) -> Option<GradFit> {
    let n = pts.len() as f64;
    let (mx, my) = (pts.iter().map(|p| p.0).sum::<f64>() / n, pts.iter().map(|p| p.1).sum::<f64>() / n);
    let mut mc = [0f64; 3];
    for p in pts {
        for k in 0..3 {
            mc[k] += p.2[k] / n;
        }
    }
    // lineares Modell je Kanal: c = mc + bx*(x-mx) + by*(y-my)
    let (mut sxx, mut syy, mut sxy) = (0.0, 0.0, 0.0);
    let mut sxc = [0f64; 3];
    let mut syc = [0f64; 3];
    let mut var = 0.0;
    for p in pts {
        let (u, v) = (p.0 - mx, p.1 - my);
        sxx += u * u;
        syy += v * v;
        sxy += u * v;
        for k in 0..3 {
            let c = p.2[k] - mc[k];
            sxc[k] += u * c;
            syc[k] += v * c;
            var += c * c;
        }
    }
    let det = sxx * syy - sxy * sxy;
    if det.abs() < 1e-6 || var / n < 9.0 {
        return None; // praktisch einfarbig
    }
    let mut m = [[0f64; 2]; 2];
    for k in 0..3 {
        let bx = (sxc[k] * syy - syc[k] * sxy) / det;
        let by = (sxx * syc[k] - sxy * sxc[k]) / det;
        m[0][0] += bx * bx;
        m[0][1] += bx * by;
        m[1][1] += by * by;
    }
    // Hauptrichtung (größter Eigenvektor)
    let (a, b, c) = (m[0][0], m[0][1], m[1][1]);
    let tr = a + c;
    let disc = ((a - c) * (a - c) / 4.0 + b * b).sqrt();
    let l1 = tr / 2.0 + disc;
    let mut dir = if b.abs() > 1e-12 { [l1 - c, b] } else if a >= c { [1.0, 0.0] } else { [0.0, 1.0] };
    let dl = (dir[0] * dir[0] + dir[1] * dir[1]).sqrt();
    if dl < 1e-12 {
        return None;
    }
    dir = [dir[0] / dl, dir[1] / dl];
    let mut ts: Vec<f64> = pts.iter().map(|p| (p.0 - mx) * dir[0] + (p.1 - my) * dir[1]).collect();
    let mut sorted = ts.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let t0 = sorted[(sorted.len() as f64 * 0.02) as usize];
    let t1 = sorted[((sorted.len() as f64 * 0.98) as usize).min(sorted.len() - 1)];
    if t1 - t0 < 8.0 {
        return None;
    }
    let nb = 7usize;
    let mut acc = vec![[0f64; 4]; nb];
    for (i, p) in pts.iter().enumerate() {
        let f = ((ts[i] - t0) / (t1 - t0)).clamp(0.0, 0.9999);
        let bi = (f * nb as f64) as usize;
        for k in 0..3 {
            acc[bi][k] += p.2[k];
        }
        acc[bi][3] += 1.0;
    }
    let mut stops: Vec<(f64, [f64; 3])> = Vec::new();
    for (bi, a) in acc.iter().enumerate() {
        if a[3] < 8.0 {
            continue;
        }
        stops.push(((bi as f64 + 0.5) / nb as f64, [a[0] / a[3], a[1] / a[3], a[2] / a[3]]));
    }
    if stops.len() < 2 {
        return None;
    }
    // Fehler: flach gegen Verlauf
    let model = |f: f64| -> [f64; 3] {
        if f <= stops[0].0 {
            return stops[0].1;
        }
        for s in stops.windows(2) {
            if f <= s[1].0 {
                let k = (f - s[0].0) / (s[1].0 - s[0].0);
                return [0, 1, 2].map(|j| s[0].1[j] + (s[1].1[j] - s[0].1[j]) * k);
            }
        }
        stops[stops.len() - 1].1
    };
    let (mut ef, mut eg) = (0.0, 0.0);
    for (i, p) in pts.iter().enumerate() {
        let f = ((ts[i] - t0) / (t1 - t0)).clamp(0.0, 1.0);
        ef += cdist(p.2, mc);
        eg += cdist(p.2, model(f));
    }
    let spread = stops.iter().map(|s| cdist(s.1, stops[0].1)).fold(0.0, f64::max);
    if eg > ef * 0.72 || spread < 10.0 {
        return None;
    }
    ts.clear();
    // überflüssige Zwischenstopps entfernen (fast auf der Geraden)
    let mut keep: Vec<(f64, [f64; 3])> = vec![stops[0]];
    for i in 1..stops.len() - 1 {
        let (a, b, c) = (keep[keep.len() - 1], stops[i], stops[i + 1]);
        let k = (b.0 - a.0) / (c.0 - a.0);
        let lin = [0, 1, 2].map(|j| a.1[j] + (c.1[j] - a.1[j]) * k);
        if cdist(lin, b.1) > 3.0 {
            keep.push(b);
        }
    }
    keep.push(stops[stops.len() - 1]);
    let p1 = [mx + dir[0] * t0, my + dir[1] * t0];
    let p2 = [mx + dir[0] * t1, my + dir[1] * t1];
    Some((p1, p2, keep.into_iter().map(|(f, c)| (f, [c[0].round() as u8, c[1].round() as u8, c[2].round() as u8])).collect()))
}

/// Farbe eines Verlaufs an einer Stelle (Quellbild-Koordinaten).
pub fn grad_color(g: &Grad, p: Pt) -> [u8; 3] {
    let (dx, dy) = (g.p2[0] - g.p1[0], g.p2[1] - g.p1[1]);
    let l2 = (dx * dx + dy * dy).max(1e-9);
    let f = (((p[0] - g.p1[0]) * dx + (p[1] - g.p1[1]) * dy) / l2).clamp(0.0, 1.0);
    let s = &g.stops;
    if f <= s[0].0 {
        return s[0].1;
    }
    for i in 0..s.len() - 1 {
        if f <= s[i + 1].0 {
            let k = (f - s[i].0) / (s[i + 1].0 - s[i].0).max(1e-9);
            return [0, 1, 2].map(|j| (s[i].1[j] as f64 + (s[i + 1].1[j] as f64 - s[i].1[j] as f64) * k).round() as u8);
        }
    }
    s[s.len() - 1].1
}

/// Offene oder geschlossene Linie mit runden Enden als Fläche (für Vorschau und PNG).
pub fn stroke_line(cov: &mut [f32], w: usize, h: usize, p: &[Pt], closed: bool, bw: f64, ss: usize) {
    let r = bw / 2.0;
    if r <= 0.0 || p.len() < 2 {
        return;
    }
    let n = if closed { p.len() } else { p.len() - 1 };
    for i in 0..n {
        let (a, b) = (p[i], p[(i + 1) % p.len()]);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let l = (dx * dx + dy * dy).sqrt();
        if l < 1e-9 {
            continue;
        }
        let nn = [-dy / l * r, dx / l * r];
        let quad = vec![[a[0] + nn[0], a[1] + nn[1]], [b[0] + nn[0], b[1] + nn[1]], [b[0] - nn[0], b[1] - nn[1]], [a[0] - nn[0], a[1] - nn[1]]];
        fill_evenodd(cov, w, h, &[quad], ss);
        if r > 0.6 {
            let oct: Vec<Pt> = (0..10).map(|k| {
                let t = k as f64 / 10.0 * std::f64::consts::TAU;
                [b[0] + r * t.cos(), b[1] + r * t.sin()]
            }).collect();
            fill_evenodd(cov, w, h, &[oct], ss);
        }
    }
}

/// Umriss einer Glyphe (Befehle) in eine Form verwandeln.
pub fn shape_from_cmds(c: Vec<Cmd>) -> Shape {
    Shape::Path(c)
}
