//! Eigenes Mal-Werkzeug: Dreiecke mit einem kugelförmigen Pinsel einfärben,
//! Flächen füllen und die Kante des Pinsels durch Unterteilen der Dreiecke
//! glätten (Halbierung der längsten Kante, Netz bleibt geschlossen).

use crate::math::V3;
use crate::paint::ColoredMesh;
use std::collections::{HashMap, HashSet};

/// Räumliches Raster zum schnellen Finden der Dreiecke in der Nähe des Pinsels.
struct Grid {
    cell: f64,
    map: HashMap<(i32, i32, i32), Vec<u32>>,
}

impl Grid {
    fn key(&self, p: V3) -> (i32, i32, i32) {
        ((p.x / self.cell).floor() as i32, (p.y / self.cell).floor() as i32, (p.z / self.cell).floor() as i32)
    }

    fn insert(&mut self, m: &ColoredMesh, t: u32) {
        let tri = m.tris[t as usize];
        let (a, b, c) = (m.verts[tri[0] as usize], m.verts[tri[1] as usize], m.verts[tri[2] as usize]);
        let lo = self.key(a.min(b).min(c));
        let hi = self.key(a.max(b).max(c));
        // sehr große Dreiecke nur grob eintragen (Ecken und Mitte)
        if (hi.0 - lo.0 + 1) as i64 * (hi.1 - lo.1 + 1) as i64 * (hi.2 - lo.2 + 1) as i64 > 512 {
            for p in [a, b, c, (a + b + c) * (1.0 / 3.0)] {
                let k = self.key(p);
                self.map.entry(k).or_default().push(t);
            }
            return;
        }
        for x in lo.0..=hi.0 {
            for y in lo.1..=hi.1 {
                for z in lo.2..=hi.2 {
                    self.map.entry((x, y, z)).or_default().push(t);
                }
            }
        }
    }

    fn near(&self, c: V3, r: f64) -> Vec<u32> {
        let lo = self.key(c - V3 { x: r, y: r, z: r });
        let hi = self.key(c + V3 { x: r, y: r, z: r });
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for x in lo.0..=hi.0 {
            for y in lo.1..=hi.1 {
                for z in lo.2..=hi.2 {
                    if let Some(v) = self.map.get(&(x, y, z)) {
                        for &t in v {
                            if seen.insert(t) {
                                out.push(t);
                            }
                        }
                    }
                }
            }
        }
        out
    }
}

/// Ein Objekt zum Bemalen.
pub struct Canvas {
    pub mesh: ColoredMesh,
    grid: Grid,
    /// gerichtete Kante -> Dreieck (für Unterteilen und Füllen)
    emap: Option<HashMap<(u32, u32), u32>>,
    /// höchstens so viele Dreiecke durch Unterteilen
    pub max_tris: usize,
}

fn tri_normal(m: &ColoredMesh, t: u32) -> V3 {
    let [a, b, c] = m.tris[t as usize];
    let (a, b, c) = (m.verts[a as usize], m.verts[b as usize], m.verts[c as usize]);
    let n = (b - a).cross(c - a);
    if n.norm() > 0.0 {
        n.normalized()
    } else {
        V3::ZERO
    }
}

impl Canvas {
    pub fn new(mesh: ColoredMesh) -> Canvas {
        // Rastergröße: etwa die vierfache mittlere Kantenlänge
        let mut len = 0.0;
        let n = mesh.tris.len().min(20_000);
        let step = (mesh.tris.len() / n.max(1)).max(1);
        let mut cnt = 0;
        for t in mesh.tris.iter().step_by(step) {
            len += mesh.verts[t[0] as usize].dist(mesh.verts[t[1] as usize]);
            cnt += 1;
        }
        let avg = if cnt > 0 { len / cnt as f64 } else { 1.0 };
        let mut g = Grid { cell: (avg * 4.0).max(0.2), map: HashMap::new() };
        for t in 0..mesh.tris.len() as u32 {
            g.insert(&mesh, t);
        }
        let max_tris = (mesh.tris.len() * 4).clamp(400_000, 3_000_000);
        Canvas { mesh, grid: g, emap: None, max_tris }
    }

