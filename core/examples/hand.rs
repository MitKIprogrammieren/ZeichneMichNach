//! Testlauf: cargo run --release --example hand -- bild.png ausgabe.svg [k]
use nz_core::{hand, vector};
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (w, h, img) = vector::decode_png(&std::fs::read(&a[1]).unwrap()).unwrap();
    let k: usize = a.get(3).and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut d = vector::Doc::new(w, h, img);
    let o = vector::Opts::default();
    let ho = hand::HandOpts::default();
    let t = std::time::Instant::now();
    hand::prepare(&mut d, &o, &ho, &[]);
    eprintln!("vorbereitet {:.2}s, {} Linien, Foto {:.0}%", t.elapsed().as_secs_f64(), d.hand.lines.len(), d.hand.photo * 100.0);
    hand::finish(&mut d, &o, k, &ho, &[]);
    let (p, n) = d.stats();
    eprintln!("fertig {:.2}s: {} Farben, {} Formen, {} Knoten, {} Verläufe", t.elapsed().as_secs_f64(), d.layers.len(), p, n, d.hand.grads.len());
    std::fs::write(&a[2], d.build_svg(&o, false)).unwrap();
}
