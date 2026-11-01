//! Debug: Druckbarkeit prüfen. printcheck <datei>
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let p = mesh2step_core::preview_input(std::path::Path::new(&a[1])).unwrap();
    let t0 = std::time::Instant::now();
    let r = mesh2step_core::printcheck::check(&p.tris, &Default::default());
    eprintln!("{} ({:?}) in {:.1} s", r.verdict, r.level, t0.elapsed().as_secs_f64());
    for f in &r.findings {
        eprintln!("  {:?} {}: {} [{}] {} Dreiecke", f.level, f.title, f.summary, f.badge, f.tris.len());
    }
}
