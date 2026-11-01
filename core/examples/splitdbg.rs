//! Debug: einen Farbbereich abtrennen und offene Kanten des Teils untersuchen.
//! splitdbg <3mf> [objekt]
use mesh2step_core::{paint, split};
use std::collections::{HashMap, HashSet};

fn key(p: mesh2step_core::math::V3) -> (i64, i64, i64) {
    ((p.x * 1e5).round() as i64, (p.y * 1e5).round() as i64, (p.z * 1e5).round() as i64)
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let obj: usize = a.get(2).map(|s| s.parse().unwrap()).unwrap_or(0);
    let (raw, _, _) = paint::load_colored(std::path::Path::new(&a[1])).unwrap();
    let mut c = raw[obj].clone();
    split::clean_specks(&mut c, 2.0);
    split::smooth_boundaries(&mut c, 0.3);
    let reg = split::regions(&c);
    let orig: HashSet<_> = c.verts.iter().map(|p| key(*p)).collect();
    let mut ids: Vec<usize> = (0..reg.list.len()).filter(|&r| reg.list[r].color != reg.base_color).collect();
    ids.sort_by(|&x, &y| reg.list[y].area.partial_cmp(&reg.list[x].area).unwrap());
    for &r in ids.iter().take(6) {
        let ex: Vec<bool> = (0..reg.list.len()).map(|i| i == r).collect();
        for cap in [split::CapMethod::Auto, split::CapMethod::Depth] {
            let (parts, _) = split::split_by_color(&c, &reg, &ex, &split::ColorSplitOpts { cap, depth: 1.5, merge_same_color: false }, 0);
            for p in &parts {
                if p.kind == split::PartKind::Body && p.open_edges < 200 { continue; }
                let mut em: HashMap<(u32, u32), (u32, u32)> = HashMap::new();
                for t in &p.tris {
                    for k in 0..3 {
                        let (x, y) = (t[k], t[(k + 1) % 3]);
                        let e = em.entry((x.min(y), x.max(y))).or_default();
                        if x < y { e.0 += 1 } else { e.1 += 1 }
                    }
                }
                let mut cls: HashMap<String, usize> = HashMap::new();
                for (&(x, y), &(f, b)) in &em {
                    if (f, b) == (1, 1) { continue; }
                    let ox = orig.contains(&key(p.verts[x as usize]));
                    let oy = orig.contains(&key(p.verts[y as usize]));
                    let kind = match (f, b) { (1, 0) | (0, 1) => "offen", (2, 0) | (0, 2) => "gleich-gerichtet", _ => "mehrfach" };
                    *cls.entry(format!("{kind} {}", match (ox, oy) { (true, true) => "orig-orig", (false, false) => "neu-neu", _ => "orig-neu" })).or_default() += 1;
                }
                println!("Bereich {r} Farbe {} Fläche {:.0} {:?}: {} {} Dreiecke offen={} vol={:.1} {:?}", reg.list[r].color, reg.list[r].area, cap, p.name, p.tris.len(), p.open_edges, p.volume, cls);
            }
        }
    }
}