    fn emap(&mut self) -> &mut HashMap<(u32, u32), u32> {
        if self.emap.is_none() {
            let mut e = HashMap::with_capacity(self.mesh.tris.len() * 3);
            for (t, tri) in self.mesh.tris.iter().enumerate() {
                for k in 0..3 {
                    e.insert((tri[k], tri[(k + 1) % 3]), t as u32);
                }
            }
            self.emap = Some(e);
        }
        self.emap.as_mut().unwrap()
    }

    /// Häufigste Farbe (nach Fläche) – Grundfarbe für den Radierer.
    pub fn base_color(&self) -> u8 {
        let mut a: HashMap<u8, f64> = HashMap::new();
        for (t, tri) in self.mesh.tris.iter().enumerate() {
            let (p, q, r) = (self.mesh.verts[tri[0] as usize], self.mesh.verts[tri[1] as usize], self.mesh.verts[tri[2] as usize]);
            *a.entry(self.mesh.col[t]).or_default() += (q - p).cross(r - p).norm();
        }
        a.into_iter().max_by(|x, y| x.1.partial_cmp(&y.1).unwrap()).map(|x| x.0).unwrap_or(1)
    }

    /// Kante (a, b) des Dreiecks `t` halbieren (auch im Nachbarn). Rückgabe: neue Dreiecke.
    fn split_edge(&mut self, t: u32, k: usize) -> Vec<u32> {
        let tri = self.mesh.tris[t as usize];
        let (a, b, c) = (tri[k], tri[(k + 1) % 3], tri[(k + 2) % 3]);
        let u = self.emap().get(&(b, a)).copied();
        let m = self.mesh.verts.len() as u32;
        self.mesh.verts.push((self.mesh.verts[a as usize] + self.mesh.verts[b as usize]) * 0.5);
        let col_t = self.mesh.col[t as usize];
        let t2 = self.mesh.tris.len() as u32;
        self.mesh.tris[t as usize] = [a, m, c];
        self.mesh.tris.push([m, b, c]);
        self.mesh.col.push(col_t);
        let mut new = vec![t, t2];
        let e = self.emap();
        e.remove(&(a, b));
        e.insert((a, m), t);
        e.insert((m, c), t);
        e.insert((c, a), t);
        e.insert((m, b), t2);
        e.insert((b, c), t2);
        e.insert((c, m), t2);
        if let Some(u) = u {
            let ut = self.mesh.tris[u as usize];
            let j = (0..3).find(|&j| ut[j] == b && ut[(j + 1) % 3] == a).unwrap_or(0);
            let d = ut[(j + 2) % 3];
            let col_u = self.mesh.col[u as usize];
            let u2 = self.mesh.tris.len() as u32;
            self.mesh.tris[u as usize] = [b, m, d];
            self.mesh.tris.push([m, a, d]);
            self.mesh.col.push(col_u);
            let e = self.emap();
            e.remove(&(b, a));
            e.insert((b, m), u);
            e.insert((m, d), u);
            e.insert((d, b), u);
            e.insert((m, a), u2);
            e.insert((a, d), u2);
            e.insert((d, m), u2);
            new.push(u);
            new.push(u2);
            self.grid.insert(&self.mesh, u2);
        }
        self.grid.insert(&self.mesh, t2);
        new
    }

    fn inside(&self, t: u32, c: V3, r: f64) -> usize {
        self.mesh.tris[t as usize].iter().filter(|&&v| self.mesh.verts[v as usize].dist(c) <= r).count()
    }

    fn longest(&self, t: u32) -> (usize, f64) {
        let tri = self.mesh.tris[t as usize];
        (0..3)
            .map(|k| (k, self.mesh.verts[tri[k] as usize].dist(self.mesh.verts[tri[(k + 1) % 3] as usize])))
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .unwrap()
    }

