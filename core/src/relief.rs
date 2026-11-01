//! 3D-Aufbau aus der Vektorgrafik: Farbschichten auf einer Grundschicht, außen
//! herum ein eigener Rand, der die Lücke zwischen Grundschicht und Umriss füllt.
//! Ausgabe als Teile (für 3MF) oder als STEP mit echten Flächen.
//!
//!   Schnitt:  [ Farbschichten über die volle Breite bis zum Umriss      ]
//!             [R][ Grundschicht = Umriss − Randbreite                 ][R]

use crate::split::{Part, PartKind};
use crate::step::{esc, r, timestamp, W};
use crate::vector::{extrude_rings, flatten, fit_loop, to_cubics, trace_mask, Doc, Opts, Pt, Shape};
use std::fmt::Write as _;

#[derive(Clone, Debug, PartialEq)]
pub struct Build3d {
    /// Breite des ganzen Bildes in mm
    pub width_mm: f64,
    /// true = Höhe je Farbe, false = alle Farbschichten gleich hoch
    pub stepped: bool,
    /// Höhe der Farbschichten je Farbgruppe (mm, über der Grundschicht)
    pub heights: Vec<f64>,
    /// Höhe aller Farbschichten, wenn nicht gestuft
    pub uniform_mm: f64,
    pub base: bool,
    pub base_mm: f64,
    pub base_color: [u8; 3],
    pub rim: bool,
    /// Randbreite = so viel ist die Grundschicht kleiner als der Umriss
    pub rim_w_mm: f64,
    /// Höhe des Rands (Standard wie die Grundschicht)
    pub rim_h_mm: f64,
    pub rim_color: [u8; 3],
    /// Namen der Farbgruppen (für die Körper), leer = „Farbe n“
    pub names: Vec<String>,
}

impl Default for Build3d {
    fn default() -> Self {
        Build3d {
            width_mm: 100.0,
            stepped: false,
            heights: Vec::new(),
            uniform_mm: 0.6,
            base: true,
            base_mm: 1.6,
            base_color: [30, 30, 30],
            rim: true,
            rim_w_mm: 1.0,
            rim_h_mm: 1.6,
            rim_color: [240, 240, 240],
            names: Vec::new(),
        }
    }
}

/// Ein Körper: Formen (Quellbild-Koordinaten), von z0 bis z1 hochgezogen.
#[derive(Clone, Debug)]
pub struct Body {
    pub name: String,
    pub color: [u8; 3],
    pub z0: f64,
    pub z1: f64,
    pub shapes: Vec<Shape>,
}

/// Maske → glatte Formen (Quellbild-Koordinaten).
fn mask_shapes(doc: &Doc, m: &[bool], o: &Opts) -> Vec<Shape> {
    trace_mask(m, doc.w, doc.h).iter().filter_map(|c| fit_loop(c, o, doc.sc)).collect()
}

/// Körper bauen: Grundschicht, Rand, Farbschichten.
pub fn bodies(doc: &Doc, o: &Opts, b: &Build3d) -> Vec<Body> {
    let (w, h) = (doc.w, doc.h);
    let n = w * h;
    let mut out = Vec::new();
    let outline: Vec<bool> = if doc.g.len() == n { doc.g.iter().map(|&v| v >= 0).collect() } else { vec![true; n] };
    let base_top = if b.base { b.base_mm.max(0.0) } else { 0.0 };
    if b.base && b.base_mm > 0.0 {
        let ppm = w as f64 / b.width_mm.max(1.0);
        let rim_px = if b.rim { (b.rim_w_mm * ppm) as f32 } else { 0.0 };
        let inner: Vec<bool> = if rim_px > 0.0 {
            let d = crate::hand::dist_in(&outline, w, h);
            d.iter().map(|&v| v > rim_px).collect()
        } else {
            outline.clone()
        };
        out.push(Body { name: "Grundschicht".into(), color: b.base_color, z0: 0.0, z1: base_top, shapes: mask_shapes(doc, &inner, o) });
        if b.rim && rim_px > 0.0 {
            let ring: Vec<bool> = (0..n).map(|p| outline[p] && !inner[p]).collect();
            out.push(Body { name: "Rand".into(), color: b.rim_color, z0: 0.0, z1: b.rim_h_mm.max(0.1), shapes: mask_shapes(doc, &ring, o) });
        }
    }
    // Farbschichten auf der Grundschicht – gleiche Farbe = ein Körper
    let mut by_group: Vec<(usize, Vec<Shape>)> = Vec::new();
    for l in &doc.layers {
        match by_group.iter_mut().find(|x| x.0 == l.idx) {
            Some(e) => e.1.extend(l.subs.iter().cloned()),
            None => by_group.push((l.idx, l.subs.clone())),
        }
    }
    for (gi, shapes) in by_group {
        let hgt = if b.stepped { b.heights.get(gi).copied().unwrap_or(b.uniform_mm) } else { b.uniform_mm };
        if hgt <= 0.0 {
            continue;
        }
        let name = b.names.get(gi).cloned().unwrap_or_else(|| format!("Farbe {}", gi + 1));
        out.push(Body { name, color: doc.gcol(gi), z0: base_top, z1: base_top + hgt, shapes });
    }
    out
}

