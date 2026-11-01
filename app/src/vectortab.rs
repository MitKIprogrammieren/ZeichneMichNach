//! Bereich „Vektorix“: Bilder in Vektorgrafiken umwandeln.

use crate::theme::{self, Icon, Settings, Tok};
use eframe::egui::{self, Color32, ColorImage, Pos2, Rect, RichText, Sense, Stroke, TextureHandle, TextureOptions, Vec2};
use nz_core::hand::{self, HandOpts};
use nz_core::vector::{self, Doc, Edit, Opts, Shape, Unit};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Bereiche des Menüs „Ergebnis verfeinern“
#[derive(Clone, Copy, PartialEq, Eq)]
enum Menu {
    Main,
    Hand,
    Background,
    Palette,
    Regions,
    Edges,
    Curves,
    Symmetry,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Compare,
    Vector,
    Palette,
    Wire,
    Original,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Fmt {
    Svg,
    Pdf,
    Dxf,
    Eps,
    Png,
    Mf3,
}

impl Fmt {
    fn ext(self) -> &'static str {
        match self {
            Fmt::Svg => "svg",
            Fmt::Pdf => "pdf",
            Fmt::Dxf => "dxf",
            Fmt::Eps => "eps",
            Fmt::Png => "png",
            Fmt::Mf3 => "3mf",
        }
    }
}

struct Rendered {
    gen: u64,
    vec: ColorImage,
    wire: ColorImage,
    pal: ColorImage,
}

#[derive(Default)]
struct Shared {
    doc: Option<Arc<Doc>>,
    path: Option<PathBuf>,
    name: String,
    busy: Option<String>,
    error: Option<String>,
    info: Option<String>,
    gen: u64,
    rendered: Option<Rendered>,
    new_src: bool,
    /// Ergebnis von „Nachzeichnen wie von Hand“ (erkannte Schriften, Hinweise)
    hand_info: Option<crate::pipeline::Info>,
}

struct Tex {
    orig: TextureHandle,
    vec: Option<TextureHandle>,
    wire: Option<TextureHandle>,
    pal: Option<TextureHandle>,
    sel: Option<TextureHandle>,
}

pub struct VectorTab {
    sh: Arc<Mutex<Shared>>,
    o: Opts,
    k: usize,
    view: View,
    split: f32,
    zoom: f32,
    pan: Vec2,
    drag_split: bool,
    tex: Option<Tex>,
    tex_gen: u64,
    sel: Option<(usize, usize)>,
    sel_info: Option<(usize, i16, Vec<i16>)>,
    pick: Option<usize>,
    fmt: Fmt,
    mm: bool,
    width_mm: f64,
    width_px: f64,
    height_3d: f64,
    base_3d: f64,
    show_opts: bool,
    menu: Menu,
    /// „Nachzeichnen wie von Hand“ an/aus und seine Schalter
    hand_on: bool,
    hand: HandOpts,
}

impl Default for VectorTab {
    fn default() -> Self {
        VectorTab {
            sh: Arc::new(Mutex::new(Shared::default())),
            o: Opts::default(),
            k: 0,
            view: View::Compare,
            split: 0.5,
            zoom: 1.0,
            pan: Vec2::ZERO,
            drag_split: false,
            tex: None,
            tex_gen: 0,
            sel: None,
            sel_info: None,
            pick: None,
            fmt: Fmt::Svg,
            mm: true,
            width_mm: 100.0,
            width_px: 0.0,
            height_3d: 2.0,
            base_3d: 0.0,
            show_opts: false,
            menu: Menu::Main,
            hand_on: true,
            hand: HandOpts::default(),
        }
        .with_test_menu()
    }
}

impl VectorTab {
    /// Für Bildschirmfotos im Test: Menü über ZMN_MENU=hand vorwählen.
    fn with_test_menu(mut self) -> Self {
        if std::env::var("ZMN_MENU").as_deref() == Ok("hand") {
            self.menu = Menu::Hand;
        }
        self
    }
}

pub fn is_image(p: &Path) -> bool {
    matches!(
        p.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).as_deref(),
        Some("png") | Some("jpg") | Some("jpeg") | Some("webp") | Some("gif") | Some("bmp")
    )
}

/// „222 k Dreiecke“
pub fn fmt_count(n: usize, what: &str) -> String {
    if n >= 10_000 {
        format!("{} k {what}", (n as f64 / 1000.0).round() as usize)
    } else {
        format!("{n} {what}")
    }
}

fn to_image(w: usize, h: usize, rgba: &[u8]) -> ColorImage {
    ColorImage::from_rgba_unmultiplied([w, h], rgba)
}