    /// Dreiecke, die die Pinselkugel schneiden, unterteilen, bis die Kante fein genug ist.
    /// Rückgabe: Netz geändert.
    pub fn refine(&mut self, c: V3, r: f64) -> bool {
        let target = (r * 0.18).max(0.04);
        let mut queue: Vec<u32> = self.grid.near(c, r);
        let mut changed = false;
        let mut guard = 0usize;
        while let Some(t) = queue.pop() {
            guard += 1;
            if guard > 400_000 || self.mesh.tris.len() >= self.max_tris {
                break;
            }
            let n = self.inside(t, c, r);
            let (k, len) = self.longest(t);
            if len <= target || n == 3 {
                continue;
            }
            if n == 0 {
                // ganz außerhalb: nur unterteilen, wenn die Kugel das Dreieck trotzdem schneidet
                let tri = self.mesh.tris[t as usize];
                let cen = (self.mesh.verts[tri[0] as usize] + self.mesh.verts[tri[1] as usize] + self.mesh.verts[tri[2] as usize]) * (1.0 / 3.0);
                if cen.dist(c) > r + len * 0.5 {
                    continue;
                }
            }
            queue.extend(self.split_edge(t, k));
            changed = true;
        }
        changed
    }

    /// Mit der Pinselkugel einfärben. `normal`: Blickrichtung zur Fläche (Rückseiten dünner
    /// Teile bleiben unberührt). Rückgabe: geänderte Dreiecke.
    pub fn dab(&mut self, c: V3, r: f64, color: u8, normal: V3) -> Vec<u32> {
        let mut out = Vec::new();
        for t in self.grid.near(c, r) {
            if self.mesh.col[t as usize] == color {
                continue;
            }
            let tri = self.mesh.tris[t as usize];
            let cen = (self.mesh.verts[tri[0] as usize] + self.mesh.verts[tri[1] as usize] + self.mesh.verts[tri[2] as usize]) * (1.0 / 3.0);
            if cen.dist(c) > r {
                continue;
            }
            if normal.norm() > 0.0 && tri_normal(&self.mesh, t).dot(normal) < -0.25 {
                continue;
            }
            self.mesh.col[t as usize] = color;
            out.push(t);
        }
        out
    }

    /// Zusammenhängende Fläche gleicher Farbe füllen, begrenzt durch Kanten steiler als
    /// `max_deg` (180 = ganzer gleichfarbiger Bereich). Rückgabe: geänderte Dreiecke.
    pub fn fill(&mut self, start: u32, color: u8, max_deg: f64) -> Vec<u32> {
        if start as usize >= self.mesh.tris.len() {
            return Vec::new();
        }
        let from = self.mesh.col[start as usize];
        if from == color {
            return Vec::new();
        }
        let cos_lim = max_deg.min(180.0).to_radians().cos();
        self.emap();
        let e = self.emap.as_ref().unwrap();
        let mut seen = vec![false; self.mesh.tris.len()];
        let mut stack = vec![start];
        seen[start as usize] = true;
        let mut out = Vec::new();
        while let Some(t) = stack.pop() {
            out.push(t);
            let nt = tri_normal(&self.mesh, t);
            let tri = self.mesh.tris[t as usize];
            for k in 0..3 {
                let (a, b) = (tri[k], tri[(k + 1) % 3]);
                if let Some(&u) = e.get(&(b, a)) {
                    if seen[u as usize] || self.mesh.col[u as usize] != from {
                        continue;
                    }
                    if max_deg < 180.0 && nt.dot(tri_normal(&self.mesh, u)) < cos_lim {
                        continue;
                    }
                    seen[u as usize] = true;
                    stack.push(u);
                }
            }
        }
        for &t in &out {
            self.mesh.col[t as usize] = color;
        }
        out
    }

