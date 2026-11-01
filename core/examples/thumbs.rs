//! Vorschaubilder aller Muster als PPM in einen Ordner schreiben.
use mesh2step_core::texture::{self, Pattern};
use std::io::Write;

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let n = 160;
    for (i, p) in Pattern::ALL.iter().enumerate() {
        let px = texture::thumbnail(*p, n);
        let mut f = std::fs::File::create(format!("{dir}/{i:02}.ppm")).unwrap();
        write!(f, "P6\n{n} {n}\n255\n").unwrap();
        for c in px.chunks(4) {
            f.write_all(&c[..3]).unwrap();
        }
        println!("{i:02} {}", p.name_de());
    }
}
