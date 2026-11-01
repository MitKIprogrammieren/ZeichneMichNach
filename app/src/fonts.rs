//! Passende Schrift für erkannten Text finden: alle Schriften des Rechners
//! (plus Inter) werden Zeichen für Zeichen auf die Tinte im Bild gelegt und
//! verglichen. Die beste Familie gewinnt, je Zeichen der beste Schnitt (so
//! werden z. B. „KADER“ fett und „SCHMIEDE“ dünn erkannt).

use nz_core::hand::{mask_iou, TextAnalysis, TextOut};
use nz_core::vector::{Cmd, Pt, Shape};
use std::path::PathBuf;

/// Ergebnis für eine Zeile (für die Anzeige).
#[derive(Clone, Debug)]
pub struct Match {
    pub text: String,
    pub family: String,
    pub styles: Vec<String>,
    pub score: f64,
}

struct Builder {
    cmds: Vec<(char, [f32; 6])>,
}

impl ttf_parser::OutlineBuilder for Builder {
    fn move_to(&mut self, x: f32, y: f32) {
        self.cmds.push(('M', [x, y, 0.0, 0.0, 0.0, 0.0]));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.cmds.push(('L', [x, y, 0.0, 0.0, 0.0, 0.0]));
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.cmds.push(('Q', [x1, y1, x, y, 0.0, 0.0]));
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.cmds.push(('C', [x1, y1, x2, y2, x, y]));
    }
    fn close(&mut self) {
        self.cmds.push(('Z', [0.0; 6]));
    }
}

/// Umriss eines Zeichens in Schrifteinheiten samt Tintenrechteck.
#[derive(Clone)]
struct Glyph {
    cmds: Vec<(char, [f32; 6])>,
    bb: [f32; 4],
}

fn glyph(face: &ttf_parser::Face, ch: char) -> Option<Glyph> {
    let id = face.glyph_index(ch)?;
    let mut b = Builder { cmds: Vec::new() };
    let r = face.outline_glyph(id, &mut b)?;
    if r.width() <= 0 || r.height() <= 0 {
        return None;
    }
    Some(Glyph { cmds: b.cmds, bb: [r.x_min as f32, r.y_min as f32, r.x_max as f32, r.y_max as f32] })
}

/// Ganze Zeichenkette als ein Umriss (Laufweite aus der Schrift).
fn glyph_run(face: &ttf_parser::Face, text: &str) -> Option<Glyph> {
    let mut cmds = Vec::new();
    let mut x = 0f32;
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    let mut any = false;
    for ch in text.chars() {
        let id = face.glyph_index(ch);
        let adv = id.and_then(|i| face.glyph_hor_advance(i)).unwrap_or((face.units_per_em() / 3) as u16) as f32;
        if let Some(id) = id {
            let mut b = Builder { cmds: Vec::new() };
            if let Some(r) = face.outline_glyph(id, &mut b) {
                any = true;
                x0 = x0.min(x + r.x_min as f32);
                x1 = x1.max(x + r.x_max as f32);
                y0 = y0.min(r.y_min as f32);
                y1 = y1.max(r.y_max as f32);
                for (c, mut v) in b.cmds {
                    for k in (0..6).step_by(2) {
                        if c != 'Z' {
                            v[k] += x;
                        }
                    }
                    cmds.push((c, v));
                }
            }
        } else if !ch.is_whitespace() {
            return None;
        }
        x += adv;
    }
    if !any {
        return None;
    }
    Some(Glyph { cmds, bb: [x0, y0, x1, y1] })
}

/// Umriss auf ein Zielrechteck abbilden (x0,y0 oben links; Breite, Höhe) – y wird gespiegelt.
fn map_pt(g: &Glyph, x: f32, y: f32, r: [f64; 4]) -> Pt {
    let sx = r[2] / (g.bb[2] - g.bb[0]).max(1e-3) as f64;
    let sy = r[3] / (g.bb[3] - g.bb[1]).max(1e-3) as f64;
    [r[0] + (x - g.bb[0]) as f64 * sx, r[1] + (g.bb[3] - y) as f64 * sy]
}