/// Teile für 3MF (Dreiecksnetze) und ihre Farben (Index = Filament − 1).
pub fn build_parts(doc: &Doc, o: &Opts, b: &Build3d) -> (Vec<Part>, Vec<[u8; 3]>) {
    let k = b.width_mm / doc.src_w.max(1) as f64;
    let hh = doc.src_h as f64;
    let mut parts = Vec::new();
    let mut colors: Vec<[u8; 3]> = Vec::new();
    for bd in bodies(doc, o, b) {
        let ci = match colors.iter().position(|c| *c == bd.color) {
            Some(i) => i + 1,
            None => {
                colors.push(bd.color);
                colors.len()
            }
        } as u8;
        let mut rings: Vec<Vec<Pt>> = Vec::new();
        for sp in &bd.shapes {
            for p in flatten(sp, 0.25) {
                rings.push(p.iter().map(|q| [q[0] * k, (hh - q[1]) * k]).collect());
            }
        }
        if rings.is_empty() || bd.z1 <= bd.z0 {
            continue;
        }
        let (verts, tris) = extrude_rings(&rings, bd.z0, bd.z1);
        if tris.is_empty() {
            continue;
        }
        let kind = if bd.name == "Grundschicht" { PartKind::Body } else { PartKind::ColorPart };
        let mut p = Part { name: bd.name, col: vec![ci; tris.len()], verts, tris, color: ci, group: 0, kind, ..Default::default() };
        p.update_stats();
        parts.push(p);
    }
    (parts, colors)
}

// ---------------------------------------------------------------------------
// STEP direkt aus den Kurven: Deckel und Boden eben, Seiten aus Geraden als
// Ebene, aus Bézierkurven als exakte Extrusionsfläche – keine Dreiecke.

#[derive(Clone, Copy)]
enum Seg {
    L(Pt, Pt),
    C(Pt, Pt, Pt, Pt),
}

impl Seg {
    fn a(&self) -> Pt {
        match *self {
            Seg::L(a, _) | Seg::C(a, _, _, _) => a,
        }
    }
    fn rev(&self) -> Seg {
        match *self {
            Seg::L(a, b) => Seg::L(b, a),
            Seg::C(a, b, c, d) => Seg::C(d, c, b, a),
        }
    }
    fn sample(&self, n: usize) -> Vec<Pt> {
        match *self {
            Seg::L(a, _) => vec![a],
            Seg::C(a, b, c, d) => (0..n)
                .map(|i| {
                    let t = i as f64 / n as f64;
                    let u = 1.0 - t;
                    let (k0, k1, k2, k3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                    [k0 * a[0] + k1 * b[0] + k2 * c[0] + k3 * d[0], k0 * a[1] + k1 * b[1] + k2 * c[1] + k3 * d[1]]
                })
                .collect(),
        }
    }
}

struct Ring {
    segs: Vec<Seg>,
    poly: Vec<Pt>,
    area: f64,
}

fn ring_of(segs: Vec<Seg>) -> Option<Ring> {
    if segs.is_empty() {
        return None;
    }
    let poly: Vec<Pt> = segs.iter().flat_map(|s| s.sample(8)).collect();
    if poly.len() < 3 {
        return None;
    }
    let mut a = 0.0;
    for i in 0..poly.len() {
        let (p, q) = (poly[i], poly[(i + 1) % poly.len()]);
        a += p[0] * q[1] - q[0] * p[1];
    }
    Some(Ring { segs, poly, area: a / 2.0 })
}

fn inside(p: Pt, poly: &[Pt]) -> bool {
    let mut c = false;
    let n = poly.len();
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
            c = !c;
        }
        j = i;
    }
    c
}

