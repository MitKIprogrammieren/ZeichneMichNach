fn main() {
    for a in std::env::args().skip(1) {
        for p in mesh2step_core::io::plate_info(std::path::Path::new(&a)) {
            println!("Platte {} '{}' Ursprung {:?} Bild {} B: {:?}", p.id, p.name, p.origin, p.thumb.as_ref().map(|t| t.len()).unwrap_or(0), p.objects);
        }
        if let Ok(pl) = std::env::var("PLATE") {
            mesh2step_core::io::set_plate_filter(std::path::Path::new(&a), Some(mesh2step_core::io::PlateFilter { plate: pl.parse().ok(), hidden: vec![] }));
        }
        let ms = mesh2step_core::io::load_file(std::path::Path::new(&a)).unwrap();
        println!("{}: {} Objekte", a, ms.len());
        for m in &ms {
            let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
            for v in &m.verts { for k in 0..3 { lo[k] = lo[k].min(v.get(k)); hi[k] = hi[k].max(v.get(k)); } }
            if let Ok(d) = std::env::var("DUMP") {
                let mut b = vec![0u8; 80];
                b.extend((m.tris.len() as u32).to_le_bytes());
                for t in &m.tris {
                    b.extend([0u8; 12]);
                    for &i in t { let v = m.verts[i as usize]; for k in 0..3 { b.extend((v.get(k) as f32).to_le_bytes()); } }
                    b.extend([0u8; 2]);
                }
                let _ = std::fs::write(format!("{d}/{}.stl", m.name), b);
            }
            println!("  {:30} {:7} Dr.  x {:7.1}..{:7.1}  y {:7.1}..{:7.1}  z {:6.1}..{:6.1}", m.name, m.tris.len(), lo[0], hi[0], lo[1], hi[1], lo[2], hi[2]);
        }
    }
}