fn polys(g: &Glyph, r: [f64; 4]) -> Vec<Vec<Pt>> {
    let mut out: Vec<Vec<Pt>> = Vec::new();
    let mut cur: Vec<Pt> = Vec::new();
    let mut last: Pt = [0.0, 0.0];
    for (c, v) in &g.cmds {
        match c {
            'M' => {
                if cur.len() > 2 {
                    out.push(std::mem::take(&mut cur));
                }
                cur.clear();
                last = map_pt(g, v[0], v[1], r);
                cur.push(last);
            }
            'L' => {
                last = map_pt(g, v[0], v[1], r);
                cur.push(last);
            }
            'Q' => {
                let (c1, p) = (map_pt(g, v[0], v[1], r), map_pt(g, v[2], v[3], r));
                for i in 1..=6 {
                    let t = i as f64 / 6.0;
                    let u = 1.0 - t;
                    cur.push([u * u * last[0] + 2.0 * u * t * c1[0] + t * t * p[0], u * u * last[1] + 2.0 * u * t * c1[1] + t * t * p[1]]);
                }
                last = p;
            }
            'C' => {
                let (c1, c2, p) = (map_pt(g, v[0], v[1], r), map_pt(g, v[2], v[3], r), map_pt(g, v[4], v[5], r));
                for i in 1..=8 {
                    let t = i as f64 / 8.0;
                    let u = 1.0 - t;
                    let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                    cur.push([a * last[0] + b * c1[0] + c * c2[0] + d * p[0], a * last[1] + b * c1[1] + c * c2[1] + d * p[1]]);
                }
                last = p;
            }
            _ => {
                if cur.len() > 2 {
                    out.push(std::mem::take(&mut cur));
                }
                cur.clear();
            }
        }
    }
    if cur.len() > 2 {
        out.push(cur);
    }
    out
}

fn to_shape(g: &Glyph, r: [f64; 4]) -> Shape {
    let mut out = Vec::new();
    for (c, v) in &g.cmds {
        match c {
            'M' => {
                let p = map_pt(g, v[0], v[1], r);
                out.push(Cmd::M(p[0], p[1]));
            }
            'L' => {
                let p = map_pt(g, v[0], v[1], r);
                out.push(Cmd::L(p[0], p[1]));
            }
            'Q' => {
                let (a, p) = (map_pt(g, v[0], v[1], r), map_pt(g, v[2], v[3], r));
                out.push(Cmd::Q(a[0], a[1], p[0], p[1]));
            }
            'C' => {
                let (a, b, p) = (map_pt(g, v[0], v[1], r), map_pt(g, v[2], v[3], r), map_pt(g, v[4], v[5], r));
                out.push(Cmd::C(a[0], a[1], b[0], b[1], p[0], p[1]));
            }
            _ => out.push(Cmd::Z),
        }
    }
    Shape::Path(out)
}

/// Maske auf höchstens `lim` Pixel Höhe verkleinern (schnellerer Vergleich).
fn shrink(mask: &[bool], mw: usize, mh: usize, lim: usize) -> (Vec<bool>, usize, usize) {
    if mh <= lim {
        return (mask.to_vec(), mw, mh);
    }
    let f = lim as f64 / mh as f64;
    let (nw, nh) = (((mw as f64 * f).round() as usize).max(1), lim);
    let mut out = vec![false; nw * nh];
    for y in 0..nh {
        for x in 0..nw {
            let (sx, sy) = (((x as f64 + 0.5) / f) as usize, ((y as f64 + 0.5) / f) as usize);
            out[y * nw + x] = mask[sy.min(mh - 1) * mw + sx.min(mw - 1)];
        }
    }
    (out, nw, nh)
}

fn score(g: &Glyph, mask: &[bool], mw: usize, mh: usize) -> f64 {
    // Seitenverhältnis muss grob passen, sonst ist es ein anderes Zeichen
    let ga = (g.bb[2] - g.bb[0]) as f64 / (g.bb[3] - g.bb[1]).max(1e-3) as f64 + 0.15;
    let ma = mw as f64 / mh.max(1) as f64 + 0.15;
    let ratio = (ga / ma).max(ma / ga);
    if ratio > 1.8 {
        return 0.0;
    }
    let iou = mask_iou(mask, mw, mh, &polys(g, [0.0, 0.0, mw as f64, mh as f64]));
    // Schriften werden nicht gestaucht oder gedehnt: abweichende Proportionen kosten
    iou * (-3.0 * (ratio.ln() - 0.08).max(0.0)).exp()
}

