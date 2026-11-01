//! STEP-Datei lesen und Kennzahlen ausgeben (zum Testen des Lesers).
use mesh2step_core::compare::{self, TriMesh};
use mesh2step_core::stepread;
fn main() {
    for a in std::env::args().skip(1) {
        let t0 = std::time::Instant::now();
        match stepread::read_file(std::path::Path::new(&a), 0.005) {
            Ok(c) => {
                let mut m = TriMesh::from_cad(&c);
                let fl = m.orient_outward();
                let s = compare::stats(&m);
                let mut kinds = std::collections::BTreeMap::new();
                for f in &c.faces { *kinds.entry(f.surf.kind_de()).or_insert(0) += 1; }
                println!("{}: {} Flächen {:?} {} Dr. Einheit {} Größe {:.3}×{:.3}×{:.3} Vol {:.2} Fläche {:.2} gedreht {} Zyl {} {:.2}s", std::path::Path::new(&a).file_name().unwrap().to_string_lossy(), c.faces.len(), kinds, c.tris.len(), c.unit, s.size.x, s.size.y, s.size.z, s.volume, s.area, fl, compare::cylinders(&c).len(), t0.elapsed().as_secs_f64());
                for w in &c.warnings { println!("   Hinweis: {w}"); }
            }
            Err(e) => println!("{}: FEHLER {e}", a),
        }
    }
}