/// Formen eines Körpers in Millimeter-Ringe (y nach oben) zerlegen und zu
/// Stücken (Außenring + Löcher) ordnen. Außen gegen, Löcher im Uhrzeigersinn.
fn lumps(shapes: &[Shape], k: f64, hh: f64) -> Vec<(Ring, Vec<Ring>)> {
    let mm = |p: Pt| [p[0] * k, (hh - p[1]) * k];
    let mut rings: Vec<Ring> = Vec::new();
    for sp in shapes {
        for c in to_cubics(sp) {
            let mut segs = Vec::new();
            let mut cur = mm(c.m);
            for (kind, v) in &c.segs {
                if *kind == 'L' {
                    let p = mm([v[0], v[1]]);
                    if (p[0] - cur[0]).hypot(p[1] - cur[1]) > 1e-4 {
                        segs.push(Seg::L(cur, p));
                    }
                    cur = p;
                } else {
                    let (b, cc, d) = (mm([v[0], v[1]]), mm([v[2], v[3]]), mm([v[4], v[5]]));
                    if (d[0] - cur[0]).hypot(d[1] - cur[1]) > 1e-4 || (b[0] - cur[0]).hypot(b[1] - cur[1]) > 1e-4 {
                        segs.push(Seg::C(cur, b, cc, d));
                    }
                    cur = d;
                }
            }
            // schließen
            let start = mm(c.m);
            if (start[0] - cur[0]).hypot(start[1] - cur[1]) > 1e-4 {
                segs.push(Seg::L(cur, start));
            }
            if let Some(r) = ring_of(segs) {
                if r.area.abs() > 1e-4 {
                    rings.push(r);
                }
            }
        }
    }
    // Verschachtelung bestimmen
    let n = rings.len();
    let depth: Vec<usize> = (0..n)
        .map(|i| {
            let p = rings[i].poly[0];
            (0..n).filter(|&j| j != i && rings[j].area.abs() > rings[i].area.abs() && inside(p, &rings[j].poly)).count()
        })
        .collect();
    let mut out: Vec<(Ring, Vec<Ring>)> = Vec::new();
    let mut outer_idx: Vec<Option<usize>> = vec![None; n];
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|a, b| rings[*b].area.abs().partial_cmp(&rings[*a].area.abs()).unwrap());
    let mut taken: Vec<Option<Ring>> = rings.into_iter().map(Some).collect();
    // Außenringe (gerade Tiefe)
    for &i in &order {
        if depth[i] % 2 == 0 {
            let mut r = taken[i].take().unwrap();
            if r.area < 0.0 {
                r.segs = r.segs.iter().rev().map(|s| s.rev()).collect();
                r.poly.reverse();
                r.area = -r.area;
            }
            outer_idx[i] = Some(out.len());
            out.push((r, Vec::new()));
        }
    }
    // Löcher (ungerade Tiefe) dem kleinsten umschließenden Außenring zuordnen
    for &i in order.iter().rev() {
        if depth[i] % 2 == 1 {
            let mut r = taken[i].take().unwrap();
            let p = r.poly[0];
            let parent = out.iter().enumerate().filter(|(_, o)| o.0.area > r.area.abs() && inside(p, &o.0.poly)).min_by(|a, b| a.1 .0.area.partial_cmp(&b.1 .0.area).unwrap()).map(|x| x.0);
            if let Some(pi) = parent {
                if r.area > 0.0 {
                    r.segs = r.segs.iter().rev().map(|s| s.rev()).collect();
                    r.poly.reverse();
                    r.area = -r.area;
                }
                out[pi].1.push(r);
            }
        }
    }
    out
}

