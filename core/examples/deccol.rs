//! Debug: bemalte Datei für die Anzeige ausdünnen und als STL mit Farbindex (Attribut) speichern.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (meshes, colors, _) = mesh2step_core::paint::load_colored(std::path::Path::new(&a[1])).unwrap();
    let mut tris = Vec::new();
    let mut col = Vec::new();
    for m in &meshes {
        for (t, c) in m.tris.iter().zip(&m.col) {
            tris.push(t.map(|i| {
                let q = m.verts[i as usize];
                [q.x as f32, q.y as f32, q.z as f32]
            }));
            col.push(*c);
        }
    }
    let (d, src) = mesh2step_core::inspect::decimate_display(&tris, 250_000);
    let mut b = vec![0u8; 80];
    b.extend_from_slice(&(d.len() as u32).to_le_bytes());
    for (t, s) in d.iter().zip(&src) {
        b.extend_from_slice(&[0u8; 12]);
        for p in t {
            for x in p {
                b.extend_from_slice(&x.to_le_bytes());
            }
        }
        b.extend_from_slice(&(col[*s as usize] as u16).to_le_bytes());
    }
    std::fs::write(&a[2], b).unwrap();
    for (i, c) in colors.iter().enumerate() {
        println!("{i} {} {} {}", c[0], c[1], c[2]);
    }
}
