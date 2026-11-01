//! Debug: Wandstärke-Berechnung zeitlich messen. thick <datei>
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let p = mesh2step_core::preview_input(std::path::Path::new(&a[1])).unwrap();
    let t1 = std::time::Instant::now();
    let (d, _) = mesh2step_core::inspect::decimate_display(&p.tris, 250_000);
    eprintln!("Anzeige: {} -> {} Dreiecke in {:.2} s", p.tris.len(), d.len(), t1.elapsed().as_secs_f64());
    if let Some(out) = a.get(2) {
        let mut b = vec![0u8; 80];
        b.extend_from_slice(&(d.len() as u32).to_le_bytes());
        for t in &d {
            b.extend_from_slice(&[0u8; 12]);
            for p in t {
                for x in p {
                    b.extend_from_slice(&x.to_le_bytes());
                }
            }
            b.extend_from_slice(&[0u8; 2]);
        }
        std::fs::write(out, b).unwrap();
        return;
    }
    let t0 = std::time::Instant::now();
    let th = mesh2step_core::inspect::thickness(&p.tris);
    let fin: Vec<f32> = th.iter().copied().filter(|x| x.is_finite()).collect();
    let mut s = fin.clone();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    eprintln!("{} Dreiecke, {:.1} s, endlich {}, Median {:.2}, 5% {:.2}", th.len(), t0.elapsed().as_secs_f64(), fin.len(), s.get(s.len() / 2).unwrap_or(&0.0), s.get(s.len() / 20).unwrap_or(&0.0));
}
