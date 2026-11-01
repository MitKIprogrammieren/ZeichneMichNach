use mesh2step_core::{paint, split};
use std::collections::HashMap;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let r: usize = a[2].parse().unwrap();
    let (raw, _, _) = paint::load_colored(std::path::Path::new(&a[1])).unwrap();
    let mut c = raw[0].clone();
    split::clean_specks(&mut c, 2.0);
    split::smooth_boundaries(&mut c, 0.3);
    let reg = split::regions(&c);
    let mut emap: HashMap<(u32, u32), Vec<u32>> = HashMap::new();
    for (t, tri) in c.tris.iter().enumerate() { for k in 0..3 { emap.entry((tri[k], tri[(k + 1) % 3])).or_default().push(t as u32); } }
    let (mut nb_other, mut none, mut dup) = (0, 0, 0);
    let mut outd: HashMap<u32, i32> = HashMap::new();
    let mut ind: HashMap<u32, i32> = HashMap::new();
    let mut same_dir = 0;
    for &t in &reg.list[r].tris {
        let tri = c.tris[t as usize];
        for k in 0..3 {
            let (x, y) = (tri[k], tri[(k + 1) % 3]);
            if emap.get(&(x, y)).map_or(0, |v| v.len()) > 1 { dup += 1; }
            match emap.get(&(y, x)) {
                Some(v) if v.iter().all(|&u| reg.of[u as usize] as usize == r) => {}
                Some(_) => { nb_other += 1; *outd.entry(x).or_default() += 1; *ind.entry(y).or_default() += 1; }
                None => { none += 1; *outd.entry(x).or_default() += 1; *ind.entry(y).or_default() += 1; if emap.contains_key(&(y,x)) {} ; if emap.get(&(x,y)).map_or(0,|v|v.len())>0 && c.tris.iter().any(|_|false) {} ; same_dir += 0; }
            }
        }
    }
    let unbalanced = outd.keys().chain(ind.keys()).filter(|v| outd.get(v).unwrap_or(&0) != ind.get(v).unwrap_or(&0)).count();
    println!("Bereich {r}: {} Dreiecke, Rand zu anderem Bereich {nb_other}, ohne Gegenkante {none}, doppelte gerichtete Kanten {dup}, unausgeglichene Ecken {unbalanced} {same_dir}", reg.list[r].tris.len());
}