    /// Ganzes Objekt in einer Farbe.
    pub fn fill_all(&mut self, color: u8) {
        for c in self.mesh.col.iter_mut() {
            *c = color;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::v3;

    fn cube(n: usize) -> ColoredMesh {
        // Würfel 10 mm, je Seite n×n Quadrate
        let mut verts = Vec::new();
        let mut tris = Vec::new();
        let mut idx: HashMap<(i64, i64, i64), u32> = HashMap::new();
        let mut vid = |p: V3, verts: &mut Vec<V3>| -> u32 {
            let k = ((p.x * 1000.0).round() as i64, (p.y * 1000.0).round() as i64, (p.z * 1000.0).round() as i64);
            *idx.entry(k).or_insert_with(|| {
                verts.push(p);
                (verts.len() - 1) as u32
            })
        };
        let s = 10.0 / n as f64;
        for ax in 0..3 {
            for side in [0.0, 10.0] {
                for i in 0..n {
                    for j in 0..n {
                        let q = |a: f64, b: f64| match ax {
                            0 => v3(side, a, b),
                            1 => v3(b, side, a),
                            _ => v3(a, b, side),
                        };
                        let (a0, a1, b0, b1) = (i as f64 * s, (i + 1) as f64 * s, j as f64 * s, (j + 1) as f64 * s);
                        let p = [q(a0, b0), q(a1, b0), q(a1, b1), q(a0, b1)];
                        let id: Vec<u32> = p.iter().map(|&x| vid(x, &mut verts)).collect();
                        let flip = side == 0.0;
                        if flip {
                            tris.push([id[0], id[2], id[1]]);
                            tris.push([id[0], id[3], id[2]]);
                        } else {
                            tris.push([id[0], id[1], id[2]]);
                            tris.push([id[0], id[2], id[3]]);
                        }
                    }
                }
            }
        }
        let col = vec![1; tris.len()];
        ColoredMesh { name: "w".into(), verts, tris, col }
    }

    fn closed(m: &ColoredMesh) -> bool {
        let mut e: HashMap<(u32, u32), i32> = HashMap::new();
        for t in &m.tris {
            for k in 0..3 {
                *e.entry((t[k], t[(k + 1) % 3])).or_default() += 1;
            }
        }
        e.iter().all(|(&(a, b), &n)| n == 1 && e.get(&(b, a)) == Some(&1))
    }

    #[test]
    fn refine_keeps_mesh_closed_and_paints_round_spot() {
        let mut c = Canvas::new(cube(2));
        assert!(closed(&c.mesh));
        let center = v3(5.0, 5.0, 10.0);
        assert!(c.refine(center, 2.0));
        assert!(closed(&c.mesh), "Netz nach dem Unterteilen offen");
        let ch = c.dab(center, 2.0, 3, v3(0.0, 0.0, 1.0));
        assert!(!ch.is_empty());
        // bemalte Fläche nahe π·r² = 12,6 mm²
        let mut area = 0.0;
        for (t, tri) in c.mesh.tris.iter().enumerate() {
            if c.mesh.col[t] == 3 {
                let (a, b, d) = (c.mesh.verts[tri[0] as usize], c.mesh.verts[tri[1] as usize], c.mesh.verts[tri[2] as usize]);
                area += (b - a).cross(d - a).norm() * 0.5;
            }
        }
        assert!((area - 12.57).abs() < 2.5, "Fläche {area}");
        // Unterseite (Normale gegen den Pinsel) bleibt unbemalt
        assert!(c.mesh.tris.iter().enumerate().all(|(t, tri)| c.mesh.col[t] != 3 || c.mesh.verts[tri[0] as usize].z > 9.0));
    }

    #[test]
    fn fill_stops_at_edges() {
        let mut c = Canvas::new(cube(3));
        // Oberseite füllen: nur die 18 Dreiecke der Deckfläche
        let top = c.mesh.tris.iter().position(|t| t.iter().all(|&v| c.mesh.verts[v as usize].z > 9.99)).unwrap() as u32;
        let ch = c.fill(top, 2, 30.0);
        assert_eq!(ch.len(), 18);
        // ohne Winkelgrenze: alles
        let ch = c.fill(0, 4, 180.0);
        assert_eq!(ch.len(), c.mesh.tris.len() - 18);
        assert_eq!(c.base_color(), 4);
    }
}
