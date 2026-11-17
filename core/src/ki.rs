//! Kanten-KI: kleines U-Netz, das für jedes Pixel schätzt, ob dort eine echte Kante liegt
//! (Umriss, Streifen, Schrift) oder nur Schattierung, Glanz und Rauschen.
//!
//! Trainiert mit `tools/zmn-ki` (künstliche Bilder mit bekannter Lösung), Gewichte in
//! `core/ki/kanten.bin` (Format ZMNK, siehe `tools/zmn-ki/trainieren.py`). Die Rechnung
//! hier ist ohne Fremdbibliothek geschrieben und muss genau dem Netz dort entsprechen:
//! 3×3-Faltungen mit Nullrand, ReLU, 2×2-Max-Pooling, Nächster-Nachbar-Vergrößerung.

use std::sync::OnceLock;

static GEWICHTE: &[u8] = include_bytes!("../ki/kanten.bin");

struct Schicht {
    aus: usize,
    ein: usize,
    k: usize,
    w: Vec<f32>,
    b: Vec<f32>,
}

pub struct Netz {
    s: Vec<Schicht>,
}

fn netz() -> Option<&'static Netz> {
    static N: OnceLock<Option<Netz>> = OnceLock::new();
    // zum Ausprobieren neuer Gewichte ohne Neubau: ZMN_KI_GEWICHTE=/pfad/kanten.bin
    N.get_or_init(|| match std::env::var_os("ZMN_KI_GEWICHTE") {
        Some(p) => std::fs::read(p).ok().and_then(|d| laden(&d)),
        None => laden(GEWICHTE),
    })
    .as_ref()
}

/// Ist eine trainierte Kanten-KI eingebaut?
pub fn verfuegbar() -> bool {
    netz().is_some()
}

