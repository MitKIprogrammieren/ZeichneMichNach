//! Debug: Eingang und Teilen-Ergebnis als Textnetz ausgeben (für Renderskripte).
use mesh2step_core::{paint, split};
use std::io::Write;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let path = std::path::Path::new(&a[1]);
    let cap = match a.get(3).map(|s| s.as_str()) {
        Some("m") => split::CapMethod::Membrane,
        Some("d") => split::CapMethod::Depth,
        _ => split::CapMethod::Auto,
    };
    let (raw, _colors, _w) = paint::load_colored(path).unwrap();
    let mut objs = Vec::new();
    let mut regs = Vec::new();
    for m in &raw {
        let mut c = m.clone();
        split::clean_specks(&mut c, 2.0);
        split::smooth_boundaries(&mut c, 0.3);
        regs.push(split::regions(&c));
        objs.push(c);
    }
    let mut f = std::io::BufWriter::new(std::fs::File::create(&a[2]).unwrap());
    for (i, m) in objs.iter().enumerate() {
        writeln!(f, "M in {} {}", i, m.name).unwrap();
        for v in &m.verts {
            writeln!(f, "V {} {} {}", v.x, v.y, v.z).unwrap();
        }
        for (t, c) in m.tris.iter().zip(&m.col) {
            writeln!(f, "F {} {} {} {}", t[0], t[1], t[2], c).unwrap();
        }
    }
    for (g, (m, reg)) in objs.iter().zip(&regs).enumerate() {
        let ex: Vec<bool> = reg.list.iter().map(|r| r.color != reg.base_color).collect();
        let (parts, w) = split::split_by_color(m, reg, &ex, &split::ColorSplitOpts { cap, depth: 1.5, merge_same_color: true }, g);
        for x in w {
            eprintln!("warn: {x}");
        }
        for (k, p) in parts.iter().enumerate() {
            eprintln!("{} open={} vol={:.1} tris={}", p.name, p.open_edges, p.volume, p.tris.len());
            writeln!(f, "M out {} {} {}", g, k, p.name.replace(' ', "_")).unwrap();
            for v in &p.verts {
                writeln!(f, "V {} {} {}", v.x, v.y, v.z).unwrap();
            }
            for (t, c) in p.tris.iter().zip(&p.col) {
                writeln!(f, "F {} {} {} {}", t[0], t[1], t[2], c).unwrap();
            }
        }
    }
}
