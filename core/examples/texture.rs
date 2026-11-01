//! cargo run --release --example texture -- <in> <out.stl> <muster 0..11> <projektion a|c|s|p> <größe> <tiefe> [drehung]
use mesh2step_core::repair::{self, RepairOpts};
use mesh2step_core::texture::{self, Pattern, Projection, TexOpts};
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let meshes = mesh2step_core::io::load_file(std::path::Path::new(&a[1])).unwrap();
    let parts: Vec<_> = meshes.iter().map(|m| repair::repair(m, &RepairOpts::default())).collect();
    let mut out = Vec::new();
    let t0 = std::time::Instant::now();
    for p in &parts {
        let o = TexOpts {
            pattern: Pattern::ALL[a[3].parse::<usize>().unwrap()],
            projection: match a[4].as_str() { "c" => Projection::Cylinder, "s" => Projection::Sphere, "p" => Projection::Planar, _ => Projection::Auto },
            scale: a[5].parse().unwrap(),
            depth: a[6].parse().unwrap(),
            rotation: a.get(7).map(|s| s.parse().unwrap()).unwrap_or(0.0),
            simplify_tol: if std::env::var("NOSIMP").is_ok() { 0.0 } else { 0.01 },
            ..Default::default()
        };
        let r = texture::apply(&p.verts, &p.tris, &o);
        eprintln!("{} → unterteilt {} → {} Dreiecke, Kante {:.2} mm {:?}", p.tris.len(), r.subdivided, r.tris.len(), r.edge, r.warnings);
        let mut q = p.clone();
        q.verts = r.verts;
        q.tris = r.tris;
        out.push(q);
    }
    eprintln!("{:.2} s", t0.elapsed().as_secs_f64());
    std::fs::write(&a[2], repair::export(&out, "stl")).unwrap();
}
