//! Maßvergleich STL ↔ STEP auf der Kommandozeile (zum Testen).
use mesh2step_core::compare::{self, Align, Grid, TriMesh};
use mesh2step_core::{io, stepread};
use std::path::Path;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.len() < 2 {
        eprintln!("masscheck <netz.stl> <modell.step> [toleranz] [vernetzung] [none|center|fit]");
        std::process::exit(2);
    }
    let tol: f64 = a.get(2).and_then(|x| x.parse().ok()).unwrap_or(0.05);
    let defl: f64 = a.get(3).and_then(|x| x.parse().ok()).unwrap_or(0.002);
    let mode = match a.get(4).map(|s| s.as_str()) { Some("center") => Align::Center, Some("fit") => Align::BestFit, _ => Align::None };
    let t0 = std::time::Instant::now();
    let raws = io::load_file(Path::new(&a[0])).expect("Netz");
    let mut stl = TriMesh::from_raws(&raws);
    if let Ok(f) = std::env::var("X_SCALE") {
        let f: f64 = f.parse().unwrap();
        let st = compare::stats(&stl);
        for v in stl.verts.iter_mut() { *v = st.center + (*v - st.center) * f; }
    }
    stl.orient_outward();
    let cad = stepread::read_file(Path::new(&a[1]), defl).expect("STEP");
    let t1 = t0.elapsed().as_secs_f64();
    let mut cm = TriMesh::from_cad(&cad);
    let flipped = cm.orient_outward();
    let (sa, sb) = (compare::stats(&stl), compare::stats(&cm));
    let mut kinds = std::collections::BTreeMap::new();
    for f in &cad.faces { *kinds.entry(f.surf.kind_de()).or_insert(0) += 1; }
    println!("STEP: {} Flächen {:?}, {} Dreiecke, Einheit {}, gedreht {}, {:.2}s", cad.faces.len(), kinds, cad.tris.len(), cad.unit, flipped, t1);
    for w in &cad.warnings { println!("  Hinweis: {w}"); }
    println!("Größe STL  {:.4} {:.4} {:.4}  Vol {:.3}  offen {}", sa.size.x, sa.size.y, sa.size.z, sa.volume, sa.open_edges);
    println!("Größe STEP {:.4} {:.4} {:.4}  Vol {:.3}  offen {}", sb.size.x, sb.size.y, sb.size.z, sb.volume, sb.open_edges);
    let gb = Grid::new(&cm);
    let m = compare::align(&stl, &cm, &gb, mode, &|_| true).unwrap();
    let (tr, ang) = compare::xf_amount(&m);
    let stl2 = stl.transformed(&m);
    let da = compare::deviation(&stl2, &gb, &|_| true).unwrap();
    let ga = Grid::new(&stl2);
    let mut db = compare::deviation(&cm, &ga, &|_| true).unwrap();
    db.negate();
    println!("Ausrichtung: {:.4} mm, {:.3}°  max STL→STEP bei {:.3} {:.3} {:.3}", tr, ang, da.max_at.x, da.max_at.y, da.max_at.z);
    println!("STL→STEP  max+ {:.4} max- {:.4} mittel {:.4} rms {:.4} inTol {:.3}", da.max_pos, da.max_neg, da.mean_abs, da.rms, da.within(tol));
    println!("STEP→STL  max+ {:.4} max- {:.4} mittel {:.4} rms {:.4} inTol {:.3}", db.max_pos, db.max_neg, db.mean_abs, db.rms, db.within(tol));
    if std::env::var("M2S_MAXT").is_ok() {
        let mut st = compare::Stamp::new();
        let (bt, _) = da.face_dev.iter().enumerate().fold((0, f32::MIN), |a, (i, &v)| if v > a.1 { (i, v) } else { a });
        let tt = stl2.tris[bt];
        let mut best = (0usize, f32::MIN);
        for k in 0..6 { if da.sample_dev[bt][k] > best.1 { best = (k, da.sample_dev[bt][k]); } }
        let pv = |i: usize| stl2.verts[tt[i] as usize];
        let pt = match best.0 { 0 => pv(0), 1 => pv(1), 2 => pv(2), 3 => (pv(0) + pv(1)) * 0.5, 4 => (pv(1) + pv(2)) * 0.5, _ => (pv(2) + pv(0)) * 0.5 };
        println!("max+ Stichprobe {} Wert {:.4} Ecken {:?} {:?} {:?}", best.0, best.1, pv(0), pv(1), pv(2));
        let h = gb.query(pt, &mut st);
        let tr = cm.tris[h.t as usize];
        println!("Punkt {:?} -> Dreieck {} d {:.4}", da.max_at, h.t, h.s);
        for &v in &tr { let p = cm.verts[v as usize]; println!("   Ecke {:.4} {:.4} {:.4}  |p|-15 = {:.5}", p.x, p.y, p.z, p.norm() - 15.0); }
        let f = cad.faces.iter().position(|f| f.first <= h.t as usize && h.t as usize <= f.last).unwrap();
        println!("   Fläche {} Bereich {}..{}", f, cad.faces[f].first, cad.faces[f].last);
    }
    if std::env::var("M2S_VTX").is_ok() {
        let mut st = compare::Stamp::new();
        let mut v: Vec<(f64, usize)> = stl2.verts.iter().enumerate().map(|(i, p)| (gb.query(*p, &mut st).d, i)).collect();
        v.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        println!("STL-Ecken: größter Abstand {:.4}, 10. {:.4}, 100. {:.4}", v[0].0, v[9].0, v[99].0);
        let p = stl2.verts[v[0].1];
        println!("  bei {:.3} {:.3} {:.3}", p.x, p.y, p.z);
    }
    if std::env::var("M2S_ORI").is_ok() {
        let mut st = compare::Stamp::new();
        let mut flipped = std::collections::BTreeMap::new();
        for f in &cad.faces {
            let (mut agree, mut dis) = (0.0, 0.0);
            for t in f.first..=f.last {
                let tr = cad.tris[t];
                let (p0, p1, p2) = (cad.verts[tr[0] as usize], cad.verts[tr[1] as usize], cad.verts[tr[2] as usize]);
                let n = (p1 - p0).cross(p2 - p0);
                let h = ga.query((p0 + p1 + p2) / 3.0, &mut st);
                let s3 = stl2.tris[h.t as usize];
                let (q0, q1, q2) = (stl2.verts[s3[0] as usize], stl2.verts[s3[1] as usize], stl2.verts[s3[2] as usize]);
                let m = (q1 - q0).cross(q2 - q0);
                if n.dot(m) >= 0.0 { agree += n.norm(); } else { dis += n.norm(); }
            }
            if dis > agree { *flipped.entry(f.surf.kind_de()).or_insert(0) += 1; }
        }
        println!("verkehrt orientiert: {:?}  (STEP area {:.1}, STL area {:.1})", flipped, sb.area, sa.area);
    }
    if std::env::var("M2S_FACES").is_ok() {
        let mut worst: Vec<(f64, usize)> = cad.faces.iter().enumerate().map(|(i, f)| ((f.first..=f.last).map(|t| (db.face_dev[t] as f64).abs()).fold(0.0, f64::max), i)).collect();
        worst.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        for (d, i) in worst.iter().take(12) {
            let f = &cad.faces[*i];
            let mut a = 0.0; let mut sv = 0.0;
            for t in f.first..=f.last { let tr = cad.tris[t]; let (p0,p1,p2) = (cad.verts[tr[0] as usize], cad.verts[tr[1] as usize], cad.verts[tr[2] as usize]); a += 0.5*(p1-p0).cross(p2-p0).norm(); sv += p0.dot(p1.cross(p2))/6.0; }
            println!("  Fläche {} {} same {} max {:.4} Dreiecke {} Fläche {:.2} Vol {:.2}", i, f.surf.kind_de(), f.same_sense, d, f.last - f.first + 1, a, sv);
        }
    }
    let cyl = compare::cylinders(&cad);
    let ms = compare::measure_cylinders(&stl2, &cyl);
    for (c, m) in cyl.iter().zip(ms.iter()) {
        match m {
            Some(m) => println!("  {} Ø {:.4} ist {:.4} Δ {:+.4} versatz {:.4} rund {:.4} L {:.2} cov {} flächen {}", if c.hole { "Bohrung" } else { "Zapfen " }, c.diameter(), 2.0 * m.r, 2.0 * m.r - c.diameter(), m.offset, m.roundness, c.length(), c.coverage, c.faces),
            None => println!("  {} Ø {:.4} nicht gefunden L {:.2} cov {}", if c.hole { "Bohrung" } else { "Zapfen " }, c.diameter(), c.length(), c.coverage),
        }
    }
    println!("Zeit {:.2}s", t0.elapsed().as_secs_f64());
}