fn laden(d: &[u8]) -> Option<Netz> {
    if d.len() < 12 || &d[0..4] != b"ZMNK" {
        return None;
    }
    let u = |o: usize| -> Option<usize> { Some(u32::from_le_bytes(d.get(o..o + 4)?.try_into().ok()?) as usize) };
    if u(4)? != 1 {
        return None;
    }
    let n = u(8)?;
    let mut o = 12;
    let mut s = Vec::with_capacity(n);
    let f = |o: usize, cnt: usize| -> Option<Vec<f32>> {
        let b = d.get(o..o + cnt * 4)?;
        Some(b.chunks(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect())
    };
    for _ in 0..n {
        let (aus, ein, k) = (u(o)?, u(o + 4)?, u(o + 8)?);
        o += 12;
        let w = f(o, aus * ein * k * k)?;
        o += aus * ein * k * k * 4;
        let b = f(o, aus)?;
        o += aus * 4;
        s.push(Schicht { aus, ein, k, w, b });
    }
    // erwarteter Aufbau: 11 Schichten
    if s.len() != 11 || s[0].ein != 3 || s[10].aus != 1 {
        return None;
    }
    Some(Netz { s })
}

/// Faltung (3×3 mit Nullrand oder 1×1), optional mit ReLU. Eingabe/Ausgabe: Kanäle × h × w.
fn conv(l: &Schicht, x: &[f32], w: usize, h: usize, relu: bool) -> Vec<f32> {
    let n = w * h;
    let mut out = vec![0f32; l.aus * n];
    let threads = std::thread::available_parallelism().map(|v| v.get()).unwrap_or(4).min(l.aus).max(1);
    let per = l.aus.div_ceil(threads);
    std::thread::scope(|sc| {
        for (ci, chunk) in out.chunks_mut(per * n).enumerate() {
            sc.spawn(move || {
                for (oi, o) in chunk.chunks_mut(n).enumerate() {
                    let oc = ci * per + oi;
                    o.fill(l.b[oc]);
                    for ic in 0..l.ein {
                        let src = &x[ic * n..(ic + 1) * n];
                        let wb = &l.w[(oc * l.ein + ic) * l.k * l.k..(oc * l.ein + ic + 1) * l.k * l.k];
                        if l.k == 1 {
                            let wv = wb[0];
                            for (a, b) in o.iter_mut().zip(src) {
                                *a += wv * b;
                            }
                            continue;
                        }
                        for ky in 0..3usize {
                            for kx in 0..3usize {
                                let wv = wb[ky * 3 + kx];
                                if wv == 0.0 {
                                    continue;
                                }
                                // Ziel (y, x) liest Quelle (y + ky − 1, x + kx − 1)
                                let (y0, y1) = (if ky == 0 { 1 } else { 0 }, if ky == 2 { h - 1 } else { h });
                                let (x0, x1) = (if kx == 0 { 1 } else { 0 }, if kx == 2 { w - 1 } else { w });
                                for y in y0..y1 {
                                    let sy = y + ky - 1;
                                    let dst = &mut o[y * w + x0..y * w + x1];
                                    let s = &src[sy * w + x0 + kx - 1..sy * w + x1 + kx - 1];
                                    for (a, b) in dst.iter_mut().zip(s) {
                                        *a += wv * b;
                                    }
                                }
                            }
                        }
                    }
                    if relu {
                        for v in o.iter_mut() {
                            if *v < 0.0 {
                                *v = 0.0;
                            }
                        }
                    }
                }
            });
        }
    });
    out
}

fn pool(x: &[f32], c: usize, w: usize, h: usize) -> Vec<f32> {
    let (w2, h2) = (w / 2, h / 2);
    let mut o = vec![0f32; c * w2 * h2];
    for ch in 0..c {
        let s = &x[ch * w * h..];
        for y in 0..h2 {
            for xx in 0..w2 {
                let a = s[(2 * y) * w + 2 * xx].max(s[(2 * y) * w + 2 * xx + 1]);
                let b = s[(2 * y + 1) * w + 2 * xx].max(s[(2 * y + 1) * w + 2 * xx + 1]);
                o[ch * w2 * h2 + y * w2 + xx] = a.max(b);
            }
        }
    }
    o
}

/// Nearest-Vergrößerung ×2 von `a` (ca Kanäle, w/2 × h/2), dahinter `b` (cb Kanäle, w × h).
fn up_cat(a: &[f32], ca: usize, b: &[f32], w: usize, h: usize) -> Vec<f32> {
    let n = w * h;
    let (w2, h2) = (w / 2, h / 2);
    let mut o = Vec::with_capacity(ca * n + b.len());
    for ch in 0..ca {
        let s = &a[ch * w2 * h2..(ch + 1) * w2 * h2];
        for y in 0..h {
            for x in 0..w {
                o.push(s[(y / 2) * w2 + x / 2]);
            }
        }
    }
    o.extend_from_slice(b);
    o
}

impl Netz {
    /// Kantenwahrscheinlichkeit (0…1) für ein RGB-Bild (Kanäle × h × w, Werte 0…1).
    /// w und h müssen durch 4 teilbar sein.
    fn rechnen(&self, x: &[f32], w: usize, h: usize) -> Vec<f32> {
        let s = &self.s;
        let e1 = conv(&s[1], &conv(&s[0], x, w, h, true), w, h, true);
        let (w2, h2) = (w / 2, h / 2);
        let p1 = pool(&e1, s[1].aus, w, h);
        let e2 = conv(&s[3], &conv(&s[2], &p1, w2, h2, true), w2, h2, true);
        let (w4, h4) = (w / 4, h / 4);
        let p2 = pool(&e2, s[3].aus, w2, h2);
        let m = conv(&s[5], &conv(&s[4], &p2, w4, h4, true), w4, h4, true);
        let d2 = up_cat(&m, s[5].aus, &e2, w2, h2);
        let d2 = conv(&s[7], &conv(&s[6], &d2, w2, h2, true), w2, h2, true);
        let d1 = up_cat(&d2, s[7].aus, &e1, w, h);
        let d1 = conv(&s[9], &conv(&s[8], &d1, w, h, true), w, h, true);
        let o = conv(&s[10], &d1, w, h, false);
        o.into_iter().map(|v| 1.0 / (1.0 + (-v).exp())).collect()
    }
}

/// Kantenkarte für ein RGBA-Bild: je Pixel Wahrscheinlichkeit einer echten Kante.
/// Gerechnet wird auf höchstens `max_seite` Pixeln (Geschwindigkeit), das Ergebnis
/// wird auf w × h zurückgerechnet. None, wenn keine KI eingebaut ist.
pub fn kanten(rgba: &[u8], w: usize, h: usize, max_seite: usize) -> Option<Vec<f32>> {
    let net = netz()?;
    if w < 8 || h < 8 {
        return None;
    }
    let sc = (max_seite as f64 / w.max(h) as f64).min(1.0);
    let (bw, bh) = (((w as f64 * sc).round() as usize).max(8), ((h as f64 * sc).round() as usize).max(8));
    let small = if (bw, bh) == (w, h) { rgba.to_vec() } else { crate::vector::resample(rgba, w, h, bw, bh) };
    // auf Vielfache von 4 auffüllen (Rand wiederholen)
    let (pw, ph) = (bw.div_ceil(4) * 4, bh.div_ceil(4) * 4);
    let mut x = vec![0f32; 3 * pw * ph];
    for y in 0..ph {
        for xx in 0..pw {
            let p = (y.min(bh - 1) * bw + xx.min(bw - 1)) * 4;
            // durchsichtige Pixel als Weiß (wie im Training gibt es keinen Alphakanal)
            let a = small[p + 3] as f32 / 255.0;
            for c in 0..3 {
                x[c * pw * ph + y * pw + xx] = (small[p + c] as f32 / 255.0) * a + (1.0 - a);
            }
        }
    }
    let k = net.rechnen(&x, pw, ph);
    // bilinear zurück auf w × h
    let mut out = vec![0f32; w * h];
    for y in 0..h {
        let fy = ((y as f64 + 0.5) * bh as f64 / h as f64 - 0.5).clamp(0.0, (bh - 1) as f64);
        let (y0, ty) = (fy.floor() as usize, (fy - fy.floor()) as f32);
        let y1 = (y0 + 1).min(bh - 1);
        for xx in 0..w {
            let fx = ((xx as f64 + 0.5) * bw as f64 / w as f64 - 0.5).clamp(0.0, (bw - 1) as f64);
            let (x0, tx) = (fx.floor() as usize, (fx - fx.floor()) as f32);
            let x1 = (x0 + 1).min(bw - 1);
            let v = |yy: usize, xq: usize| k[yy * pw + xq];
            out[y * w + xx] = (v(y0, x0) * (1.0 - tx) + v(y0, x1) * tx) * (1.0 - ty) + (v(y1, x0) * (1.0 - tx) + v(y1, x1) * tx) * ty;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Abgleich mit PyTorch: ZMN_KI_GEWICHTE und ZMN_KI_TEST=<roh.f32 Eingabe> setzen,
    /// Ausgabe landet in <Eingabe>.aus (siehe tools/zmn-ki/abgleich.py).
    #[test]
    #[ignore]
    fn abgleich_mit_pytorch() {
        let p = std::env::var("ZMN_KI_TEST").expect("ZMN_KI_TEST");
        let d = std::fs::read(&p).unwrap();
        let w = u32::from_le_bytes(d[0..4].try_into().unwrap()) as usize;
        let h = u32::from_le_bytes(d[4..8].try_into().unwrap()) as usize;
        let x: Vec<f32> = d[8..].chunks(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
        let o = netz().expect("Gewichte").rechnen(&x, w, h);
        std::fs::write(format!("{p}.aus"), o.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>()).unwrap();
    }

    #[test]
    fn leere_gewichte_sind_kein_netz() {
        assert!(laden(&[]).is_none());
        assert!(laden(b"ZMNK\x02\0\0\0\0\0\0\0").is_none());
    }
}
