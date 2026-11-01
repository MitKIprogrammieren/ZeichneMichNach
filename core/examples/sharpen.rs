//! Debug: Farbgrenzen scharf nachziehen und Zeit messen. sharpen <3mf> [stärke]
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (mut ms, _, _) = mesh2step_core::paint::load_colored(std::path::Path::new(&a[1])).unwrap();
    let s: f64 = a.get(2).and_then(|x| x.parse().ok()).unwrap_or(0.2);
    for m in ms.iter_mut() {
        let t0 = std::time::Instant::now();
        let n0 = m.tris.len();
        let k = mesh2step_core::split::sharpen_boundaries(m, s);
        eprintln!("{}: {} -> {} Dreiecke, {} geteilt, {:.2} s", m.name, n0, m.tris.len(), k, t0.elapsed().as_secs_f64());
    }
}