// ---------------------------------------------------------------------------
// Schriftdateien

pub fn font_files() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if cfg!(target_os = "macos") {
        dirs.push("/System/Library/Fonts".into());
        dirs.push("/System/Library/Fonts/Supplemental".into());
        dirs.push("/Library/Fonts".into());
        if let Some(h) = std::env::var_os("HOME") {
            dirs.push(PathBuf::from(h).join("Library/Fonts"));
        }
    } else if cfg!(target_os = "windows") {
        dirs.push("C:\\Windows\\Fonts".into());
    } else {
        dirs.push("/usr/share/fonts".into());
        if let Some(h) = std::env::var_os("HOME") {
            dirs.push(PathBuf::from(h).join(".local/share/fonts"));
        }
    }
    if let Some(extra) = std::env::var_os("ZMN_FONTS") {
        dirs = std::env::split_paths(&extra).collect();
    }
    let mut out = Vec::new();
    let mut stack = dirs;
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            let ext = p.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
            if !matches!(ext.as_str(), "ttf" | "otf" | "ttc") {
                continue;
            }
            // sehr große Dateien sind fast immer asiatische Schriften oder Emoji
            if e.metadata().map(|m| m.len() > 30_000_000).unwrap_or(true) {
                continue;
            }
            out.push(p);
        }
    }
    out.sort();
    out.dedup();
    out
}

fn name_of(face: &ttf_parser::Face, id: u16) -> Option<String> {
    face.names().into_iter().filter(|n| n.name_id == id && n.is_unicode()).find_map(|n| n.to_string())
}

struct Cand {
    family: String,
    style: String,
    /// Herkunft: Datei und Index (None = eingebautes Inter, Index in INTER)
    src: (Option<PathBuf>, u32),
    /// je Abschnitt: (Wert, Umriss)
    per: Vec<(f64, Option<Glyph>)>,
}

/// Maske eines Bausteinbereichs (Bausteine b..j) mit Tintenrechteck im Quellbild.
type Piece = (Vec<bool>, usize, usize, [f64; 4]);

fn cut_atoms(l: &TextAnalysis, b: usize, j: usize) -> Option<Piece> {
    let (xa, xb) = (l.atoms[b].0, l.atoms[j - 1].1);
    let (mut ya, mut yb) = (l.lh, 0);
    for y in 0..l.lh {
        if (xa..xb).any(|x| l.lmask[y * l.lw + x]) {
            ya = ya.min(y);
            yb = yb.max(y + 1);
        }
    }
    if yb <= ya {
        return None;
    }
    let (mw, mh) = (xb - xa, yb - ya);
    let mut m = vec![false; mw * mh];
    for y in 0..mh {
        for x in 0..mw {
            m[y * mw + x] = l.lmask[(ya + y) * l.lw + xa + x];
        }
    }
    let ink = [(l.lx + xa) as f64 / l.sc, (l.ly + ya) as f64 / l.sc, (l.lx + xb) as f64 / l.sc, (l.ly + yb) as f64 / l.sc];
    let (m, mw, mh) = shrink(&m, mw, mh, 56);
    Some((m, mw, mh, ink))
}

const MAXC: usize = 3; // Zeichen je Gruppe
const MAXA: usize = 6; // Bausteine je Gruppe

/// Gruppe: (Zeichen von, bis, Bausteine von, bis)
type Grp = (usize, usize, usize, usize);

/// Werte aller möglichen Gruppen für eine Schrift.
fn group_values(face: &ttf_parser::Face, l: &TextAnalysis, pieces: &std::collections::HashMap<(usize, usize), Piece>) -> std::collections::HashMap<Grp, f64> {
    let chars = &l.chars;
    let (nc, na) = (chars.len(), l.atoms.len());
    let mut vals = std::collections::HashMap::new();
    for a in 0..nc {
        for i in a + 1..=(a + MAXC).min(nc) {
            let g = if i - a == 1 { glyph(face, chars[a]) } else { glyph_run(face, &chars[a..i].iter().collect::<String>()) };
            let Some(g) = g else { continue };
            // Bausteine grob passend zur Lage in der Zeile (spart Rechenzeit)
            let (fa, fi) = (a as f64 / nc as f64, i as f64 / nc as f64);
            for b in 0..na {
                let pos = b as f64 / na as f64;
                if (pos - fa).abs() > 0.35 && (fi - pos).abs() > 0.35 {
                    continue;
                }
                for j in b + 1..=(b + MAXA).min(na) {
                    if let Some((m, mw, mh, _)) = pieces.get(&(b, j)) {
                        vals.insert((a, i, b, j), score(&g, m, *mw, *mh));
                    }
                }
            }
        }
    }
    vals
}