struct StepCtx {
    w: W,
}

impl StepCtx {
    fn p3(&mut self, p: Pt, z: f64) -> usize {
        self.w.pt(crate::math::v3(p[0], p[1], z))
    }
    fn vertex(&mut self, p: Pt, z: f64) -> usize {
        let pt = self.p3(p, z);
        self.w.add(format!("VERTEX_POINT('',#{pt})"))
    }
    fn curve(&mut self, s: &Seg, z: f64) -> usize {
        match *s {
            Seg::L(a, b) => {
                let pt = self.p3(a, z);
                let d = self.w.dir(crate::math::v3(b[0] - a[0], b[1] - a[1], 0.0));
                let v = self.w.add(format!("VECTOR('',#{d},1.)"));
                self.w.add(format!("LINE('',#{pt},#{v})"))
            }
            Seg::C(a, b, c, d) => {
                let ids: Vec<String> = [a, b, c, d].iter().map(|&p| format!("#{}", self.p3(p, z))).collect();
                self.w.add(format!("B_SPLINE_CURVE_WITH_KNOTS('',3,({}),.UNSPECIFIED.,.F.,.F.,(4,4),(0.,1.),.UNSPECIFIED.)", ids.join(",")))
            }
        }
    }
}

/// Einen Körper als geschlossene Schalen schreiben. Rückgabe: Ids der Festkörper.
fn write_body(cx: &mut StepCtx, bd: &Body, k: f64, hh: f64) -> Vec<usize> {
    let (z0, z1) = (bd.z0, bd.z1);
    let hgt = z1 - z0;
    let mut solids = Vec::new();
    for (outer, holes) in lumps(&bd.shapes, k, hh) {
        let mut faces: Vec<usize> = Vec::new();
        let mut top_bounds: Vec<String> = Vec::new();
        let mut bot_bounds: Vec<String> = Vec::new();
        for (ri, ring) in std::iter::once(&outer).chain(holes.iter()).enumerate() {
            let ns = ring.segs.len();
            // Ecken unten/oben je Segmentanfang, senkrechte Kanten
            let vb: Vec<usize> = ring.segs.iter().map(|s| cx.vertex(s.a(), z0)).collect();
            let vt: Vec<usize> = ring.segs.iter().map(|s| cx.vertex(s.a(), z1)).collect();
            let ve: Vec<usize> = (0..ns)
                .map(|i| {
                    let pt = cx.p3(ring.segs[i].a(), z0);
                    let d = cx.w.dir(crate::math::v3(0.0, 0.0, 1.0));
                    let v = cx.w.add(format!("VECTOR('',#{d},1.)"));
                    let ln = cx.w.add(format!("LINE('',#{pt},#{v})"));
                    cx.w.add(format!("EDGE_CURVE('',#{},#{},#{ln},.T.)", vb[i], vt[i]))
                })
                .collect();
            let mut eb = Vec::with_capacity(ns);
            let mut et = Vec::with_capacity(ns);
            for i in 0..ns {
                let j = (i + 1) % ns;
                let s = ring.segs[i];
                let cb = cx.curve(&s, z0);
                let ct = cx.curve(&s, z1);
                let e0 = cx.w.add(format!("EDGE_CURVE('',#{},#{},#{cb},.T.)", vb[i], vb[j]));
                let e1 = cx.w.add(format!("EDGE_CURVE('',#{},#{},#{ct},.T.)", vt[i], vt[j]));
                eb.push(e0);
                et.push(e1);
                // Seitenfläche: unten vorwärts, rechts hoch, oben zurück, links runter
                let oe = |cx: &mut StepCtx, e: usize, f: bool| cx.w.add(format!("ORIENTED_EDGE('',*,*,#{e},{})", if f { ".T." } else { ".F." }));
                let l = [oe(cx, e0, true), oe(cx, ve[j], true), oe(cx, e1, false), oe(cx, ve[i], false)];
                let lp = cx.w.add(format!("EDGE_LOOP('',(#{},#{},#{},#{}))", l[0], l[1], l[2], l[3]));
                let fb = cx.w.add(format!("FACE_OUTER_BOUND('',#{lp},.T.)"));
                let surf = match s {
                    Seg::L(a, b) => {
                        let t = crate::math::v3(b[0] - a[0], b[1] - a[1], 0.0);
                        let nrm = crate::math::v3(t.y, -t.x, 0.0);
                        let ax = cx.w.ax2(crate::math::v3(a[0], a[1], z0), nrm, t);
                        cx.w.add(format!("PLANE('',#{ax})"))
                    }
                    Seg::C(..) => {
                        let d = cx.w.dir(crate::math::v3(0.0, 0.0, 1.0));
                        let v = cx.w.add(format!("VECTOR('',#{d},{})", r(hgt)));
                        cx.w.add(format!("SURFACE_OF_LINEAR_EXTRUSION('',#{cb},#{v})"))
                    }
                };
                faces.push(cx.w.add(format!("ADVANCED_FACE('',(#{fb}),#{surf},.T.)")));
            }
            // Deckel: Ring vorwärts; Boden: rückwärts (Normale nach unten)
            let tl: Vec<String> = et.iter().map(|&e| format!("#{}", cx.w.add(format!("ORIENTED_EDGE('',*,*,#{e},.T.)")))).collect();
            let bl: Vec<String> = eb.iter().rev().map(|&e| format!("#{}", cx.w.add(format!("ORIENTED_EDGE('',*,*,#{e},.F.)")))).collect();
            let tlp = cx.w.add(format!("EDGE_LOOP('',({}))", tl.join(",")));
            let blp = cx.w.add(format!("EDGE_LOOP('',({}))", bl.join(",")));
            let kind = if ri == 0 { "FACE_OUTER_BOUND" } else { "FACE_BOUND" };
            top_bounds.push(format!("#{}", cx.w.add(format!("{kind}('',#{tlp},.T.)"))));
            bot_bounds.push(format!("#{}", cx.w.add(format!("{kind}('',#{blp},.T.)"))));
        }
        let o = outer.poly[0];
        let ax_t = cx.w.ax2(crate::math::v3(o[0], o[1], z1), crate::math::v3(0.0, 0.0, 1.0), crate::math::v3(1.0, 0.0, 0.0));
        let pl_t = cx.w.add(format!("PLANE('',#{ax_t})"));
        faces.push(cx.w.add(format!("ADVANCED_FACE('',({}),#{pl_t},.T.)", top_bounds.join(","))));
        let ax_b = cx.w.ax2(crate::math::v3(o[0], o[1], z0), crate::math::v3(0.0, 0.0, -1.0), crate::math::v3(1.0, 0.0, 0.0));
        let pl_b = cx.w.add(format!("PLANE('',#{ax_b})"));
        faces.push(cx.w.add(format!("ADVANCED_FACE('',({}),#{pl_b},.T.)", bot_bounds.join(","))));
        let fl: Vec<String> = faces.iter().map(|f| format!("#{f}")).collect();
        let sh = cx.w.add(format!("CLOSED_SHELL('',({}))", fl.join(",")));
        solids.push(cx.w.add(format!("MANIFOLD_SOLID_BREP('{}',#{sh})", esc(&bd.name))));
    }
    solids
}

