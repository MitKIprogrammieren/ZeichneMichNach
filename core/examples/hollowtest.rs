//! Debug: Aushöhlen mit/ohne selbsttragende Decke vergleichen. hollowtest <datei> [wand]
use mesh2step_core::hollow::{hollow, HollowOpts};
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let raws = mesh2step_core::io::load_file(std::path::Path::new(&a[1])).unwrap();
    let wall: f64 = a.get(2).and_then(|x| x.parse().ok()).unwrap_or(2.0);
    let r = mesh2step_core::repair::repair(&raws[0], &Default::default());
    for ss in [false, true] {
        let t0 = std::time::Instant::now();
        match hollow(&r.verts, &r.tris, &HollowOpts { wall, voxel: 0.0, self_supporting: ss }) {
            Ok(h) => {
                let (mut flat, mut total) = (0.0, 0.0);
                for x in &h.tris {
                    let (p0, p1, p2) = (h.verts[x[0] as usize], h.verts[x[1] as usize], h.verts[x[2] as usize]);
                    let n = (p1 - p0).cross(p2 - p0);
                    let ar = n.norm() * 0.5;
                    total += ar;
                    if n.z / n.norm().max(1e-12) < -0.85 {
                        flat += ar;
                    }
                }
                let an = mesh2step_core::repair::analyze_indexed(h.verts.clone(), h.tris.clone(), false).issues;
                eprintln!("selbsttragend={ss}: Hohlraum {:.0} mm³, flache Decke {:.1} %, offen {}, nm {}, {:.1} s", h.cavity_volume, 100.0 * flat / total, an.open_edges, an.nonmanifold_edges, t0.elapsed().as_secs_f64());
            }
            Err(e) => eprintln!("Fehler: {e}"),
        }
    }
}