/// Beste Zuordnung von Zeichen zu Bausteinen für eine Bewertungsfunktion.
fn best_groups(nc: usize, na: usize, chars_alnum: &dyn Fn(usize) -> bool, val: &dyn Fn(&Grp) -> Option<f64>) -> (Vec<Grp>, f64) {
    if nc == 0 || na == 0 {
        return (Vec::new(), 0.0);
    }
    let inf = f64::MAX / 4.0;
    let mut dp = vec![vec![inf; na + 1]; nc + 1];
    let mut from = vec![vec![(0usize, 0usize); na + 1]; nc + 1];
    dp[0][0] = 0.0;
    for i in 1..=nc {
        for j in 0..=na {
            // Satzzeichen ohne sichtbare Tinte dürfen ausgelassen werden
            if !chars_alnum(i - 1) && dp[i - 1][j] < inf {
                let c = dp[i - 1][j] + 0.6;
                if c < dp[i][j] {
                    dp[i][j] = c;
                    from[i][j] = (i - 1, j);
                }
            }
            if j == 0 {
                continue;
            }
            for a in i.saturating_sub(MAXC)..i {
                for b in j.saturating_sub(MAXA)..j {
                    if dp[a][b] >= inf {
                        continue;
                    }
                    let Some(v) = val(&(a, i, b, j)) else { continue };
                    let c = dp[a][b] + (1.0 - v) * (i - a) as f64;
                    if c < dp[i][j] {
                        dp[i][j] = c;
                        from[i][j] = (a, b);
                    }
                }
            }
        }
    }
    if dp[nc][na] >= inf {
        return (Vec::new(), 0.0);
    }
    let mut out = Vec::new();
    let (mut i, mut j) = (nc, na);
    while i > 0 {
        let (a, b) = from[i][j];
        if b < j {
            out.push((a, i, b, j));
        }
        i = a;
        j = b;
    }
    out.reverse();
    (out, 1.0 - dp[nc][na] / nc as f64)
}

const INTER: [(&[u8], &str); 2] = [(include_bytes!("../assets/Inter-Regular.otf"), "Regular"), (include_bytes!("../assets/Inter-SemiBold.otf"), "SemiBold")];

