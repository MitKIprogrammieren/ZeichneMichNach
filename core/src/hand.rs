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
}

impl Default for HandOpts {
    fn default() -> Self {
        HandOpts { bg_cut: true, bg_tol: 28.0, lines: true, photo: true, gradients: true, text: true }
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

/// Feine helle oder dunkle Linien finden. Rückgabe: Linien (Analysepixel) und Maske zum Übermalen.
pub fn find_lines(d: &[u8], w: usize, h: usize, sc: f64, keep_out: &[bool]) -> (Vec<LineOut>, Vec<bool>) {
    let n = w * h;
    let mut erase = vec![false; n];
    let mut lines = Vec::new();
    if n == 0 {
        return (lines, erase);
    }
    let l: Vec<f32> = (0..n).map(|p| if d[p * 4 + 3] < 128 { 0.0 } else { lum(d, p) as f32 }).collect();
    let r = ((3.0 * sc).round() as usize).clamp(2, 6);
    let open = morph(&morph(&l, w, h, r, false), w, h, r, true);
    let close = morph(&morph(&l, w, h, r, true), w, h, r, false);
    let minlen = (w.min(h) as f64 * 0.05).max(30.0);
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
        prune(&mut m, w, h, (9.0 * sc.max(0.7)) as usize);
        for ch in chains(&m, w, h) {
            let pts: Vec<Pt> = ch.iter().map(|&p| [(p % w) as f64 + 0.5, (p / w) as f64 + 0.5]).collect();
            let len: f64 = pts.windows(2).map(|s| ((s[1][0] - s[0][0]).powi(2) + (s[1][1] - s[0][1]).powi(2)).sqrt()).sum();
            if len < minlen {
                continue;
            }
            let closed = ch.first() == ch.last();
            let simple = rdp(&pts, 1.3 * sc.max(0.7));
            let nseg = simple.len().saturating_sub(1).max(1);
            let straightish = len / nseg as f64 >= 22.0 * sc.max(0.7);
            let circ = fit_circle(&pts).filter(|c| c.2 < 1.1 * sc.max(0.7) && c.1 > w.min(h) as f64 * 0.025);
            if !straightish && circ.is_none() {
                continue;
            }
            // Breite: Maskenpixel in der Nähe der Kette / Länge
            let rr = r as i64 + 1;
            let mut near = std::collections::HashSet::new();
            let mut col = [0f64; 3];
            for &p in &ch {
                let c = px(d, p);
                for k in 0..3 {
                    col[k] += c[k];
                }
                let (x, y) = ((p % w) as i64, (p / w) as i64);
                for dy in -rr..=rr {
                    for dx in -rr..=rr {
                        let (xx, yy) = (x + dx, y + dy);
                        if xx >= 0 && yy >= 0 && xx < w as i64 && yy < h as i64 {
                            let q = yy as usize * w + xx as usize;
                            if raw[q] {
                                near.insert(q);
                            }
                        }
                    }
                }
            }
            let width = (near.len() as f64 / len.max(1.0)).clamp(1.0, 2.0 * r as f64);
            let cn = ch.len() as f64;
            let color = [(col[0] / cn) as u8, (col[1] / cn) as u8, (col[2] / cn) as u8];
            for q in near {
                erase[q] = true;
            }
            let out_pts = if let (Some((c, rad, _)), true) = (circ, closed) {
                (0..72).map(|k| {
                    let t = k as f64 / 72.0 * std::f64::consts::TAU;
                    [c[0] + rad * t.cos(), c[1] + rad * t.sin()]
                }).collect()
            } else if straightish {
                simple
            } else {
                rdp(&pts, 0.6)
            };
            lines.push(LineOut { pts: out_pts, closed: closed && circ.is_some(), width, color });
        }
    }
    let erase = morph_mask(&erase, w, h, 1, true);
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
    /// Maske der ganzen Zeile (Analysepixel, global)
    pub erase: Vec<usize>,
}

/// Fertige Schrift einer Zeile (Umrisse im Quellbild).
#[derive(Clone, Debug)]
pub struct TextOut {
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
    // Splitter entfernen
    {
        let mut seen = vec![false; bw * bh];
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
            if comp.len() < 6 || (touches && comp.len() > bw * bh / 4) {
                for p in comp {
                    m[p] = false;
                }
            }
        }
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
    // zu viele Abschnitte: kleinste Lücken schließen
    while runs.len() > chars.len() && runs.len() > 1 {
        let (i, _) = runs.windows(2).enumerate().map(|(i, r)| (i, r[1].0 - r[0].1)).min_by_key(|x| x.1).unwrap();
        runs[i].1 = runs[i + 1].1;
        runs.remove(i + 1);
    }
    let per_char = runs.len() == chars.len();
    if !per_char {
        runs = vec![(runs[0].0, runs[runs.len() - 1].1)];
    }
    let mut segs = Vec::new();
    for (ri, &(a, b)) in runs.iter().enumerate() {
        let (mut ya, mut yb) = (bh, 0);
        for y in 0..bh {
            if (a..b).any(|x| m[y * bw + x]) {
                ya = ya.min(y);
                yb = yb.max(y + 1);
            }
        }
        if yb <= ya {
            continue;
        }
        let (mw, mh) = (b - a, yb - ya);
        let mut mask = vec![false; mw * mh];
        for y in 0..mh {
            for x in 0..mw {
                mask[y * mw + x] = m[(ya + y) * bw + a + x];
            }
        }
        let ink = [(x0 + a) as f64 / sc, (y0 + ya) as f64 / sc, (x0 + b) as f64 / sc, (y0 + yb) as f64 / sc];
        let ch = if per_char { chars[ri].to_string() } else { chars.iter().collect() };
        segs.push(TextSeg { chars: ch, ink, mw, mh, mask });
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
        erase,
    })
}

/// Polygone (in Maskenpixeln) rastern und mit der Maske vergleichen (Schnitt/Vereinigung).
pub fn mask_iou(mask: &[bool], mw: usize, mh: usize, polys: &[Vec<Pt>]) -> f64 {
    let mut cov = vec![0f32; mw * mh];
    fill_evenodd(&mut cov, mw, mh, polys, 2);
    let (mut i, mut u) = (0usize, 0usize);
    for p in 0..mw * mh {
        let a = mask[p];
        let b = cov[p] >= 0.5;
        if a && b {
            i += 1;
        }
        if a || b {
            u += 1;
        }
    }
    if u == 0 {
        0.0
    } else {
        i as f64 / u as f64
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
    let kk = if k == 0 { (doc.q.auto + 4).min(doc.q.pal.len()).max(1) } else { k };
    doc.set_preset(kk);
    doc.apply_map(o);
    doc.vectorize(o);
    let mut ntext = 0;
    for t in texts {
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
    doc.hand.orig = Vec::new();
}

/// Passende Farbgruppe für eine Farbe (oder neue Gruppe mit fester Farbe).
fn nearest_group(doc: &mut Doc, c: [u8; 3]) -> usize {
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
    if best.0 < 14.0 {
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