fn render_all(d: &Doc, o: &Opts, gen: u64, dark: bool) -> Rendered {
    let long = 1400.0f64;
    let s = (long / d.src_w.max(d.src_h).max(1) as f64).min(4.0);
    let (w, h) = (((d.src_w as f64 * s).round() as usize).max(1), ((d.src_h as f64 * s).round() as usize).max(1));
    let vec = d.render_rgba(o, w, h, None);
    let wc = if dark { [230, 235, 241] } else { [20, 26, 35] };
    let mut ow = o.clone();
    ow.aa = true;
    let wire = d.render_rgba(&ow, w, h, Some(wc));
    let mut pal = vec![0u8; d.w * d.h * 4];
    let cols: Vec<[u8; 3]> = (0..d.groups.len()).map(|i| d.gcol(i)).collect();
    for (p, &g) in d.g.iter().enumerate() {
        if g >= 0 {
            let c = cols[g as usize];
            pal[p * 4..p * 4 + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
        }
    }
    Rendered { gen, vec: to_image(w, h, &vec), wire: to_image(w, h, &wire), pal: to_image(d.w, d.h, &pal) }
}

impl VectorTab {
    pub fn load(&mut self, path: PathBuf, ctx: &egui::Context) {
        let sh = self.sh.clone();
        let ctx2 = ctx.clone();
        let o = self.o.clone();
        let hand = self.hand_on.then(|| self.hand.clone());
        let dark = ctx.style().visuals.dark_mode;
        {
            let mut s = sh.lock().unwrap();
            s.busy = Some("Bild laden".into());
            s.error = None;
            s.info = None;
            s.gen += 1;
        }
        self.reset_view();
        let gen = sh.lock().unwrap().gen;
        std::thread::spawn(move || {
            let res = std::fs::read(&path).map_err(|e| e.to_string()).and_then(|b| image::load_from_memory(&b).map_err(|e| e.to_string()));
            match res {
                Ok(img) => {
                    let img = img.to_rgba8();
                    let (w, h) = (img.width() as usize, img.height() as usize);
                    let name = path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                    Self::compute_new(sh, ctx2, Doc::new(w, h, img.into_raw()), Some(path), name, o, hand, gen, dark);
                }
                Err(e) => {
                    let mut s = sh.lock().unwrap();
                    s.busy = None;
                    s.error = Some(format!("Das Bild ließ sich nicht öffnen: {e}"));
                    ctx2.request_repaint();
                }
            }
        });
    }

    pub fn open_dialog(&mut self, ctx: &egui::Context) {
        crate::dialog::pick_file("vec_open", ctx, "Bilder", &["png", "jpg", "jpeg", "webp", "gif", "bmp", "PNG", "JPG", "JPEG"]);
    }

    pub fn load_sample(&mut self, ctx: &egui::Context) {
        let (w, h, img) = vector::sample_image();
        let sh = self.sh.clone();
        let ctx2 = ctx.clone();
        let o = self.o.clone();
        let hand = self.hand_on.then(|| self.hand.clone());
        let dark = ctx.style().visuals.dark_mode;
        let gen = {
            let mut s = sh.lock().unwrap();
            s.busy = Some("Beispielbild".into());
            s.gen += 1;
            s.gen
        };
        self.reset_view();
        std::thread::spawn(move || Self::compute_new(sh, ctx2, Doc::new(w, h, img), None, "Beispielbild".into(), o, hand, gen, dark));
    }

    fn reset_view(&mut self) {
        self.sel = None;
        self.sel_info = None;
        self.pick = None;
        self.zoom = 1.0;
        self.pan = Vec2::ZERO;
        self.k = 0;
        self.width_px = 0.0;
    }

    #[allow(clippy::too_many_arguments)]
    fn compute_new(sh: Arc<Mutex<Shared>>, ctx: egui::Context, mut d: Doc, path: Option<PathBuf>, name: String, o: Opts, hand: Option<HandOpts>, gen: u64, dark: bool) {
        sh.lock().unwrap().busy = Some("Farben und Pfade berechnen".into());
        ctx.request_repaint();
        let sh2 = sh.clone();
        let ctx2 = ctx.clone();
        let p2 = path.clone();
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let info = match &hand {
                Some(h) => {
                    let progress = |msg: &str, _f: f32| {
                        let mut s = sh2.lock().unwrap();
                        if s.gen == gen {
                            s.busy = Some(msg.trim_end_matches(" …").to_string());
                        }
                        ctx2.request_repaint();
                    };
                    Some(crate::pipeline::run(&mut d, p2.as_deref(), &o, h, 0, &progress))
                }
                None => {
                    d.run(0, &o, 0);
                    None
                }
            };
            let rd = render_all(&d, &o, gen, dark);
            (d, rd, info)
        }));
        let mut s = sh.lock().unwrap();
        if s.gen != gen {
            return;
        }
        s.busy = None;
        match r {
            Ok((d, rd, info)) => {
                s.doc = Some(Arc::new(d));
                s.path = path;
                s.name = name;
                s.rendered = Some(rd);
                s.new_src = true;
                s.hand_info = info;
            }
            Err(_) => s.error = Some("Interner Fehler bei der Berechnung".into()),
        }
        ctx.request_repaint();
    }

    /// Das aktuelle Bild komplett neu nachzeichnen (z. B. nach Umschalten von
    /// „Nachzeichnen wie von Hand“), inklusive Texterkennung.
    fn retrace(&mut self, ctx: &egui::Context) {
        let (doc, path, name) = {
            let s = self.sh.lock().unwrap();
            match &s.doc {
                Some(d) => (d.clone(), s.path.clone(), s.name.clone()),
                None => return,
            }
        };
        let gen = {
            let mut s = self.sh.lock().unwrap();
            s.gen += 1;
            s.busy = Some("Wird nachgezeichnet".into());
            s.gen
        };
        self.k = 0;
        let mut d = Doc::new(doc.src_w, doc.src_h, doc.src.clone());
        d.edits = doc.edits.clone();
        let (sh, ctx2, o) = (self.sh.clone(), ctx.clone(), self.o.clone());
        let hand = self.hand_on.then(|| self.hand.clone());
        let dark = ctx.style().visuals.dark_mode;
        std::thread::spawn(move || Self::compute_new(sh, ctx2, d, path, name, o, hand, gen, dark));
    }

    /// Neu berechnen ab Stufe (0..3; 4 = nur neu zeichnen). `f` ändert vorher das Dokument.
    fn recompute(&mut self, ctx: &egui::Context, stage: u8, f: impl FnOnce(&mut Doc)) {
        let sh = self.sh.clone();
        let (doc, gen) = {
            let mut s = sh.lock().unwrap();
            let d = match &s.doc {
                Some(d) => d.clone(),
                None => return,
            };
            s.gen += 1;
            s.busy = Some("Wird berechnet".into());
            (d, s.gen)
        };
        let mut d = (*doc).clone();
        f(&mut d);
        let o = self.o.clone();
        let k = self.k;
        let ctx2 = ctx.clone();
        let dark = ctx.style().visuals.dark_mode;
        let hand = (d.hand.k > 0).then(|| self.hand.clone());
        std::thread::spawn(move || {
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                if stage <= 3 {
                    match &hand {
                        Some(h) => hand::refresh(&mut d, &o, h, k, stage),
                        None => d.run(stage, &o, k),
                    }
                }
                let rd = render_all(&d, &o, gen, dark);
                (d, rd)
            }));
            let mut s = sh.lock().unwrap();
            if s.gen != gen {
                return;
            }
            s.busy = None;
            match r {
                Ok((d, rd)) => {
                    s.doc = Some(Arc::new(d));
                    s.rendered = Some(rd);
                }
                Err(_) => s.error = Some("Interner Fehler bei der Berechnung".into()),
            }
            ctx2.request_repaint();
        });
    }

    fn doc(&self) -> Option<Arc<Doc>> {
        self.sh.lock().unwrap().doc.clone()
    }

    fn update_textures(&mut self, ctx: &egui::Context) {
        let mut s = self.sh.lock().unwrap();
        if s.new_src {
            s.new_src = false;
            if let Some(d) = &s.doc {
                let img = to_image(d.src_w, d.src_h, &d.src);
                let orig = ctx.load_texture("vw_orig", img, TextureOptions::LINEAR);
                self.tex = Some(Tex { orig, vec: None, wire: None, pal: None, sel: None });
                if self.width_px == 0.0 {
                    self.width_px = d.src_w as f64;
                }
            }
        }
        if let Some(r) = s.rendered.take() {
            if let Some(t) = &mut self.tex {
                t.vec = Some(ctx.load_texture("vw_vec", r.vec, TextureOptions::LINEAR));
                t.wire = Some(ctx.load_texture("vw_wire", r.wire, TextureOptions::LINEAR));
                t.pal = Some(ctx.load_texture("vw_pal", r.pal, TextureOptions::NEAREST));
                self.tex_gen = r.gen;
            }
            drop(s);
            self.refresh_selection(ctx);
        }
    }

    fn refresh_selection(&mut self, ctx: &egui::Context) {
        let d = match self.doc() {
            Some(d) => d,
            None => return,
        };
        let (x, y) = match self.sel {
            Some(p) => p,
            None => {
                if let Some(t) = &mut self.tex {
                    t.sel = None;
                }
                self.sel_info = None;
                return;
            }
        };
        let (px, v, nb) = match d.region_at(x, y) {
            Some(r) => r,
            None => {
                self.sel = None;
                return;
            }
        };
        let (w, h) = (d.w, d.h);
        let mut mask = vec![false; w * h];
        for &p in &px {
            mask[p as usize] = true;
        }
        let mut img = vec![0u8; w * h * 4];
        for &p in &px {
            let p = p as usize;
            let (xx, yy) = (p % w, p / w);
            let edge = xx == 0 || yy == 0 || xx == w - 1 || yy == h - 1 || !mask[p - 1] || !mask[p + 1] || !mask[p - w] || !mask[p + w];
            if edge {
                img[p * 4..p * 4 + 4].copy_from_slice(&[120, 255, 60, 255]);
            } else if ((xx + yy) >> 2) & 1 == 1 {
                img[p * 4..p * 4 + 4].copy_from_slice(&[120, 255, 60, 80]);
            }
        }
        if let Some(t) = &mut self.tex {
            t.sel = Some(ctx.load_texture("vw_sel", to_image(w, h, &img), TextureOptions::NEAREST));
        }
        self.sel_info = Some((px.len(), v, nb));
    }

    fn edit(&mut self, ctx: &egui::Context, to: i32) {
        let (x, y) = match self.sel {
            Some(p) => p,
            None => return,
        };
        if let Some((_, v, _)) = &self.sel_info {
            if *v as i32 == to {
                return;
            }
        }
        self.recompute(ctx, 2, move |d| {
            d.edits.push(Edit { x, y, to });
            d.redo.clear();
        });
    }

    fn undo(&mut self, ctx: &egui::Context) {
        self.recompute(ctx, 2, |d| {
            if let Some(e) = d.edits.pop() {
                d.redo.push(e);
            }
        });
    }

    fn redo(&mut self, ctx: &egui::Context) {
        self.recompute(ctx, 2, |d| {
            if let Some(e) = d.redo.pop() {
                d.edits.push(e);
            }
        });
    }

    fn export_opts(&self, d: &Doc) -> Opts {
        let mut o = self.o.clone();
        if self.mm {
            o.unit = Unit::Mm;
            o.dpi = 96.0;
            o.scale = (self.width_mm * o.dpi / 25.4) / d.src_w.max(1) as f64;
        } else {
            o.unit = Unit::Px;
            o.dpi = 96.0;
            o.scale = (self.width_px.max(1.0)) / d.src_w.max(1) as f64;
        }
        o
    }

    pub fn save(&mut self, ctx: &egui::Context, st: &Settings) {
        if self.doc().is_none() {
            return;
        }
        let (path, name) = {
            let s = self.sh.lock().unwrap();
            (s.path.clone(), s.name.clone())
        };
        let stem = Path::new(&name).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "vektor".into());
        let ext = self.fmt.ext();
        let dir = st.out_dir.clone().or_else(|| path.as_ref().and_then(|p| p.parent().map(|x| x.to_path_buf())));
        crate::dialog::save_file("vec_save", ctx, (ext.to_uppercase(), vec![ext.to_string()]), format!("{stem}.{ext}"), dir);
    }

    fn save_to(&mut self, out: PathBuf, st: &Settings) {
        let d = match self.doc() {
            Some(d) => d,
            None => return,
        };
        let name = self.sh.lock().unwrap().name.clone();
        let stem = Path::new(&name).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "vektor".into());
        let o = self.export_opts(&d);
        let data: Vec<u8> = match self.fmt {
            Fmt::Svg => d.build_svg(&o, false).into_bytes(),
            Fmt::Pdf => d.build_pdf(&o),
            Fmt::Eps => d.build_eps(&o).into_bytes(),
            Fmt::Dxf => d.build_dxf(&o).into_bytes(),
            Fmt::Png => d.build_png(&o),
            Fmt::Mf3 => {
                let parts = d.build_3d(&self.o, self.width_mm, self.height_3d, self.base_3d);
                nz_core::export3mf::write_3mf(&parts, &[stem.clone()], &nz_core::export3mf::ExportOpts::default())
            }
        };
        let mut s = self.sh.lock().unwrap();
        match std::fs::write(&out, data) {
            Ok(_) => {
                s.info = Some(format!("„{}“ gespeichert", out.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default()));
                s.error = None;
                if st.reveal {
                    crate::reveal(&out);
                }
            }
            Err(e) => s.error = Some(format!("Speichern fehlgeschlagen: {e}")),
        }
    }

    // -----------------------------------------------------------------------

    pub fn ui(&mut self, ctx: &egui::Context, st: &mut Settings, hovering: bool) {
        if let Some(f) = crate::dialog::take("vec_open").and_then(|v| v.into_iter().next()) {
            self.load(f, ctx);
        }
        if let Some(f) = crate::dialog::take("vec_save").and_then(|v| v.into_iter().next()) {
            self.save_to(f, st);
        }
        self.update_textures(ctx);
        let t = theme::tok_for(ctx.style().visuals.dark_mode, st);
        if self.o.aa != st.antialias {
            self.o.aa = st.antialias;
            self.recompute(ctx, 4, |_| {});
        }
        // Tastenkürzel
        let (undo, redo, esc) = ctx.input(|i| {
            let z = i.key_pressed(egui::Key::Z) && i.modifiers.command;
            (z && !i.modifiers.shift, z && i.modifiers.shift, i.key_pressed(egui::Key::Escape))
        });
        if undo {
            self.undo(ctx);
        }
        if redo {
            self.redo(ctx);
        }
        if esc {
            self.sel = None;
            self.refresh_selection(ctx);
        }
        let busy = self.sh.lock().unwrap().busy.clone();
        egui::SidePanel::left("vw_left")
            .resizable(false)
            .exact_width(340.0)
            .frame(crate::side_frame(&t))
            .show(ctx, |ui| {
                egui::CentralPanel::default().frame(egui::Frame::new().inner_margin(16.0)).show_inside(ui, |ui| {
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| self.left_ui(ui, &t, ctx, hovering));
                });
            });
        egui::SidePanel::right("vw_right")
            .resizable(false)
            .exact_width(310.0)
            .frame(crate::right_frame(&t))
            .show(ctx, |ui| {
                egui::TopBottomPanel::bottom("vw_right_bottom").frame(crate::foot_frame(&t)).show_inside(ui, |ui| self.export_ui(ui, &t, st, busy.is_some()));
                egui::CentralPanel::default().frame(egui::Frame::new().inner_margin(16.0)).show_inside(ui, |ui| {
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| self.edit_ui(ui, &t, ctx));
                });
            });
        egui::CentralPanel::default().frame(egui::Frame::new().fill(t.bg)).show(ctx, |ui| self.center_ui(ui, &t, st, busy));
    }

    fn left_ui(&mut self, ui: &mut egui::Ui, t: &Tok, ctx: &egui::Context, hovering: bool) {
        let (doc, name, error, info) = {
            let s = self.sh.lock().unwrap();
            (s.doc.clone(), s.name.clone(), s.error.clone(), s.info.clone())
        };
        let pick = || crate::dialog::pick_file("vec_open", ctx, "Bilder", &["png", "jpg", "jpeg", "webp", "gif", "bmp", "PNG", "JPG", "JPEG"]);
        theme::step(ui, t, 1, "Bild");
        match &doc {
            None => {
                if theme::drop_zone(ui, t, hovering, "Bild hierher ziehen", "PNG, JPG, WebP, GIF oder BMP", 90.0) {
                    pick();
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    theme::hint(ui, t, "Kein Bild zur Hand?");
                    if ui.add(egui::Button::new(RichText::new("Beispielbild").color(t.accent)).frame(false)).clicked() {
                        self.load_sample(ctx);
                    }
                });
            }
            Some(d) => {
                let mut change = false;
                theme::card(ui, t, |ui| {
                    ui.horizontal(|ui| {
                        let ext = Path::new(&name).extension().map(|e| e.to_string_lossy().to_ascii_uppercase()).unwrap_or_else(|| "BILD".into());
                        crate::file_icon(ui, t, &ext, 40.0);
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                // „Ändern“ zuerst platzieren, der Name bekommt den Rest
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.add(egui::Button::new(RichText::new("Ändern").color(t.accent)).frame(false)).clicked() {
                                        change = true;
                                    }
                                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                        ui.add(egui::Label::new(theme::strong(name.clone(), 13.5).color(t.text)).truncate()).on_hover_text(name.clone());
                                    });
                                });
                            });
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 5.0;
                                theme::chip(ui, t, &format!("{} × {} px", d.src_w, d.src_h));
                                theme::chip(ui, t, &format!("{} Farben", d.layers.len()));
                            });
                        });
                    });
                });
                if change {
                    pick();
                }
            }
        }
        if let Some(e) = error {
            ui.label(RichText::new(e).size(12.0).color(t.err));
        }
        if let Some(i) = info {
            ui.label(RichText::new(i).size(12.0).color(t.ok));
        }
        let d = match doc {
            Some(d) => d,
            None => {
                ui.add_space(14.0);
                for (h, s) in [
                    ("Echte Kurven", "Bézierkurven, Kreise und Ellipsen statt Treppenstufen. Spitze Ecken bleiben spitz."),
                    ("Volle Kontrolle über Farben", "Palette nach Farbanzahl, Töne zusammenführen, Hintergrund transparent machen."),
                    ("Flächen nachbearbeiten", "Einzelne Bereiche umfärben oder mit Nachbarn verschmelzen, mit Rückgängig."),
                    ("Für den 3D-Druck", "Als 3MF speichern: jede Farbe ein eigenes Teil für den Mehrfarbdruck."),
                ] {
                    theme::card(ui, t, |ui| {
                        ui.label(theme::strong(h, 13.0).color(t.text));
                        ui.label(RichText::new(s).size(12.0).color(t.muted));
                    });
                    ui.add_space(4.0);
                }
                return;
            }
        };
        ui.add_space(12.0);
        if self.menu == Menu::Main {
            self.menu_ui(ui, t, &d);
            return;
        }
        if ui.add(egui::Button::new(RichText::new("‹ Ergebnis verfeinern").size(13.0).color(t.accent)).frame(false)).clicked() {
            self.menu = Menu::Main;
            return;
        }
        let title = match self.menu {
            Menu::Background => "Hintergrund",
            Menu::Palette => "Palette",
            Menu::Regions => "Regionen",
            Menu::Edges => "Flächen & Kanten",
            Menu::Curves => "Kurven & Formen",
            Menu::Symmetry => "Symmetrie",
            Menu::Hand => "Nachzeichnen wie von Hand",
            Menu::Main => "",
        };
        ui.label(theme::strong(title, 20.0).color(t.text));
        theme::hint(ui, t, "Änderungen wirken sofort auf die Vorschau");
        ui.add_space(8.0);
        if self.menu == Menu::Hand {
            self.hand_ui(ui, t, ctx, &d);
            return;
        }
        if self.menu == Menu::Background {
            self.background_ui(ui, t, ctx, &d);
        }
        if self.menu == Menu::Regions {
            self.regions_ui(ui, t, ctx, &d);
        }
        if self.menu == Menu::Symmetry {
            let mut v = self.o.symmetry;
            theme::card(ui, t, |ui| {
                theme::row(ui, t, "Symmetrische Formen angleichen", |ui| {
                    if theme::toggle(ui, t, &mut v).changed() {}
                });
                theme::hint(ui, t, "Formen, die links und rechts oder oben und unten fast spiegelgleich sind, werden exakt symmetrisch – z. B. Umrisse, Wappen, Kreise. Schrift und unsymmetrische Formen bleiben unverändert.");
                if self.o.symmetry {
                    ui.add_space(4.0);
                    ui.label(theme::strong(&format!("{} symmetrische Formen erkannt", d.nsym), 13.0).color(t.text));
                }
            });
            if v != self.o.symmetry {
                self.o.symmetry = v;
                self.recompute(ctx, 3, |_| {});
            }
        }
        if self.menu != Menu::Palette {
            self.options_ui(ui, t, ctx);
            return;
        }
        theme::card(ui, t, |ui| {
            ui.horizontal(|ui| {
                ui.label(theme::strong("Anzahl", 13.0).color(t.text));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(format!("Auto = {}", d.q.auto)).size(12.0).color(t.muted));
                });
            });
            ui.add_space(2.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(5.0, 5.0);
                for k in 1..=d.q.pal.len().min(12) {
                    // Vorschau: die drei größten Gruppen dieser Stufe
                    let mut gs: Vec<(usize, [u8; 3])> = d.q.presets[k]
                        .iter()
                        .map(|m| {
                            let (mut r, mut g, mut b, mut w) = (0.0, 0.0, 0.0, 0.0);
                            for &i in m {
                                let c = d.q.pal[i];
                                let kk = if d.q.cnt[i] == 0 { 1.0 } else { d.q.cnt[i] as f64 };
                                r += c[0] as f64 * kk;
                                g += c[1] as f64 * kk;
                                b += c[2] as f64 * kk;
                                w += kk;
                            }
                            (w as usize, [(r / w) as u8, (g / w) as u8, (b / w) as u8])
                        })
                        .collect();
                    gs.sort_by(|a, b| b.0.cmp(&a.0));
                    let cols: Vec<[u8; 3]> = gs.iter().take(3).map(|x| x.1).collect();
                    let (r, resp) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::click());
                    let c = r.center();
                    let n = cols.len().max(1);
                    for (i, col) in cols.iter().enumerate() {
                        let a0 = -std::f32::consts::FRAC_PI_2 + i as f32 / n as f32 * std::f32::consts::TAU;
                        let a1 = a0 + std::f32::consts::TAU / n as f32;
                        let mut pts = vec![c];
                        for s in 0..=16 {
                            let a = a0 + (a1 - a0) * s as f32 / 16.0;
                            pts.push(c + Vec2::new(a.cos(), a.sin()) * 12.0);
                        }
                        ui.painter().add(egui::Shape::convex_polygon(pts, theme::c32(*col), Stroke::NONE));
                    }
                    let sel = d.k == k;
                    if sel {
                        ui.painter().circle_stroke(c, 14.0, Stroke::new(2.0, t.accent));
                    } else if k == d.q.auto {
                        ui.painter().circle_stroke(c, 13.0, Stroke::new(1.0, t.text));
                    }
                    let lum = cols.first().map(|c| 0.299 * c[0] as f32 + 0.587 * c[1] as f32 + 0.114 * c[2] as f32).unwrap_or(0.0);
                    let tc = if lum > 170.0 { Color32::from_gray(30) } else { Color32::WHITE };
                    ui.painter().text(c, egui::Align2::CENTER_CENTER, k.to_string(), egui::FontId::new(10.5, theme::semi()), tc);
                    let resp = resp.on_hover_text(format!("{k} Farben{}", if k == d.q.auto { " (Auto)" } else { "" }));
                    if resp.clicked() {
                        self.k = k;
                        self.pick = None;
                        self.recompute(ctx, 2, move |d| {
                            d.set_preset(k);
                            d.redo.clear();
                        });
                    }
                }
            });
            // Mehr als 12 Farben (detailreiche Bilder, Fotos): Regler bis zur vollen Palette
            let n = d.q.pal.len();
            if n > 12 {
                ui.add_space(4.0);
                let mut kk = d.k as f64;
                theme::label_value(ui, t, "Farben", &if d.k == n { format!("{n} (alle)") } else { d.k.to_string() });
                ui.spacing_mut().slider_width = ui.available_width();
                let r = ui.add(egui::Slider::new(&mut kk, 1.0..=n as f64).step_by(1.0).show_value(false));
                if r.drag_stopped() || (r.changed() && !r.dragged()) {
                    let k = kk.round() as usize;
                    self.k = k;
                    self.pick = None;
                    self.recompute(ctx, 2, move |d| {
                        d.set_preset(k);
                        d.redo.clear();
                    });
                }
            }
        });
        ui.add_space(6.0);
        // Ausgabefarben
        let tot: usize = d.q.cnt.iter().sum::<usize>().max(1);
        let mut action: Option<Box<dyn FnOnce(&mut Doc)>> = None;
        for gi in 0..d.groups.len() {
            let g = &d.groups[gi];
            let target = self.pick.is_some() && !g.src.contains(&self.pick.unwrap());
            let fr = egui::Frame::new()
                .fill(if target { t.accent_soft } else { t.card })
                .stroke(Stroke::new(1.0, if target { t.accent } else { t.line }))
                .corner_radius(10.0)
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        let mut c = d.gcol(gi);
                        ui.scope(|ui| {
                            ui.spacing_mut().interact_size = Vec2::new(34.0, 34.0);
                            if egui::color_picker::color_edit_button_srgb(ui, &mut c).changed() {
                                action = Some(Box::new(move |d: &mut Doc| d.groups[gi].over = Some(c)));
                            }
                        });
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                let pct = d.group_weight(g) as f64 / tot as f64 * 100.0;
                                let label = if g.transp { "transparent".to_string() } else { vector::hex(d.gcol(gi)).to_uppercase() };
                                ui.label(RichText::new(format!("{label} · {:.1} %", pct).replace('.', ",")).size(12.0).color(t.muted));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let txt = RichText::new("Transparent").size(11.0).color(if g.transp { t.text } else { t.muted });
                                    if ui.add(egui::Button::new(txt).selected(g.transp).corner_radius(6.0)).clicked() {
                                        let v = !g.transp;
                                        action = Some(Box::new(move |d: &mut Doc| d.groups[gi].transp = v));
                                    }
                                });
                            });
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
                                for &i in &g.src {
                                    let (r, resp) = ui.allocate_exact_size(Vec2::splat(17.0), Sense::click());
                                    ui.painter().circle_filled(r.center(), 8.0, theme::c32(d.q.pal[i]));
                                    ui.painter().circle_stroke(r.center(), 8.0, Stroke::new(1.0, Color32::from_gray(128)));
                                    if self.pick == Some(i) {
                                        ui.painter().circle_stroke(r.center(), 10.5, Stroke::new(2.0, t.warn));
                                    }
                                    if resp.on_hover_text(format!("Erkannter Ton {} – antippen zum Verschieben", vector::hex(d.q.pal[i]))).clicked() {
                                        self.pick = if self.pick == Some(i) { None } else { Some(i) };
                                    }
                                }
                                if let Some(p) = self.pick {
                                    if g.src.contains(&p) && g.src.len() > 1 && ui.small_button("Als eigene Farbe").clicked() {
                                        self.pick = None;
                                        action = Some(Box::new(move |d: &mut Doc| {
                                            d.groups[gi].src.retain(|&x| x != p);
                                            d.groups.push(vector::Group { src: vec![p], over: None, transp: false });
                                        }));
                                    }
                                }
                            });
                        });
                    });
                });
            if target && fr.response.interact(Sense::click()).clicked() {
                let p = self.pick.take().unwrap();
                action = Some(Box::new(move |d: &mut Doc| {
                    for h in d.groups.iter_mut() {
                        h.src.retain(|&x| x != p);
                    }
                    d.groups[gi].src.push(p);
                    d.groups.retain(|h| !h.src.is_empty());
                }));
            }
            ui.add_space(3.0);
        }
        if let Some(a) = action {
            self.recompute(ctx, 2, a);
        }
        ui.horizontal(|ui| {
            if theme::ghost(ui, t, "Alle zurücksetzen", true).clicked() {
                self.k = 0;
                self.pick = None;
                self.sel = None;
                self.recompute(ctx, 2, |d| {
                    d.edits.clear();
                    d.redo.clear();
                    let a = d.q.auto;
                    d.set_preset(a);
                });
            }
        });
    }

    /// Hauptmenü „Ergebnis verfeinern“
    fn menu_ui(&mut self, ui: &mut egui::Ui, t: &Tok, d: &Doc) {
        ui.label(theme::strong("Ergebnis verfeinern", 20.0).color(t.text));
        ui.add_space(6.0);
        let bg = d.background_group().map(|g| d.groups[g].transp);
        let hand_badge = if self.hand_on { "an" } else { "aus" }.to_string();
        let items: [(Menu, &str, &str, Option<String>); 7] = [
            (Menu::Hand, "Nachzeichnen wie von Hand", "Versteht das Bild: Text, feine Linien, Verläufe, Fotobereiche – alles lokal", Some(hand_badge)),
            (Menu::Background, "Hintergrund", "Hintergrund entfernen oder als Fläche behalten", bg.map(|b| if b { "entfernt".to_string() } else { "behalten".to_string() })),
            (Menu::Palette, "Palette", "Anzahl der Farben, Farben ändern, zusammenlegen oder transparent machen", Some(d.groups.len().to_string())),
            (Menu::Regions, "Regionen", "Einzelne Flächen anklicken und umfärben, zusammenführen oder entfernen", if d.edits.is_empty() { None } else { Some(d.edits.len().to_string()) }),
            (Menu::Edges, "Flächen & Kanten", "Muster beruhigen, Kanten begradigen, Genauigkeit", None),
            (Menu::Curves, "Kurven & Formen", "Bézierkurven, Kreise, Ecken, Aussparungen oder übereinander", None),
            (Menu::Symmetry, "Symmetrie", "Gespiegelte Formen erkennen und angleichen", Some(if self.o.symmetry { d.nsym.to_string() } else { "aus".to_string() })),
        ];
        for (m, title, sub, badge) in items {
            let r = egui::Frame::new()
                .fill(t.card)
                .stroke(Stroke::new(1.0, t.line))
                .corner_radius(10.0)
                .inner_margin(egui::Margin::symmetric(10, 10))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        // Symbol: Kreis mit Anfangsbuchstabe
                        let (ir, _) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::hover());
                        ui.painter().rect_filled(ir, 8.0, t.accent.gamma_multiply(0.18));
                        ui.painter().text(ir.center(), egui::Align2::CENTER_CENTER, &title[..1], egui::FontId::new(14.0, theme::semi()), t.accent);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // Pfeil nach rechts
                            let (ar, _) = ui.allocate_exact_size(Vec2::new(12.0, 18.0), Sense::hover());
                            let c = ar.center();
                            ui.painter().line_segment([c + Vec2::new(-3.0, -5.0), c + Vec2::new(2.0, 0.0)], Stroke::new(1.6, t.muted));
                            ui.painter().line_segment([c + Vec2::new(2.0, 0.0), c + Vec2::new(-3.0, 5.0)], Stroke::new(1.6, t.muted));
                            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(theme::strong(title, 14.0).color(t.text));
                                    if let Some(b) = &badge {
                                        theme::chip(ui, t, b);
                                    }
                                });
                                ui.add(egui::Label::new(RichText::new(sub).size(11.5).color(t.muted)).wrap());
                            });
                        });
                    });
                });
            if r.response.interact(Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                self.menu = m;
            }
            ui.add_space(4.0);
        }
    }

    /// Hintergrund entfernen / behalten
    fn background_ui(&mut self, ui: &mut egui::Ui, t: &Tok, ctx: &egui::Context, d: &Doc) {
        let bg = d.background_group();
        theme::card(ui, t, |ui| match bg {
            None => theme::hint(ui, t, "Kein einheitlicher Hintergrund erkannt. Einzelne Flächen kannst du unter „Regionen“ oder in der Palette transparent machen."),
            Some(g) => {
                ui.horizontal(|ui| {
                    theme::swatch(ui, d.gcol(g), 22.0);
                    ui.label(RichText::new(format!("Erkannte Hintergrundfarbe {}", vector::hex(d.gcol(g)).to_uppercase())).color(t.text));
                });
                ui.add_space(6.0);
                let mut removed = d.groups[g].transp;
                let before = removed;
                theme::segmented(ui, t, &mut removed, &[(false, "Behalten"), (true, "Entfernen")], true);
                if removed != before {
                    self.recompute(ctx, 2, move |d| d.groups[g].transp = removed);
                }
                theme::hint(ui, t, "Entfernen macht die Farbe am Bildrand transparent – im SVG fällt sie ganz weg.");
            }
        });
    }

    /// Regionen: Hinweise und Bearbeitungsliste (die Auswahl selbst steht rechts)
    /// Schalter von „Nachzeichnen wie von Hand“
    fn hand_ui(&mut self, ui: &mut egui::Ui, t: &Tok, ctx: &egui::Context, d: &Doc) {
        let mut on = self.hand_on;
        let mut h = self.hand.clone();
        theme::card(ui, t, |ui| {
            theme::row(ui, t, "Eingeschaltet", |ui| {
                theme::toggle(ui, t, &mut on);
            });
            theme::hint(ui, t, "Statt nur Pixelflächen nachzuziehen, wird das Bild erst verstanden. Läuft komplett auf deinem Mac, ohne Internet.");
        });
        ui.add_space(6.0);
        let rows: [(&str, &str, bool); 5] = [
            ("Hintergrund außen freistellen", "Einfarbiger Rand um das Motiv wird transparent", h.bg_cut),
            ("Text erkennen & ersetzen", "Apple-Texterkennung, passende Schrift von deinem Mac – auch fett und dünn gemischt", h.text),
            ("Feine Linien als Striche", "Spielfeld- und Rahmenlinien bleiben durchgehend, Kreise werden echte Kreise", h.lines),
            ("Fotobereiche beruhigen", "Fotos und Muster (Waben, Raster) werden weiche Flächen statt Flecken", h.photo),
            ("Farbverläufe erkennen", "Gold, Metall und Schatten als echter Verlauf statt Farbstufen", h.gradients),
        ];
        let mut vals: Vec<bool> = rows.iter().map(|r| r.2).collect();
        theme::card(ui, t, |ui| {
            if !on {
                ui.disable();
            }
            for (i, (title, sub, _)) in rows.iter().enumerate() {
                if i > 0 {
                    ui.add_space(6.0);
                }
                theme::row(ui, t, title, |ui| {
                    theme::toggle(ui, t, &mut vals[i]);
                });
                ui.add(egui::Label::new(RichText::new(*sub).size(11.5).color(t.muted)).wrap());
            }
        });
        h.bg_cut = vals[0];
        h.text = vals[1];
        h.lines = vals[2];
        h.photo = vals[3];
        h.gradients = vals[4];
        ui.add_space(6.0);
        // Ergebnis
        if self.hand_on {
            let info = self.sh.lock().unwrap().hand_info.clone();
            theme::card(ui, t, |ui| {
                ui.label(theme::strong("Gefunden", 13.0).color(t.text));
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(5.0, 5.0);
                    theme::chip(ui, t, &format!("{} Texte", d.hand.texts));
                    theme::chip(ui, t, &format!("{} Linien", d.hand.lines.len()));
                    theme::chip(ui, t, &format!("{} Verläufe", d.hand.grads.len()));
                    if d.hand.photo > 0.0 {
                        theme::chip(ui, t, &format!("{:.0} % Foto", d.hand.photo * 100.0));
                    }
                });
                if let Some(i) = &info {
                    if let Some(n) = &i.note {
                        ui.label(RichText::new(n).size(11.5).color(t.warn));
                    }
                    ui.label(RichText::new(format!("berechnet in {:.1} s", i.secs)).size(11.0).color(t.faint));
                }
            });
            ui.add_space(6.0);
        }
        if theme::primary_button(ui, t, "Neu nachzeichnen", true, true).clicked() {
            self.retrace(ctx);
        }
        if on != self.hand_on || h != self.hand {
            let only_grad = on == self.hand_on && HandOpts { gradients: self.hand.gradients, ..h.clone() } == self.hand;
            self.hand_on = on;
            self.hand = h;
            if only_grad && d.hand.k > 0 {
                self.recompute(ctx, 3, |_| {});
            } else {
                self.retrace(ctx);
            }
        }
    }

    fn regions_ui(&mut self, ui: &mut egui::Ui, t: &Tok, ctx: &egui::Context, d: &Doc) {
        theme::card(ui, t, |ui| {
            theme::hint(ui, t, "Tippe in der Vorschau auf eine Fläche. Rechts kannst du sie dann umfärben, mit einem Nachbarn verschmelzen oder transparent machen.");
            ui.add_space(6.0);
            ui.label(RichText::new(format!("{} Bearbeitung{}", d.edits.len(), if d.edits.len() == 1 { "" } else { "en" })).color(t.text));
            ui.horizontal(|ui| {
                if theme::ghost(ui, t, "Rückgängig", !d.edits.is_empty()).clicked() && !d.edits.is_empty() {
                    self.undo(ctx);
                }
                if theme::ghost(ui, t, "Wiederholen", !d.redo.is_empty()).clicked() && !d.redo.is_empty() {
                    self.redo(ctx);
                }
            });
            if !d.edits.is_empty() && theme::ghost(ui, t, "Alle Bearbeitungen verwerfen", true).clicked() {
                self.sel = None;
                self.recompute(ctx, 2, |d| {
                    d.edits.clear();
                    d.redo.clear();
                });
            }
        });
    }

    /// Einstellungen für „Flächen & Kanten“ bzw. „Kurven & Formen“
    fn options_ui(&mut self, ui: &mut egui::Ui, t: &Tok, ctx: &egui::Context) {
        let menu = self.menu;
        if menu != Menu::Edges && menu != Menu::Curves {
            return;
        }
        let mut stage: Option<u8> = None;
        theme::card(ui, t, |ui| {
            ui.spacing_mut().slider_width = ui.available_width();
            let mut slider = |ui: &mut egui::Ui, label: &str, v: &mut f64, range: std::ops::RangeInclusive<f64>, step: f64, txt: String, st: u8, stage: &mut Option<u8>| {
                theme::label_value(ui, t, label, &txt);
                let r = ui.add(egui::Slider::new(v, range).step_by(step).show_value(false));
                if r.drag_stopped() || (r.changed() && !r.dragged()) {
                    *stage = Some(stage.map_or(st, |s: u8| s.min(st)));
                }
            };
            if menu == Menu::Edges {
                // Fotos, Verläufe, Muster: feine Strukturen zu ruhigen Flächen zusammenfassen
                ui.label(RichText::new("Flächen beruhigen").color(t.muted));
                let mut calm = self.o.calm.round() as u8;
                if theme::segmented(ui, t, &mut calm, &[(0u8, "Aus"), (1, "Leicht"), (2, "Stark")], true) {
                    self.o.calm = calm as f64;
                    stage = Some(0);
                }
                theme::row(ui, t, "Gerade Kanten begradigen", |ui| {
                    if theme::toggle(ui, t, &mut self.o.straight).changed() {
                        stage = Some(stage.map_or(3, |s| s.min(3)));
                    }
                });
                theme::row(ui, t, "Kanten subpixelgenau", |ui| {
                    if theme::toggle(ui, t, &mut self.o.subpx).changed() {
                        stage = Some(stage.map_or(3, |s| s.min(3)));
                    }
                });
                let tl = format!("{:.2} px", self.o.tol).replace('.', ",");
                slider(ui, "Pfad-Genauigkeit", &mut self.o.tol, 0.05..=2.0, 0.05, tl, 3, &mut stage);
                let sm = if self.o.smooth > 0.0 { format!("{} px", self.o.smooth) } else { "aus".into() };
                slider(ui, "Konturglättung", &mut self.o.smooth, 0.0..=4.0, 0.5, sm, 2, &mut stage);
                let ma = format!("{:.0} px²", self.o.min_area);
                slider(ui, "Mindestfläche", &mut self.o.min_area, 0.0..=100.0, 1.0, ma, 3, &mut stage);
                ui.add_space(4.0);
                ui.label(RichText::new("Analyse-Auflösung").color(t.muted));
                if theme::segmented(ui, t, &mut self.o.res, &[(800u32, "800"), (1200, "1200"), (1800, "1800"), (2600, "2600")], true) {
                    stage = Some(0);
                }
                theme::row(ui, t, "Rauschen entfernen", |ui| {
                    if theme::toggle(ui, t, &mut self.o.noise).changed() {
                        stage = Some(stage.map_or(2, |s| s.min(2)));
                    }
                });
                theme::row(ui, t, "Kantensäume entfernen", |ui| {
                    if theme::toggle(ui, t, &mut self.o.halo).changed() {
                        stage = Some(stage.map_or(2, |s| s.min(2)));
                    }
                });
            } else {
                let co = format!("{:.0}°", self.o.corner);
                slider(ui, "Ecken-Schwelle", &mut self.o.corner, 15.0..=160.0, 5.0, co, 3, &mut stage);
                ui.add_space(4.0);
                ui.label(RichText::new("Formen").color(t.muted));
                if theme::segmented(ui, t, &mut self.o.stack, &[(false, "Aussparungen"), (true, "Übereinander")], true) {
                    stage = Some(stage.map_or(3, |s| s.min(3)));
                }
                ui.label(RichText::new("Kurven").color(t.muted));
                for (label, v) in [
                    ("Quadratische Bézierkurven", &mut self.o.quad),
                    ("Kubische Bézierkurven", &mut self.o.cubic),
                    ("Kreise erkennen", &mut self.o.circles),
                    ("Ellipsen erkennen", &mut self.o.ellipses),
                ] {
                    theme::row(ui, t, label, |ui| {
                        if theme::toggle(ui, t, v).changed() {
                            stage = Some(stage.map_or(3, |s| s.min(3)));
                        }
                    });
                }
                ui.add_space(4.0);
                theme::row(ui, t, "Lückenfüller (Kontur unter Flächen)", |ui| {
                    if theme::toggle(ui, t, &mut self.o.gap).changed() {
                        stage = Some(stage.unwrap_or(4));
                    }
                });
                theme::row(ui, t, "Formen mit Kontur zeichnen", |ui| {
                    if theme::toggle(ui, t, &mut self.o.stroke).changed() {
                        stage = Some(stage.unwrap_or(4));
                    }
                });
                if self.o.stroke {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Breite").color(t.muted));
                        if ui.add(egui::DragValue::new(&mut self.o.stroke_w).range(0.1..=20.0).speed(0.1)).changed() {
                            stage = Some(stage.unwrap_or(4));
                        }
                        if egui::color_picker::color_edit_button_srgb(ui, &mut self.o.stroke_color).changed() {
                            stage = Some(stage.unwrap_or(4));
                        }
                        if ui.checkbox(&mut self.o.stroke_only, "nur Kontur").changed() {
                            stage = Some(stage.unwrap_or(4));
                        }
                    });
                }
            }
        });
        if let Some(sg) = stage {
            self.recompute(ctx, sg, |_| {});
        }
    }

    fn edit_ui(&mut self, ui: &mut egui::Ui, t: &Tok, ctx: &egui::Context) {
        let d = match self.doc() {
            Some(d) => d,
            None => {
                ui.label(theme::strong("Fläche bearbeiten", 15.0).color(t.text));
                theme::hint(ui, t, "Erst ein Bild laden.");
                return;
            }
        };
        let info = self.sh.lock().unwrap().hand_info.clone();
        if let Some(i) = info.filter(|i| !i.matches.is_empty()) {
            ui.label(theme::strong("Erkannter Text", 15.0).color(t.text));
            ui.add_space(2.0);
            for m in &i.matches {
                theme::card(ui, t, |ui| {
                    ui.label(theme::strong(&m.text, 13.5).color(t.text));
                    ui.add(egui::Label::new(RichText::new(format!("{} · {}", m.family, m.styles.join(", "))).size(11.5).color(t.muted)).wrap());
                    ui.label(RichText::new(format!("Übereinstimmung {:.0} %", m.score * 100.0)).size(11.0).color(t.faint));
                });
                ui.add_space(4.0);
            }
            theme::hint(ui, t, "Schrift aus den Schriften deines Macs, als Kurven gespeichert – die SVG sieht überall gleich aus.");
            ui.add_space(12.0);
        }
        ui.label(theme::strong("Fläche bearbeiten", 15.0).color(t.text));
        ui.horizontal(|ui| {
            if theme::ghost(ui, t, "Rückgängig", !d.edits.is_empty()).on_hover_text("Strg/Cmd+Z").clicked() && !d.edits.is_empty() {
                self.undo(ctx);
            }
            if theme::ghost(ui, t, "Wiederholen", !d.redo.is_empty()).on_hover_text("Strg/Cmd+Umschalt+Z").clicked() && !d.redo.is_empty() {
                self.redo(ctx);
            }
        });
        ui.add_space(4.0);
        let info = self.sel_info.clone();
        theme::card(ui, t, |ui| match info {
            None => theme::hint(ui, t, "Tippe im Bild auf eine Fläche. Sie wird grün markiert – dann kannst du sie umfärben, mit einem Nachbarn verschmelzen oder transparent machen."),
            Some((n, v, nb)) => {
                let orig = (n as f64 / (d.sc * d.sc)).round() as usize;
                ui.label(RichText::new(format!("Ausgewählt · etwa {} px im Original", fmt_count(orig, "").trim())).size(12.0).color(t.muted));
                ui.add_space(4.0);
                ui.label(theme::strong("Farbe", 12.5).color(t.text));
                let mut to: Option<i32> = None;
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                    for gi in 0..d.groups.len() {
                        if d.groups[gi].transp {
                            continue;
                        }
                        if color_chip(ui, t, Some(d.gcol(gi)), v == gi as i16).on_hover_text(vector::hex(d.gcol(gi))).clicked() {
                            to = Some(gi as i32);
                        }
                    }
                    if color_chip(ui, t, None, v < 0).on_hover_text("Transparent").clicked() {
                        to = Some(-1);
                    }
                });
                if !nb.is_empty() {
                    ui.add_space(4.0);
                    ui.label(theme::strong("Farben der Nachbarn", 12.5).color(t.text));
                    ui.horizontal_wrapped(|ui| {
                        for &g in &nb {
                            let c = if g >= 0 { Some(d.gcol(g as usize)) } else { None };
                            if color_chip(ui, t, c, false).clicked() {
                                to = Some(g as i32);
                            }
                        }
                    });
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if theme::ghost(ui, t, "Mit Nachbar verschmelzen", !nb.is_empty()).clicked() {
                        to = nb.first().map(|&g| g as i32);
                    }
                    if theme::ghost(ui, t, "Transparent", true).clicked() {
                        to = Some(-1);
                    }
                });
                if let Some(to) = to {
                    self.edit(ctx, to);
                }
            }
        });
        if !d.edits.is_empty() {
            ui.add_space(4.0);
            theme::hint(ui, t, &format!("{} Bearbeitung{}", d.edits.len(), if d.edits.len() == 1 { "" } else { "en" }));
        }
    }

    fn export_ui(&mut self, ui: &mut egui::Ui, t: &Tok, st: &Settings, busy: bool) {
        let d = self.doc();
        theme::step(ui, t, 3, "Speichern als");
        let fmts = [(Fmt::Svg, "SVG"), (Fmt::Pdf, "PDF"), (Fmt::Dxf, "DXF"), (Fmt::Eps, "EPS"), (Fmt::Png, "PNG"), (Fmt::Mf3, "3MF")];
        let w = (ui.available_width() - 12.0) / 3.0;
        for row in fmts.chunks(3) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                for (f, label) in row {
                    let (r, resp) = ui.allocate_exact_size(Vec2::new(w, 32.0), Sense::click());
                    let on = self.fmt == *f;
                    ui.painter().rect_filled(r, 9.0, if on { t.accent_soft } else if resp.hovered() { t.card2 } else { Color32::TRANSPARENT });
                    ui.painter().rect_stroke(r, 9.0, Stroke::new(1.0, if on { t.accent } else { t.line }), egui::StrokeKind::Inside);
                    ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, *label, egui::FontId::new(12.5, theme::semi()), if on { t.text } else { t.muted });
                    if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        self.fmt = *f;
                    }
                }
            });
        }
        ui.add_space(4.0);
        if let Some(d) = &d {
            let ar = d.src_h as f64 / d.src_w.max(1) as f64;
            if self.fmt == Fmt::Mf3 {
                egui::Grid::new("dim3").num_columns(2).spacing([10.0, 5.0]).show(ui, |ui| {
                    ui.label(RichText::new("Breite").color(t.muted));
                    ui.add(egui::DragValue::new(&mut self.width_mm).range(5.0..=1000.0).speed(0.5).suffix(" mm"));
                    ui.end_row();
                    ui.label(RichText::new("Dicke je Farbe").color(t.muted));
                    ui.add(egui::DragValue::new(&mut self.height_3d).range(0.2..=50.0).speed(0.05).suffix(" mm"));
                    ui.end_row();
                    ui.label(RichText::new("Sockel").color(t.muted));
                    ui.add(egui::DragValue::new(&mut self.base_3d).range(0.0..=20.0).speed(0.05).suffix(" mm"));
                    ui.end_row();
                });
                theme::hint(ui, t, &format!("{:.0} × {:.0} mm · jede Farbe ein Teil mit eigenem Filament", self.width_mm, self.width_mm * ar));
            } else {
                ui.horizontal(|ui| {
                    theme::segmented(ui, t, &mut self.mm, &[(true, "mm"), (false, "px")], false);
                    if self.mm {
                        ui.add(egui::DragValue::new(&mut self.width_mm).range(1.0..=5000.0).speed(0.5).prefix("Breite ").suffix(" mm"));
                    } else {
                        ui.add(egui::DragValue::new(&mut self.width_px).range(1.0..=20000.0).speed(1.0).prefix("Breite ").suffix(" px"));
                    }
                });
                let txt = if self.mm {
                    format!("{:.1} × {:.1} mm", self.width_mm, self.width_mm * ar)
                } else {
                    format!("{:.0} × {:.0} px", self.width_px, self.width_px * ar)
                };
                theme::hint(ui, t, &txt.replace('.', ","));
            }
        }
        ui.add_space(4.0);
        let label = format!("{} speichern …", self.fmt.ext().to_uppercase());
        if theme::primary(ui, t, &label, d.is_some() && !busy, true).clicked() {
            let ctx = ui.ctx().clone();
            self.save(&ctx, st);
        }
        if self.fmt == Fmt::Svg {
            if let Some(d) = &d {
                ui.vertical_centered(|ui| {
                    if ui.add(egui::Button::new(RichText::new("SVG-Code kopieren").size(12.0).color(t.accent)).frame(false)).clicked() {
                        let o = self.export_opts(d);
                        ui.ctx().copy_text(d.build_svg(&o, false));
                        self.sh.lock().unwrap().info = Some("SVG-Code in die Zwischenablage kopiert".into());
                    }
                });
            }
        }
    }

    fn center_ui(&mut self, ui: &mut egui::Ui, t: &Tok, st: &Settings, busy: Option<String>) {
        let rect = ui.max_rect();
        let (bc, be) = crate::preview_bg(t, st);
        crate::paint_bg(ui.painter(), rect, bc, be);
        let d = self.doc();
        let (d, tex) = match (d, &self.tex) {
            (Some(d), Some(tex)) => (d, tex),
            _ => {
                let msg = if busy.is_some() { "Wird berechnet …" } else { "Bild links hinzufügen – oder einfach ins Fenster ziehen" };
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, msg, egui::FontId::proportional(14.0), t.muted);
                return;
            }
        };
        let resp = ui.interact(rect, ui.id().with("vw_stage"), Sense::click_and_drag());
        // Bildfläche einpassen
        let ar = d.src_w as f32 / d.src_h.max(1) as f32;
        let avail = rect.shrink2(Vec2::new(40.0, 70.0));
        let mut size = if avail.width() / avail.height() > ar { Vec2::new(avail.height() * ar, avail.height()) } else { Vec2::new(avail.width(), avail.width() / ar) };
        size *= self.zoom;
        let img = Rect::from_center_size(avail.center() + self.pan, size);
        // Zoom per Scrollen
        if resp.hovered() {
            let (scroll, zd) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let f = (scroll * 0.002).exp() * zd;
            if (f - 1.0).abs() > 1e-4 {
                let old = self.zoom;
                self.zoom = (self.zoom * f).clamp(0.2, 30.0);
                if let Some(mp) = resp.hover_pos() {
                    let k = self.zoom / old;
                    let c = avail.center() + self.pan;
                    self.pan += (mp - c) * (1.0 - k);
                }
            }
        }
        let split_x = img.left() + img.width() * self.split;
        if resp.drag_started() {
            if let Some(p) = resp.interact_pointer_pos() {
                self.drag_split = self.view == View::Compare && (p.x - split_x).abs() < 24.0;
            }
        }
        if resp.dragged() {
            if self.drag_split {
                if let Some(p) = resp.interact_pointer_pos() {
                    self.split = ((p.x - img.left()) / img.width()).clamp(0.0, 1.0);
                }
            } else {
                self.pan += resp.drag_delta();
            }
        }
        if resp.drag_stopped() {
            self.drag_split = false;
        }
        if resp.double_clicked() {
            self.zoom = 1.0;
            self.pan = Vec2::ZERO;
        } else if resp.clicked() && self.view != View::Original {
            if let Some(p) = resp.interact_pointer_pos() {
                if img.contains(p) {
                    let x = ((p.x - img.left()) / img.width() * d.w as f32) as usize;
                    let y = ((p.y - img.top()) / img.height() * d.h as f32) as usize;
                    self.sel = Some((x.min(d.w - 1), y.min(d.h - 1)));
                    self.refresh_selection(ui.ctx());
                }
            }
        }
        let tex = match &self.tex {
            Some(t) => t,
            None => return,
        };
        let painter = ui.painter_at(rect);
        // Schatten und Karomuster (Transparenz)
        painter.rect_filled(img.translate(Vec2::new(0.0, 10.0)).expand(6.0), 10.0, Color32::from_black_alpha(if t.dark { 70 } else { 22 }));
        let clip = img.intersect(rect);
        let cp = painter.with_clip_rect(clip);
        let (c1, c2) = if t.dark { (Color32::from_rgb(31, 39, 51), Color32::from_rgb(28, 36, 49)) } else { (Color32::from_rgb(227, 231, 237), Color32::from_rgb(244, 246, 249)) };
        cp.rect_filled(img, 0.0, c1);
        let cell = 10.0;
        let (x0, y0) = (clip.left().max(img.left()), clip.top().max(img.top()));
        let mut y = y0 - ((y0 - img.top()) % cell);
        while y < clip.bottom() {
            let mut x = x0 - ((x0 - img.left()) % cell);
            while x < clip.right() {
                let ix = ((x - img.left()) / cell).round() as i64;
                let iy = ((y - img.top()) / cell).round() as i64;
                if (ix + iy) % 2 == 0 {
                    cp.rect_filled(Rect::from_min_size(Pos2::new(x, y), Vec2::splat(cell)), 0.0, c2);
                }
                x += cell;
            }
            y += cell;
        }
        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        let draw = |p: &egui::Painter, th: &TextureHandle| {
            p.image(th.id(), img, uv, Color32::WHITE);
        };
        match self.view {
            View::Original => draw(&cp, &tex.orig),
            View::Vector => {
                if let Some(v) = &tex.vec {
                    draw(&cp, v)
                }
            }
            View::Palette => {
                if let Some(v) = &tex.pal {
                    draw(&cp, v)
                }
            }
            View::Wire => {
                if let Some(v) = &tex.wire {
                    draw(&cp, v)
                }
            }
            View::Compare => {
                let left = Rect::from_min_max(img.min, Pos2::new(split_x, img.max.y)).intersect(clip);
                let right = Rect::from_min_max(Pos2::new(split_x, img.min.y), img.max).intersect(clip);
                draw(&painter.with_clip_rect(left), &tex.orig);
                if let Some(v) = &tex.vec {
                    draw(&painter.with_clip_rect(right), v);
                }
            }
        }
        if self.view != View::Original {
            if let Some(s) = &tex.sel {
                cp.image(s.id(), img, uv, Color32::WHITE);
            }
        }
        if self.view == View::Compare {
            let orange = Color32::from_rgb(249, 115, 22);
            painter.line_segment([Pos2::new(split_x, img.top().max(rect.top())), Pos2::new(split_x, img.bottom().min(rect.bottom()))], Stroke::new(2.0, orange));
            let hc = Pos2::new(split_x, img.center().y.clamp(rect.top() + 20.0, rect.bottom() - 20.0));
            painter.circle_filled(hc, 13.0, t.panel);
            painter.circle_filled(hc, 10.0, orange);
            for (txt, left) in [("ORIGINAL", true), ("VEKTOR", false)] {
                let font = egui::FontId::new(10.0, theme::semi());
                let tw = ui.fonts(|f| f.layout_no_wrap(txt.to_string(), font.clone(), t.muted).size().x);
                let pos = if left { Pos2::new(img.left().max(rect.left()) + 10.0, img.top().max(rect.top()) + 10.0) } else { Pos2::new(img.right().min(rect.right()) - 10.0 - tw - 16.0, img.top().max(rect.top()) + 10.0) };
                let r = Rect::from_min_size(pos, Vec2::new(tw + 16.0, 20.0));
                painter.rect_filled(r, 10.0, Color32::from_black_alpha(140));
                painter.text(r.center(), egui::Align2::CENTER_CENTER, txt, font, Color32::from_gray(225));
            }
            if (resp.hovered() || self.drag_split) && resp.hover_pos().is_some_and(|p| (p.x - split_x).abs() < 24.0) {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            }
        }
        // Ansichtsleiste oben
        let bar = Rect::from_min_size(Pos2::new(rect.left(), rect.top() + 16.0), Vec2::new(rect.width(), 40.0));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(bar).layout(egui::Layout::left_to_right(egui::Align::Center)));
        crate::floating_bar(&mut child, t, |ui| {
            theme::segmented(
                ui,
                t,
                &mut self.view,
                &[(View::Compare, "Vergleich"), (View::Vector, "Vektor"), (View::Palette, "Palette"), (View::Wire, "Konturen"), (View::Original, "Original")],
                false,
            );
        });
        if let Some(b) = &busy {
            let r = Rect::from_center_size(Pos2::new(rect.center().x, rect.top() + 80.0), Vec2::new(220.0, 30.0));
            ui.painter().rect_filled(r, 15.0, t.text);
            ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, format!("{b} …"), egui::FontId::new(12.5, theme::semi()), t.bg);
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
        }
        // Statistik unten links
        let (paths, nodes) = d.stats();
        let ells = d.layers.iter().flat_map(|l| l.subs.iter()).filter(|s| matches!(s, Shape::Ellipse(_))).count();
        let lr = Rect::from_min_size(rect.left_bottom() + Vec2::new(16.0, -46.0), Vec2::new(rect.width() - 80.0, 30.0));
        let mut lu = ui.new_child(egui::UiBuilder::new().max_rect(lr).layout(egui::Layout::left_to_right(egui::Align::Center)));
        for txt in [
            format!("{} Farben", d.layers.len()),
            fmt_count(paths, "Formen"),
            fmt_count(nodes, "Knoten"),
            format!("{ells} Kreise/Ellipsen"),
        ] {
            stat_chip(&mut lu, t, &txt);
        }
        for (n, what) in [(d.hand.grads.len(), "Verläufe"), (d.hand.lines.len(), "Linien"), (d.hand.texts, "Texte")] {
            if n > 0 {
                stat_chip(&mut lu, t, &format!("{n} {what}"));
            }
        }
        // Zoom unten rechts
        let zr = Rect::from_min_size(rect.right_bottom() - Vec2::new(50.0, 130.0), Vec2::new(34.0, 116.0));
        let mut zc = ui.new_child(egui::UiBuilder::new().max_rect(zr).layout(egui::Layout::top_down(egui::Align::Center)));
        zc.spacing_mut().item_spacing.y = 7.0;
        if theme::icon_button(&mut zc, t, Icon::Plus, "Vergrößern").clicked() {
            self.zoom = (self.zoom * 1.4).min(30.0);
        }
        if theme::icon_button(&mut zc, t, Icon::Minus, "Verkleinern").clicked() {
            self.zoom = (self.zoom / 1.4).max(0.2);
        }
        if theme::icon_button(&mut zc, t, Icon::Reset, "Einpassen (auch Doppelklick)").clicked() {
            self.zoom = 1.0;
            self.pan = Vec2::ZERO;
        }
    }
}

