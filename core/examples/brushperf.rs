use mesh2step_core::{brush::Canvas, paint};
use std::time::Instant;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (raw, _, _) = paint::load_colored(std::path::Path::new(&a[1])).unwrap();
    let t0 = Instant::now();
    let mut c = Canvas::new(raw[0].clone());
    println!("Canvas {} Dreiecke: {:?}", c.mesh.tris.len(), t0.elapsed());
    let p = c.mesh.verts[c.mesh.tris[1000][0] as usize];
    let t1 = Instant::now();
    let n = c.dab(p, 3.0, 5, mesh2step_core::math::V3::ZERO).len();
    println!("Tupfer: {n} Dreiecke {:?}", t1.elapsed());
    let t2 = Instant::now();
    c.refine(p, 3.0);
    println!("Verfeinern: {:?}, jetzt {} Dreiecke", t2.elapsed(), c.mesh.tris.len());
    let t3 = Instant::now();
    let n = c.fill(1000, 6, 30.0).len();
    println!("Füllen: {n} {:?}", t3.elapsed());
}
