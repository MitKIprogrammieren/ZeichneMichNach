//! Debug: Teilen eines Objekts rendern (Eingang / Ergebnis / nur Farbteile) als PNG.
//! shot <3mf> <objekt> <out.png> [cap a|m|d] [zoom-faktor]
use mesh2step_core::math::V3;
use mesh2step_core::{filament_rgb, paint, split, vector};

type M<'a> = (&'a [V3], &'a [[u32; 3]], &'a [u8], [f64; 3]);

fn tile(ms: &[M], colors: &[[u8; 3]], el: f64, az: f64, s: usize, img: &mut [u8], iw: usize, ox: usize, oy: usize, fit: Option<([f64; 3], f64)>) {
    let (ce, se, ca, sa) = (el.to_radians().cos(), el.to_radians().sin(), az.to_radians().cos(), az.to_radians().sin());
    let rot = |p: [f64; 3]| -> [f64; 3] {
        let x = p[0] * ca - p[1] * sa;
        let y = p[0] * sa + p[1] * ca;
        // Blick von oben geneigt: Kamera-Y = z*ce - y*se, Tiefe = y*ce + z*se
        [x, p[2] * ce - y * se, y * ce + p[2] * se]
    };
    let mut pts: Vec<Vec<[f64; 3]>> = Vec::new();
    let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
    for m in ms {
        let v: Vec<[f64; 3]> = m.0.iter().map(|p| rot([p.x + m.3[0], p.y + m.3[1], p.z + m.3[2]])).collect();
        for q in &v {
            for k in 0..2 {
                lo[k] = lo[k].min(q[k]);
                hi[k] = hi[k].max(q[k]);
            }
        }
        pts.push(v);
    }
    let (cx, cy, span) = match fit {
        Some((c, r)) => {
            let q = rot(c);
            (q[0], q[1], 2.0 * r)
        }
        None => ((lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (hi[0] - lo[0]).max(hi[1] - lo[1]) * 1.05),
    };
    let sc = s as f64 / span.max(1e-9);
    let mut zb = vec![f64::MIN; s * s];
    let l = { let v = [0.35, -0.45, 0.82]; let n = (v[0]*v[0]+v[1]*v[1]+v[2]*v[2] as f64).sqrt(); [v[0]/n, v[1]/n, v[2]/n] };
    for (m, v) in ms.iter().zip(&pts) {
        for (t, &c) in m.1.iter().zip(m.2) {
            let p: Vec<[f64; 3]> = t.iter().map(|&i| {
                let q = v[i as usize];
                [(q[0] - cx) * sc + s as f64 / 2.0, s as f64 / 2.0 - (q[1] - cy) * sc, q[2]]
            }).collect();
            let e1 = [p[1][0] - p[0][0], p[1][1] - p[0][1]];
            let e2 = [p[2][0] - p[0][0], p[2][1] - p[0][1]];
            let area = e1[0] * e2[1] - e1[1] * e2[0];
            if area.abs() < 1e-12 { continue; }
            // Normale in Kamerakoordinaten (aus 3D)
            let a = v[t[0] as usize]; let b = v[t[1] as usize]; let d = v[t[2] as usize];
            let u = [b[0]-a[0], b[1]-a[1], b[2]-a[2]]; let w = [d[0]-a[0], d[1]-a[1], d[2]-a[2]];
            let mut n = [u[1]*w[2]-u[2]*w[1], u[2]*w[0]-u[0]*w[2], u[0]*w[1]-u[1]*w[0]];
            let nl = (n[0]*n[0]+n[1]*n[1]+n[2]*n[2]).sqrt().max(1e-12);
            for k in 0..3 { n[k] /= nl; }
            let back = n[2] < 0.0;
            let sh = 0.3 + 0.7 * (n[0]*l[0] + n[1]*l[1] + n[2]*l[2]).abs();
            let mut rgb = filament_rgb(colors, c);
            if !back { rgb = [255, 40, 200]; } // Rückseite sichtbar = magenta
            let x0 = p.iter().map(|q| q[0]).fold(f64::MAX, f64::min).floor().max(0.0) as usize;
            let x1 = (p.iter().map(|q| q[0]).fold(f64::MIN, f64::max).ceil() as isize).min(s as isize - 1);
            let y0 = p.iter().map(|q| q[1]).fold(f64::MAX, f64::min).floor().max(0.0) as usize;
            let y1 = (p.iter().map(|q| q[1]).fold(f64::MIN, f64::max).ceil() as isize).min(s as isize - 1);
            if x1 < 0 || y1 < 0 { continue; }
            for y in y0..=y1 as usize {
                for x in x0..=x1 as usize {
                    let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
                    let w0 = ((p[1][0]-px)*(p[2][1]-py) - (p[1][1]-py)*(p[2][0]-px)) / area;
                    let w1 = ((p[2][0]-px)*(p[0][1]-py) - (p[2][1]-py)*(p[0][0]-px)) / area;
                    let w2 = 1.0 - w0 - w1;
                    if w0 < -1e-9 || w1 < -1e-9 || w2 < -1e-9 { continue; }
                    let z = w0*p[0][2] + w1*p[1][2] + w2*p[2][2];
                    let k = y * s + x;
                    if z > zb[k] {
                        zb[k] = z;
                        let o = ((oy + y) * iw + ox + x) * 4;
                        for j in 0..3 { img[o+j] = (rgb[j] as f64 * sh).min(255.0) as u8; }
                        img[o+3] = 255;
                    }
                }
            }
        }
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let obj: usize = a[2].parse().unwrap();
    let cap = match a.get(4).map(|s| s.as_str()) { Some("m") => split::CapMethod::Membrane, Some("d") => split::CapMethod::Depth, _ => split::CapMethod::Auto };
    let (raw, colors, _) = if a[1].ends_with(".txt") {
        // Dump aus M2S_DUMP: Region, Art, Größe, 9 Koordinaten. Farbe: Einzeldreieck-Ebene = 2,
        // Ebene = 1, Freiform = 4, sonstige Grundform = 3
        let mut verts = Vec::new();
        let mut tris = Vec::new();
        let mut col = Vec::new();
        for l in std::fs::read_to_string(&a[1]).unwrap().lines() {
            let f: Vec<f64> = l.split_whitespace().map(|x| x.parse().unwrap()).collect();
            let b = verts.len() as u32;
            for k in 0..3 { verts.push(mesh2step_core::math::v3(f[3 + 3 * k], f[4 + 3 * k], f[5 + 3 * k])); }
            tris.push([b, b + 1, b + 2]);
            col.push(if std::env::var("DUMPREG").is_ok() { if f[2] as usize <= 2 { 2 } else { 3 + (f[0] as u64 * 7 % 13) as u8 } } else { match (f[1] as u8, f[2] as usize) { (0, n) if n <= 2 => 2, (0, _) => 1, (5, _) => 4, _ => 3 } });
        }
        (vec![paint::ColoredMesh { name: "dump".into(), verts, tris, col }], Vec::new(), Vec::<String>::new())
    } else { paint::load_colored(std::path::Path::new(&a[1])).unwrap() };
    let mut c = raw[obj].clone();
    split::clean_specks(&mut c, 2.0);
    split::smooth_boundaries(&mut c, 0.3);
    let reg = split::regions(&c);
    let ex: Vec<bool> = reg.list.iter().map(|r| r.color != reg.base_color).collect();
    let (parts, w) = split::split_by_color(&c, &reg, &ex, &split::ColorSplitOpts { cap, depth: 1.5, merge_same_color: false }, 0);
    for x in w { eprintln!("warn {x}"); }
    for p in &parts {
        let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
        for q in &p.verts { for (k, x) in [q.x, q.y, q.z].iter().enumerate() { lo[k] = lo[k].min(*x); hi[k] = hi[k].max(*x); } }
        eprintln!("{} open={} vol={:.1} c={:.1},{:.1},{:.1} r={:.1}", p.name, p.open_edges, p.volume, (lo[0]+hi[0])/2.0, (lo[1]+hi[1])/2.0, (lo[2]+hi[2])/2.0, (0..3).map(|k| hi[k]-lo[k]).fold(0.0, f64::max)/2.0);
    }
    // Zoom auf Ausschnitt: optional "cx,cy,cz,r"
    let fit = a.get(5).map(|s| { let v: Vec<f64> = s.split(',').map(|x| x.parse().unwrap()).collect(); ([v[0], v[1], v[2]], v[3]) });
    let views: Vec<(f64, f64)> = match std::env::var("VIEWS") {
        Ok(v) => v.split(';').map(|p| { let q: Vec<f64> = p.split(',').map(|x| x.parse().unwrap()).collect(); (q[0], q[1]) }).collect(),
        Err(_) => vec![(60.0, 0.0), (35.0, -50.0), (35.0, 130.0), (-50.0, 30.0)],
    };
    let s = 520;
    let (iw, ih) = (s * 4, s * 3);
    let mut img = vec![255u8; iw * ih * 4];
    let inp: Vec<M> = vec![(&c.verts, &c.tris, &c.col, [0.0; 3])];
    let g: [f64; 3] = { let mut g = [0.0; 3]; for p in &c.verts { g[0]+=p.x; g[1]+=p.y; g[2]+=p.z; } let n = c.verts.len() as f64; [g[0]/n, g[1]/n, g[2]/n] };
    let ex_m: Vec<M> = parts.iter().map(|p| {
        let n = p.verts.len().max(1) as f64;
        let cc = p.verts.iter().fold([0.0;3], |acc, q| [acc[0]+q.x/n, acc[1]+q.y/n, acc[2]+q.z/n]);
        let f = if p.kind == split::PartKind::Body { 0.0 } else { 0.6 };
        (p.verts.as_slice(), p.tris.as_slice(), p.col.as_slice(), [(cc[0]-g[0])*f, (cc[1]-g[1])*f, (cc[2]-g[2])*f + if p.kind == split::PartKind::Body {0.0} else {8.0}])
    }).collect();
    let only: Vec<M> = parts.iter().filter(|p| p.kind != split::PartKind::Body).map(|p| (p.verts.as_slice(), p.tris.as_slice(), p.col.as_slice(), [0.0;3])).collect();
    for (k, &(el, az)) in views.iter().enumerate() {
        tile(&inp, &colors, el, az, s, &mut img, iw, k * s, 0, fit);
        tile(&ex_m, &colors, el, az, s, &mut img, iw, k * s, s, None);
        if !only.is_empty() { tile(&only, &colors, el, az, s, &mut img, iw, k * s, 2 * s, fit); }
    }
    std::fs::write(&a[3], vector::encode_png(iw, ih, &img)).unwrap();
}