fn stat_chip(ui: &mut egui::Ui, t: &Tok, text: &str) {
    let font = egui::FontId::proportional(12.0);
    let tw = ui.fonts(|f| f.layout_no_wrap(text.to_string(), font.clone(), t.text).size().x);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(tw + 22.0, 26.0), Sense::hover());
    let fill = if t.dark { Color32::from_rgba_unmultiplied(22, 25, 31, 225) } else { Color32::from_rgba_unmultiplied(255, 255, 255, 235) };
    ui.painter().rect_filled(rect, 13.0, fill);
    ui.painter().rect_stroke(rect, 13.0, Stroke::new(1.0, t.line), egui::StrokeKind::Inside);
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, text, font, t.text);
}

/// Farbfeld zum Anklicken (None = transparent, kariert).
fn color_chip(ui: &mut egui::Ui, t: &Tok, c: Option<[u8; 3]>, selected: bool) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::click());
    match c {
        Some(c) => {
            ui.painter().rect_filled(r, 7.0, theme::c32(c));
        }
        None => {
            ui.painter().rect_filled(r, 7.0, Color32::from_gray(225));
            for (i, j) in [(0, 0), (1, 1)] {
                let q = Rect::from_min_size(r.min + Vec2::new(i as f32 * 14.0, j as f32 * 14.0), Vec2::splat(14.0));
                ui.painter().rect_filled(q, 0.0, Color32::from_gray(160));
            }
        }
    }
    ui.painter().rect_stroke(r, 7.0, Stroke::new(1.0, Color32::from_white_alpha(40)), egui::StrokeKind::Inside);
    if selected {
        ui.painter().rect_stroke(r.expand(3.0), 9.0, Stroke::new(2.0, t.text), egui::StrokeKind::Inside);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}
