fn main() {
    let a: Vec<String> = std::env::args().collect();
    let p = mesh2step_core::preview_input(std::path::Path::new(&a[1])).unwrap();
    let r = mesh2step_core::printcheck::check(&p.tris, &Default::default());
    let mut out = String::new();
    for f in r.findings.iter().filter(|f| f.key == "detail" || f.key == "wall") {
        for &t in &f.tris { out.push_str(&format!("{} {}\n", t, f.key)); }
        let th = mesh2step_core::inspect::thickness_checked(&p.tris, 1.0);
        let mut lo = [f32::MAX; 3];
        let mut hi = [f32::MIN; 3];
        let mut ths: Vec<f32> = f.tris.iter().map(|&t| th[t as usize]).collect();
        ths.sort_by(|a, b| a.partial_cmp(b).unwrap());
        for &t in &f.tris {
            for q in &p.tris[t as usize] {
                for k in 0..3 { lo[k] = lo[k].min(q[k]); hi[k] = hi[k].max(q[k]); }
            }
        }
        eprintln!("{}: {} tris, dicke median {:.2}, bbox {:?}..{:?}", f.key, f.tris.len(), ths[ths.len() / 2], lo, hi);
        for &t in f.tris.iter().step_by((f.tris.len() / 6).max(1)).take(6) {
            let q = &p.tris[t as usize];
            eprintln!("   {:.1},{:.1},{:.1} th {:.2}", q[0][0], q[0][1], q[0][2], th[t as usize]);
        }
    }
    std::fs::write("/tmp/claude-0/thin_idx.txt", out).unwrap();
    let mut b = vec![0u8; 80];
    b.extend_from_slice(&(p.tris.len() as u32).to_le_bytes());
    for t in &p.tris {
        b.extend_from_slice(&[0u8; 12]);
        for q in t { for x in q { b.extend_from_slice(&x.to_le_bytes()); } }
        b.extend_from_slice(&[0u8; 2]);
    }
    std::fs::write("/tmp/claude-0/thin_mesh.stl", b).unwrap();
}