/// Körper als STEP (AP214) mit echten Flächen und Farben.
pub fn step_from_bodies(bodies: &[Body], width_mm: f64, src_w: usize, src_h: usize, name: &str, file_name: &str) -> String {
    let k = width_mm / src_w.max(1) as f64;
    let hh = src_h as f64;
    let mut cx = StepCtx { w: W { out: String::new(), id: 0 } };
    let w = &mut cx.w;
    let ctx_app = w.add("APPLICATION_CONTEXT('core data for automotive mechanical design processes')");
    w.add(format!("APPLICATION_PROTOCOL_DEFINITION('international standard','automotive_design',2000,#{ctx_app})"));
    let pctx = w.add(format!("PRODUCT_CONTEXT('',#{ctx_app},'mechanical')"));
    let pdctx = w.add(format!("PRODUCT_DEFINITION_CONTEXT('part definition',#{ctx_app},'design')"));
    let prod = w.add(format!("PRODUCT('{0}','{0}','',(#{pctx}))", esc(name)));
    w.add(format!("PRODUCT_RELATED_PRODUCT_CATEGORY('part',$,(#{prod}))"));
    let pdf = w.add(format!("PRODUCT_DEFINITION_FORMATION('','',#{prod})"));
    let pd = w.add(format!("PRODUCT_DEFINITION('design','',#{pdf},#{pdctx})"));
    let pds = w.add(format!("PRODUCT_DEFINITION_SHAPE('','',#{pd})"));
    let mm = w.add("(LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT(.MILLI.,.METRE.))");
    let rad = w.add("(NAMED_UNIT(*) PLANE_ANGLE_UNIT() SI_UNIT($,.RADIAN.))");
    let sr = w.add("(NAMED_UNIT(*) SI_UNIT($,.STERADIAN.) SOLID_ANGLE_UNIT())");
    let unc = w.add(format!("UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE(1.E-03),#{mm},'distance_accuracy_value','confusion accuracy')"));
    let ctx = w.add(format!(
        "(GEOMETRIC_REPRESENTATION_CONTEXT(3) GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT((#{unc})) GLOBAL_UNIT_ASSIGNED_CONTEXT((#{mm},#{rad},#{sr})) REPRESENTATION_CONTEXT('Context3D','3D Context'))"
    ));
    let origin = w.ax2(crate::math::v3(0.0, 0.0, 0.0), crate::math::v3(0.0, 0.0, 1.0), crate::math::v3(1.0, 0.0, 0.0));
    let mut items = vec![origin];
    let mut styled = Vec::new();
    for bd in bodies {
        let sids = write_body(&mut cx, bd, k, hh);
        if sids.is_empty() {
            continue;
        }
        let w = &mut cx.w;
        let c = bd.color;
        let col = w.add(format!("COLOUR_RGB('',{},{},{})", r(c[0] as f64 / 255.0), r(c[1] as f64 / 255.0), r(c[2] as f64 / 255.0)));
        let fc = w.add(format!("FILL_AREA_STYLE_COLOUR('',#{col})"));
        let fa = w.add(format!("FILL_AREA_STYLE('',(#{fc}))"));
        let ss = w.add(format!("SURFACE_STYLE_FILL_AREA(#{fa})"));
        let sss = w.add(format!("SURFACE_SIDE_STYLE('',(#{ss}))"));
        let su = w.add(format!("SURFACE_STYLE_USAGE(.BOTH.,#{sss})"));
        let psa = w.add(format!("PRESENTATION_STYLE_ASSIGNMENT((#{su}))"));
        for s in sids {
            items.push(s);
            styled.push(w.add(format!("STYLED_ITEM('color',(#{psa}),#{s})")));
        }
    }
    let w = &mut cx.w;
    let it: Vec<String> = items.iter().map(|i| format!("#{i}")).collect();
    let rep = w.add(format!("ADVANCED_BREP_SHAPE_REPRESENTATION('{}',({}),#{ctx})", esc(name), it.join(",")));
    w.add(format!("SHAPE_DEFINITION_REPRESENTATION(#{pds},#{rep})"));
    if !styled.is_empty() {
        let st: Vec<String> = styled.iter().map(|i| format!("#{i}")).collect();
        w.add(format!("MECHANICAL_DESIGN_GEOMETRIC_PRESENTATION_REPRESENTATION('',({}),#{ctx})", st.join(",")));
    }
    let mut s = String::with_capacity(w.out.len() + 1024);
    s.push_str("ISO-10303-21;\nHEADER;\n");
    let _ = writeln!(s, "FILE_DESCRIPTION(('ZeichneMichNach: aus einer Vektorgrafik hochgezogene Koerper'),'2;1');");
    let _ = writeln!(s, "FILE_NAME('{}','{}',(''),(''),'ZeichneMichNach 1.0','ZeichneMichNach','');", esc(file_name), timestamp());
    s.push_str("FILE_SCHEMA(('AUTOMOTIVE_DESIGN { 1 0 10303 214 1 1 1 1 }'));\nENDSEC;\nDATA;\n");
    s.push_str(&w.out);
    s.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
    s
}