/// Passende Schrift für alle Zeilen suchen und Umrisse erzeugen.
pub fn match_lines(lines: &[TextAnalysis], progress: &dyn Fn(f32)) -> (Vec<TextOut>, Vec<Match>) {
    // Masken verkleinern
    let segs: Vec<Vec<(Vec<bool>, usize, usize)>> = lines.iter().map(|l| l.segs.iter().map(|s| shrink(&s.mask, s.mw, s.mh, 56)).collect()).collect();
    let mut cands: Vec<Cand> = Vec::new();
    let eval_face = |face: &ttf_parser::Face, family: String, style: String, src: (Option<PathBuf>, u32), cands: &mut Vec<Cand>| {
        let mut per = Vec::new();
        let mut any = false;
        for (li, l) in lines.iter().enumerate() {
            for (si, s) in l.segs.iter().enumerate() {
                let g = if s.chars.chars().count() == 1 { s.chars.chars().next().and_then(|c| glyph(face, c)) } else { glyph_run(face, &s.chars) };
                let v = match &g {
                    Some(g) => {
                        let (m, mw, mh) = &segs[li][si];
                        score(g, m, *mw, *mh)
                    }
                    None => 0.0,
                };
                if v > 0.0 {
                    any = true;
                }
                per.push((v, g));
            }
        }
        if any {
            cands.push(Cand { family, style, src, per });
        }
    };
    for (i, (data, style)) in INTER.iter().enumerate() {
        if let Ok(f) = ttf_parser::Face::parse(data, 0) {
            eval_face(&f, "Inter".into(), style.to_string(), (None, i as u32), &mut cands);
        }
    }
    let files = font_files();
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(1, 12);
    let done = std::sync::atomic::AtomicUsize::new(0);
    let parts: Vec<Vec<Cand>> = std::thread::scope(|sc| {
        let hs: Vec<_> = (0..threads)
            .map(|t| {
                let (files, eval_face, done) = (&files, &eval_face, &done);
                sc.spawn(move || {
                    let mut local = Vec::new();
                    for p in files.iter().skip(t).step_by(threads) {
                        done.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Ok(data) = std::fs::read(p) else { continue };
                        let n = ttf_parser::fonts_in_collection(&data).unwrap_or(1).min(24);
                        for idx in 0..n {
                            let Ok(face) = ttf_parser::Face::parse(&data, idx) else { continue };
                            let family = name_of(&face, 16).or_else(|| name_of(&face, 1)).unwrap_or_else(|| p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default());
                            let style = name_of(&face, 17).or_else(|| name_of(&face, 2)).unwrap_or_else(|| "Regular".into());
                            if face.is_italic() || style.to_lowercase().contains("italic") || style.to_lowercase().contains("oblique") {
                                continue;
                            }
                            eval_face(&face, family, style, (Some(p.clone()), idx), &mut local);
                        }
                    }
                    local
                })
            })
            .collect();
        // Fortschritt melden, solange gearbeitet wird
        while hs.iter().any(|h| !h.is_finished()) {
            progress(done.load(std::sync::atomic::Ordering::Relaxed) as f32 / files.len().max(1) as f32);
            std::thread::sleep(std::time::Duration::from_millis(60));
        }
        hs.into_iter().map(|h| h.join().unwrap_or_default()).collect()
    });
    for p in parts {
        cands.extend(p);
    }
    progress(1.0);
    // Abschnitte je Zeile durchzählen
    let mut ranges = Vec::new();
    let mut k = 0;
    for l in lines {
        ranges.push(k..k + l.segs.len());
        k += l.segs.len();
    }
    let debug = std::env::var_os("ZMN_DEBUG").is_some();
    let mut outs = Vec::new();
    let mut matches = Vec::new();
    for (li, l) in lines.iter().enumerate() {
        let r = ranges[li].clone();
        if r.is_empty() || l.atoms.is_empty() {
            continue;
        }
        // Stufe 1: grobe Rangliste mit der geschätzten Zeichenverteilung
        let mut rank: Vec<(f64, usize)> = cands
            .iter()
            .enumerate()
            .map(|(ci, c)| (r.clone().map(|si| c.per[si].0).sum::<f64>() / r.len() as f64, ci))
            .collect();
        rank.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        // Stufe 2: für die besten Schriften Zeichen und Tintenbausteine neu
        // zuordnen – mit dem echten Vergleich statt geschätzter Breiten
        let mut pieces = std::collections::HashMap::new();
        for j in 1..=l.atoms.len() {
            for b in j.saturating_sub(MAXA)..j {
                if let Some(p) = cut_atoms(l, b, j) {
                    pieces.insert((b, j), p);
                }
            }
        }
        // Familien der besten Schnitte, davon alle Schnitte prüfen
        let mut fams: Vec<String> = Vec::new();
        for &(_, ci) in rank.iter().take(8) {
            if !fams.contains(&cands[ci].family) {
                fams.push(cands[ci].family.clone());
            }
        }
        // alle Schnitte dieser Familien parallel bewerten
        let todo: Vec<usize> = cands.iter().enumerate().filter(|(_, c)| fams.contains(&c.family)).map(|x| x.0).collect();
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(1, 12);
        let evald: Vec<(usize, std::collections::HashMap<Grp, f64>)> = std::thread::scope(|sc| {
            let hs: Vec<_> = (0..threads)
                .map(|t| {
                    let (todo, cands, pieces) = (&todo, &cands, &pieces);
                    sc.spawn(move || {
                        let mut out = Vec::new();
                        for &ci in todo.iter().skip(t).step_by(threads) {
                            let c = &cands[ci];
                            let data: std::borrow::Cow<[u8]> = match &c.src.0 {
                                Some(p) => match std::fs::read(p) {
                                    Ok(d) => std::borrow::Cow::Owned(d),
                                    Err(_) => continue,
                                },
                                None => std::borrow::Cow::Borrowed(INTER[c.src.1 as usize].0),
                            };
                            let Ok(face) = ttf_parser::Face::parse(&data, if c.src.0.is_some() { c.src.1 } else { 0 }) else { continue };
                            out.push((ci, group_values(&face, l, pieces)));
                        }
                        out
                    })
                })
                .collect();
            hs.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
        });
        // je Familie: jede Gruppe darf den besten Schnitt nehmen (fett + dünn in einer Zeile)
        let mut best: Option<(f64, String, Vec<Grp>)> = None;
        for fam in &fams {
            let members: Vec<&(usize, std::collections::HashMap<Grp, f64>)> = evald.iter().filter(|e| &cands[e.0].family == fam).collect();
            let val = |g: &Grp| -> Option<f64> { members.iter().filter_map(|e| e.1.get(g).copied()).fold(None, |m, v| Some(m.map_or(v, |m: f64| m.max(v)))) };
            let (grp, v) = best_groups(l.chars.len(), l.atoms.len(), &|i| l.chars[i].is_alphanumeric(), &val);
            if debug {
                eprintln!("      {fam}: {:.2} {:?}", v, grp.iter().map(|g| (l.chars[g.0..g.1].iter().collect::<String>(), (val(g).unwrap_or(0.0) * 100.0).round())).collect::<Vec<_>>());
            }
            if best.as_ref().map(|b| v > b.0).unwrap_or(true) {
                best = Some((v, fam.clone(), grp));
            }
        }
        let Some((val, fam, grp)) = best else { continue };
        if val < 0.5 {
            continue; // nichts passt gut genug – lieber als Fläche nachzeichnen
        }
        let mut shapes = Vec::new();
        let mut styles: Vec<String> = Vec::new();
        for g in &grp {
            let mut pick = (f64::MIN, usize::MAX);
            for (ci, table) in &evald {
                if cands[*ci].family != fam {
                    continue;
                }
                let v = table.get(g).copied().unwrap_or(0.0);
                if v > pick.0 {
                    pick = (v, *ci);
                }
            }
            if pick.1 == usize::MAX {
                continue;
            }
            let c = &cands[pick.1];
            let data: std::borrow::Cow<[u8]> = match &c.src.0 {
                Some(p) => std::borrow::Cow::Owned(std::fs::read(p).unwrap_or_default()),
                None => std::borrow::Cow::Borrowed(INTER[c.src.1 as usize].0),
            };
            let Ok(face) = ttf_parser::Face::parse(&data, if c.src.0.is_some() { c.src.1 } else { 0 }) else { continue };
            let text: String = l.chars[g.0..g.1].iter().collect();
            let gl = if g.1 - g.0 == 1 { glyph(&face, l.chars[g.0]) } else { glyph_run(&face, &text) };
            if let (Some(gl), Some(p)) = (gl, pieces.get(&(g.2, g.3))) {
                let ink = p.3;
                let (iw, ih) = (ink[2] - ink[0], ink[3] - ink[1]);
                // nicht stärker verzerren als nötig: weicht das Seitenverhältnis deutlich
                // ab, die Glyphe in Originalproportion mittig setzen
                let ga = ((gl.bb[2] - gl.bb[0]) / (gl.bb[3] - gl.bb[1]).max(1e-3)) as f64;
                let k = (iw / ih.max(1e-6)) / ga.max(1e-6);
                let rect = if !(0.77..=1.3).contains(&k) {
                    let w2 = (ga * ih).min(iw.max(ga * ih * 0.77));
                    [ink[0] + (iw - w2) / 2.0, ink[1], w2, ih]
                } else {
                    [ink[0], ink[1], iw, ih]
                };
                shapes.push(to_shape(&gl, rect));
                if !styles.contains(&c.style) {
                    styles.push(c.style.clone());
                }
            }
        }
        outs.push(TextOut { color: l.fg, shapes, erase: l.erase.clone() });
        matches.push(Match { text: l.line.text.clone(), family: fam, styles, score: val });
    }
    (outs, matches)
}
