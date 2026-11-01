//! Aussehen: Einstellungen (gespeichert), Farbtoken für Hell/Dunkel, Schrift und
//! eigene Bedienelemente im Stil der App.

use eframe::egui::{self, Color32, CornerRadius, FontFamily, FontId, Pos2, Rect, Response, RichText, Sense, Stroke, Ui, Vec2};
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ThemeMode {
    Light,
    Dark,
    System,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PreviewBg {
    Gradient,
    Dark,
    Light,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SplitFormat {
    Mf3,
    Stl,
    Step,
}

/// Bekannte Drucker: Name und Druckraum X × Y × Z (mm).
/// H2-Serie: gesamter Druckraum laut Hersteller (mit einer Düse bzw. beiden Düsen etwas kleiner).
pub const PRINTERS: [(&str, [f64; 3]); 10] = [
    ("Bambu Lab A1 mini", [180.0, 180.0, 180.0]),
    ("Bambu Lab A1", [256.0, 256.0, 256.0]),
    ("Bambu Lab P1S / P1P", [256.0, 256.0, 256.0]),
    ("Bambu Lab X1 Carbon", [256.0, 256.0, 256.0]),
    ("Bambu Lab H2S", [340.0, 320.0, 340.0]),
    ("Bambu Lab H2D", [350.0, 320.0, 320.0]),
    ("Bambu Lab H2C", [330.0, 320.0, 320.0]),
    ("Prusa MK4", [250.0, 210.0, 220.0]),
    ("Prusa Core One", [250.0, 220.0, 270.0]),
    ("Creality K1 Max", [300.0, 300.0, 300.0]),
];

/// Index des Druckers, dessen Druckraum zur Einstellung passt (erster Treffer).
pub fn printer_of(bed: [f64; 3]) -> Option<usize> {
    PRINTERS.iter().position(|(_, b)| (0..3).all(|k| (b[k] - bed[k]).abs() < 0.5))
}

/// Studio-Grün (Standard)
pub const STUDIO_GREEN: [u8; 3] = [0, 174, 66];

pub const ACCENTS: [[u8; 3]; 9] = [
    STUDIO_GREEN,
    [59, 130, 246],
    [139, 92, 246],
    [236, 72, 153],
    [239, 68, 68],
    [245, 158, 11],
    [34, 197, 94],
    [20, 184, 166],
    [100, 116, 139],
];

pub const DEFAULT_KIND_COLORS: [[u8; 3]; 7] = [
    [178, 180, 188],
    [76, 168, 92],
    [222, 170, 48],
    [160, 96, 210],
    [52, 172, 178],
    [70, 128, 230],
    [226, 82, 72],
];

/// Standard-Bauteilfarbe (Grau) und Auswahl in den Einstellungen
pub const DEFAULT_MODEL_COLOR: [u8; 3] = [172, 174, 180];
pub const MODEL_COLORS: [[u8; 3]; 7] = [DEFAULT_MODEL_COLOR, [236, 236, 236], [70, 74, 82], [245, 130, 32], [59, 130, 246], [34, 197, 94], [220, 60, 60]];

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub theme: ThemeMode,
    pub accent: [u8; 3],
    /// 0 = klein, 1 = normal, 2 = groß
    pub text_size: u8,
    pub preview_bg: PreviewBg,
    pub show_edges: bool,
    pub antialias: bool,
    pub kind_colors: [[u8; 3]; 7],
    /// Grundfarbe der Modelle ohne eigene Farbe in der Vorschau
    pub model_color: [u8; 3],
    /// Druckplatte in der Vorschau: 0 = strukturiertes PEI, 1 = glattes PEI, 2 = Cool Plate, 3 = Raster
    pub plate: u8,
    /// Druckbarkeit: Düse, Überhangwinkel, Mindestwand (mm/Grad), Druckraum (mm)
    pub nozzle: f64,
    pub overhang_deg: f64,
    pub min_wall: f64,
    pub bed: [f64; 3],
    pub out_dir: Option<PathBuf>,
    pub split_format: SplitFormat,
    pub reveal: bool,
    /// gewählter Slicer (None = automatisch, z. B. Bambu Studio)
    pub slicer: Option<PathBuf>,
    /// Engine für „Umwandeln“ (mesh2step, stl2step, breptile, freecad)
    pub engine: String,
    /// mehrere Engines vergleichen
    pub compare: bool,
    /// Engines im Vergleich
    pub compare_engines: Vec<String>,
    /// stl2step: TrueForm (sonst Verbatim)
    pub trueform: bool,
    /// breptile: auto, prismatic, tessellated
    pub breptile_mode: String,
    /// FreeCAD: ebene Flächen zusammenfassen
    pub freecad_refine: bool,
    /// Engine zum Lesen fremder STEP-Dateien im Maßvergleich
    pub step_reader: String,
    /// beim Start still nach Engine-Updates suchen
    pub engine_updates: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: ThemeMode::Dark,
            accent: STUDIO_GREEN,
            text_size: 1,
            preview_bg: PreviewBg::Gradient,
            show_edges: true,
            antialias: true,
            kind_colors: DEFAULT_KIND_COLORS,
            model_color: DEFAULT_MODEL_COLOR,
            plate: 0,
            nozzle: 0.4,
            overhang_deg: 45.0,
            min_wall: 0.8,
            bed: [256.0, 256.0, 256.0],
            out_dir: None,
            split_format: SplitFormat::Mf3,
            reveal: true,
            engine: "mesh2step".into(),
            compare: false,
            compare_engines: vec!["mesh2step".into(), "stl2step".into()],
            trueform: true,
            breptile_mode: "auto".into(),
            freecad_refine: true,
            step_reader: "mesh2step".into(),
            engine_updates: true,
            slicer: None,
        }
    }
}

fn config_path() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support/ZeichneMichNach/einstellungen.txt"))
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA").map(|h| PathBuf::from(h).join("ZeichneMichNach").join("einstellungen.txt"))
    }
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config/zeichnemichnach/einstellungen.txt"))
    }
}

fn parse_rgb(s: &str) -> Option<[u8; 3]> {
    let h = s.trim().trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    let p = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    Some([p(0)?, p(2)?, p(4)?])
}

fn rgb_hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

impl Settings {
    pub fn load() -> Settings {
        let mut s = Settings::default();
        let text = match config_path().and_then(|p| std::fs::read_to_string(p).ok()) {
            Some(t) => t,
            None => return s,
        };
        let studio = text.lines().any(|l| l.trim() == "studio=1");
        for line in text.lines() {
            let (k, v) = match line.split_once('=') {
                Some(x) => (x.0.trim(), x.1.trim()),
                None => continue,
            };
            // erster Start von Mesh2STEP Studio: altes Design und Akzent nicht übernehmen
            if !studio && (k == "design" || k == "akzent" || k == "hintergrund") {
                continue;
            }
            match k {
                "design" => {
                    s.theme = match v {
                        "hell" => ThemeMode::Light,
                        "system" => ThemeMode::System,
                        _ => ThemeMode::Dark,
                    }
                }
                "akzent" => s.accent = parse_rgb(v).unwrap_or(s.accent),
                "bauteilfarbe" => s.model_color = parse_rgb(v).unwrap_or(s.model_color),
                "druckplatte" => s.plate = v.parse().unwrap_or(0).min(3),
                "duese" => s.nozzle = v.parse().unwrap_or(s.nozzle),
                "ueberhang" => s.overhang_deg = v.parse().unwrap_or(s.overhang_deg),
                "mindestwand" => s.min_wall = v.parse().unwrap_or(s.min_wall),
                "druckraum" => {
                    let p: Vec<f64> = v.split('x').filter_map(|x| x.trim().parse().ok()).collect();
                    if p.len() == 3 {
                        s.bed = [p[0], p[1], p[2]];
                    }
                }
                "textgroesse" => s.text_size = v.parse().unwrap_or(1).min(2),
                "hintergrund" => {
                    s.preview_bg = match v {
                        "dunkel" => PreviewBg::Dark,
                        "hell" => PreviewBg::Light,
                        _ => PreviewBg::Gradient,
                    }
                }
                "grenzen" => s.show_edges = v == "1",
                "glaetten" => s.antialias = v == "1",
                "speicherort" => s.out_dir = if v.is_empty() { None } else { Some(PathBuf::from(v)) },
                "format_teilen" => {
                    s.split_format = match v {
                        "stl" => SplitFormat::Stl,
                        "step" => SplitFormat::Step,
                        _ => SplitFormat::Mf3,
                    }
                }
                "zeigen" => s.reveal = v == "1",
                "slicer" => s.slicer = if v.is_empty() { None } else { Some(PathBuf::from(v)) },
                "engine" => s.engine = v.to_string(),
                "vergleich" => s.compare = v == "1",
                "vergleich_engines" => s.compare_engines = v.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect(),
                "trueform" => s.trueform = v == "1",
                "breptile_modus" => s.breptile_mode = v.to_string(),
                "freecad_zusammenfassen" => s.freecad_refine = v == "1",
                "step_leser" => s.step_reader = v.to_string(),
                "engine_updates" => s.engine_updates = v == "1",
                _ => {
                    if let Some(i) = k.strip_prefix("flaeche").and_then(|x| x.parse::<usize>().ok()) {
                        if i < 7 {
                            if let Some(c) = parse_rgb(v) {
                                s.kind_colors[i] = c;
                            }
                        }
                    }
                }
            }
        }
        s
    }

    pub fn save(&self) {
        let p = match config_path() {
            Some(p) => p,
            None => return,
        };
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let mut t = String::from("studio=1\n");
        t += &format!(
            "design={}\n",
            match self.theme {
                ThemeMode::Light => "hell",
                ThemeMode::Dark => "dunkel",
                ThemeMode::System => "system",
            }
        );
        t += &format!("akzent={}\n", rgb_hex(self.accent));
        t += &format!("bauteilfarbe={}\n", rgb_hex(self.model_color));
        t += &format!("druckplatte={}\n", self.plate);
        t += &format!("duese={}\nueberhang={}\nmindestwand={}\n", self.nozzle, self.overhang_deg, self.min_wall);
        t += &format!("druckraum={}x{}x{}\n", self.bed[0], self.bed[1], self.bed[2]);
        t += &format!("textgroesse={}\n", self.text_size);
        t += &format!(
            "hintergrund={}\n",
            match self.preview_bg {
                PreviewBg::Gradient => "verlauf",
                PreviewBg::Dark => "dunkel",
                PreviewBg::Light => "hell",
            }
        );
        t += &format!("grenzen={}\n", self.show_edges as u8);
        t += &format!("glaetten={}\n", self.antialias as u8);
        t += &format!("speicherort={}\n", self.out_dir.as_ref().map(|p| p.display().to_string()).unwrap_or_default());
        t += &format!(
            "format_teilen={}\n",
            match self.split_format {
                SplitFormat::Mf3 => "3mf",
                SplitFormat::Stl => "stl",
                SplitFormat::Step => "step",
            }
        );
        t += &format!("zeigen={}\n", self.reveal as u8);
        t += &format!("slicer={}\n", self.slicer.as_ref().map(|p| p.display().to_string()).unwrap_or_default());
        t += &format!("engine={}\nvergleich={}\nvergleich_engines={}\n", self.engine, self.compare as u8, self.compare_engines.join(","));
        t += &format!("trueform={}\nbreptile_modus={}\nfreecad_zusammenfassen={}\n", self.trueform as u8, self.breptile_mode, self.freecad_refine as u8);
        t += &format!("step_leser={}\nengine_updates={}\n", self.step_reader, self.engine_updates as u8);
        for (i, c) in self.kind_colors.iter().enumerate() {
            t += &format!("flaeche{i}={}\n", rgb_hex(*c));
        }
        let _ = std::fs::write(p, t);
    }
}

/// Farbtoken für das aktuelle Erscheinungsbild.
#[derive(Clone, Copy)]
pub struct Tok {
    pub dark: bool,
    /// Kopfleiste (wie in Bambu Studio dunkel, auch im hellen Design)
    pub top: Color32,
    pub bg: Color32,
    pub panel: Color32,
    pub card: Color32,
    pub card2: Color32,
    pub line: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub faint: Color32,
    pub accent: Color32,
    pub accent_soft: Color32,
    pub ok: Color32,
    pub warn: Color32,
    pub err: Color32,
}

pub fn tok(ui: &Ui, s: &Settings) -> Tok {
    tok_for(ui.visuals().dark_mode, s)
}

pub fn tok_for(dark: bool, s: &Settings) -> Tok {
    let a = Color32::from_rgb(s.accent[0], s.accent[1], s.accent[2]);
    if dark {
        Tok {
            dark,
            top: Color32::from_rgb(28, 29, 32),
            bg: Color32::from_rgb(38, 40, 44),
            panel: Color32::from_rgb(45, 46, 51),
            card: Color32::from_rgb(52, 53, 58),
            card2: Color32::from_rgb(62, 64, 70),
            line: Color32::from_rgb(70, 72, 78),
            text: Color32::from_rgb(230, 231, 234),
            muted: Color32::from_rgb(162, 165, 172),
            faint: Color32::from_rgb(120, 124, 131),
            accent: a,
            accent_soft: Color32::from_rgba_unmultiplied(s.accent[0], s.accent[1], s.accent[2], 40),
            ok: Color32::from_rgb(74, 222, 128),
            warn: Color32::from_rgb(251, 191, 36),
            err: Color32::from_rgb(248, 113, 113),
        }
    } else {
        Tok {
            dark,
            top: Color32::from_rgb(43, 47, 51),
            bg: Color32::from_rgb(236, 237, 240),
            panel: Color32::from_rgb(255, 255, 255),
            card: Color32::from_rgb(255, 255, 255),
            card2: Color32::from_rgb(244, 244, 245),
            line: Color32::from_rgb(226, 227, 231),
            text: Color32::from_rgb(38, 46, 48),
            muted: Color32::from_rgb(107, 114, 128),
            faint: Color32::from_rgb(156, 163, 175),
            accent: a,
            accent_soft: Color32::from_rgba_unmultiplied(s.accent[0], s.accent[1], s.accent[2], 30),
            ok: Color32::from_rgb(0, 150, 57),
            warn: Color32::from_rgb(196, 110, 0),
            err: Color32::from_rgb(214, 48, 49),
        }
    }
}

pub fn semi() -> FontFamily {
    FontFamily::Name("semi".into())
}

/// Text in halbfetter Schrift.
pub fn strong(text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text).family(semi()).size(size)
}

pub fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts
        .font_data
        .insert("Inter".into(), std::sync::Arc::new(egui::FontData::from_static(include_bytes!("../assets/Inter-Regular.otf"))));
    fonts
        .font_data
        .insert("Inter-SemiBold".into(), std::sync::Arc::new(egui::FontData::from_static(include_bytes!("../assets/Inter-SemiBold.otf"))));
    let fallback = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "Inter".into());
    let mut semi_list = vec!["Inter-SemiBold".to_string()];
    semi_list.extend(fallback);
    fonts.families.insert(semi(), semi_list);
    ctx.set_fonts(fonts);
}

fn visuals_for(dark: bool, s: &Settings) -> egui::Visuals {
    let t = tok_for(dark, s);
    let mut v = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    v.panel_fill = t.panel;
    v.window_fill = t.panel;
    v.extreme_bg_color = t.bg;
    v.faint_bg_color = t.card;
    v.code_bg_color = t.card2;
    v.window_stroke = Stroke::new(1.0, t.line);
    v.hyperlink_color = t.accent;
    v.selection.bg_fill = t.accent;
    v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    v.window_corner_radius = CornerRadius::same(8);
    v.menu_corner_radius = CornerRadius::same(6);
    let r = CornerRadius::same(4);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = t.panel;
    w.noninteractive.weak_bg_fill = t.card;
    w.noninteractive.bg_stroke = Stroke::new(1.0, t.line);
    w.noninteractive.fg_stroke = Stroke::new(1.0, t.text);
    w.noninteractive.corner_radius = r;
    w.inactive.bg_fill = t.card2;
    w.inactive.weak_bg_fill = t.card2;
    w.inactive.bg_stroke = Stroke::new(1.0, t.line);
    w.inactive.fg_stroke = Stroke::new(1.0, t.text);
    w.inactive.corner_radius = r;
    w.hovered.bg_fill = t.card2;
    w.hovered.weak_bg_fill = t.card2;
    w.hovered.bg_stroke = Stroke::new(1.0, t.accent);
    w.hovered.fg_stroke = Stroke::new(1.5, t.text);
    w.hovered.corner_radius = r;
    w.active.bg_fill = t.accent;
    w.active.weak_bg_fill = t.accent;
    w.active.bg_stroke = Stroke::new(1.0, t.accent);
    w.active.fg_stroke = Stroke::new(1.5, Color32::WHITE);
    w.active.corner_radius = r;
    w.open.bg_fill = t.card2;
    w.open.weak_bg_fill = t.card2;
    w.open.bg_stroke = Stroke::new(1.0, t.line);
    w.open.corner_radius = r;
    v.slider_trailing_fill = true;
    v
}

/// Erscheinungsbild auf den Kontext anwenden.
pub fn apply(ctx: &egui::Context, s: &Settings) {
    ctx.set_theme(match s.theme {
        ThemeMode::Light => egui::ThemePreference::Light,
        ThemeMode::Dark => egui::ThemePreference::Dark,
        ThemeMode::System => egui::ThemePreference::System,
    });
    let scale = match s.text_size {
        0 => 0.92,
        2 => 1.14,
        _ => 1.0,
    };
    for (theme, dark) in [(egui::Theme::Dark, true), (egui::Theme::Light, false)] {
        let mut style = (*ctx.style_of(theme)).clone();
        style.visuals = visuals_for(dark, s);
        use egui::TextStyle as T;
        style.text_styles = [
            (T::Small, FontId::proportional(11.5 * scale)),
            (T::Body, FontId::proportional(13.5 * scale)),
            (T::Button, FontId::proportional(13.5 * scale)),
            (T::Heading, FontId::new(19.0 * scale, semi())),
            (T::Monospace, FontId::monospace(12.5 * scale)),
        ]
        .into();
        style.spacing.item_spacing = Vec2::new(8.0, 7.0);
        style.spacing.button_padding = Vec2::new(10.0, 5.0);
        style.spacing.interact_size.y = 26.0 * scale;
        style.spacing.slider_width = 170.0;
        style.spacing.scroll.bar_width = 6.0;
        ctx.set_style_of(theme, style);
    }
}

// ---------------------------------------------------------------------------
// Bedienelemente

pub fn card<R>(ui: &mut Ui, t: &Tok, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .fill(t.card)
        .stroke(Stroke::new(1.0, t.line))
        .corner_radius(8.0)
        .inner_margin(12.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// Abschnittsüberschrift im Studio-Stil: graue Leiste mit grünem Kästchen und Titel.
pub fn step(ui: &mut Ui, t: &Tok, _n: u32, text: &str) {
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(Vec2::new(w, 32.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(r, 6.0, t.card2);
    let sq = Rect::from_center_size(Pos2::new(r.left() + 16.0, r.center().y), Vec2::splat(14.0));
    p.rect_filled(sq, 3.0, t.accent);
    p.text(Pos2::new(sq.right() + 9.0, r.center().y), egui::Align2::LEFT_CENTER, text, FontId::new(13.5, semi()), t.text);
    ui.add_space(6.0);
}

/// Segmentierte Auswahl. Rückgabe: geändert.
pub fn segmented<T: PartialEq + Copy>(ui: &mut Ui, t: &Tok, value: &mut T, options: &[(T, &str)], full: bool) -> bool {
    let font = FontId::proportional(13.0);
    let pad = 14.0;
    let widths: Vec<f32> = options
        .iter()
        .map(|(_, s)| ui.fonts(|f| f.layout_no_wrap(s.to_string(), font.clone(), t.text).size().x) + pad * 2.0)
        .collect();
    let total: f32 = widths.iter().sum::<f32>() + 6.0;
    let w = if full { ui.available_width() } else { total };
    let h = 32.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 6.0, t.card2);
    p.rect_stroke(rect, 6.0, Stroke::new(1.0, t.line), egui::StrokeKind::Inside);
    let scale = (w - 6.0) / (total - 6.0);
    let mut x = rect.left() + 3.0;
    let mut changed = false;
    // ui.id() ist bei Geschwister-Bereichen gleich (z. B. zwei Spalten) – deshalb alle
    // Beschriftungen einbeziehen, sonst teilen sich gleich beschriftete Knöpfe einen
    // Klick („Dunkel“ bei Design und bei Vorschau-Hintergrund).
    let base = options.iter().fold(ui.id().with("seg"), |id, (_, l)| id.with(*l));
    for (i, (val, label)) in options.iter().enumerate() {
        let sw = widths[i] * scale;
        let r = Rect::from_min_size(Pos2::new(x, rect.top() + 3.0), Vec2::new(sw, h - 6.0));
        let resp = ui.interact(r, base.with(i), Sense::click());
        let sel = *value == *val;
        if sel {
            ui.painter().rect_filled(r, 4.0, t.accent);
        } else if resp.hovered() {
            ui.painter().rect_filled(r, 4.0, t.line);
        }
        ui.painter().text(
            r.center(),
            egui::Align2::CENTER_CENTER,
            *label,
            font.clone(),
            if sel { Color32::WHITE } else if resp.hovered() { t.text } else { t.muted },
        );
        if resp.clicked() && !sel {
            *value = *val;
            changed = true;
        }
        x += sw;
    }
    changed
}

/// Kästchen zum Anhaken. Rückgabe: geändert.
pub fn checkbox(ui: &mut Ui, t: &Tok, on: &mut bool) -> bool {
    let (r, resp) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::click());
    let changed = resp.clicked();
    if changed {
        *on = !*on;
    }
    if *on {
        ui.painter().rect_filled(r, 4.0, t.accent);
        let p = |x: f32, y: f32| Pos2::new(r.left() + x, r.top() + y);
        ui.painter().line_segment([p(4.5, 9.5), p(7.5, 12.5)], Stroke::new(2.0, Color32::WHITE));
        ui.painter().line_segment([p(7.5, 12.5), p(13.5, 5.5)], Stroke::new(2.0, Color32::WHITE));
    } else {
        ui.painter().rect_stroke(r, 4.0, Stroke::new(1.4, t.faint), egui::StrokeKind::Inside);
    }
    changed
}

/// Schalter (An/Aus).
pub fn toggle(ui: &mut Ui, t: &Tok, on: &mut bool) -> Response {
    let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(36.0, 20.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let k = ui.ctx().animate_bool(resp.id, *on);
    let off = if t.dark { Color32::from_rgb(58, 65, 80) } else { Color32::from_rgb(207, 212, 220) };
    let bg = lerp_color(off, t.accent, k);
    ui.painter().rect_filled(rect, 10.0, bg);
    let x = egui::lerp(rect.left() + 10.0..=rect.right() - 10.0, k);
    ui.painter().circle_filled(Pos2::new(x, rect.center().y), 8.0, Color32::WHITE);
    resp
}

pub fn lerp_color(a: Color32, b: Color32, k: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * k).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

/// Ergebnis einer Schaltfläche (nur „angeklickt?“).
pub struct Click(bool);

impl Click {
    pub fn clicked(&self) -> bool {
        self.0
    }
}

/// Hauptaktion des aktuellen Bereichs (wird oben rechts in der Kopfleiste gezeigt, wie
/// „Platte slicen“). Die erste breite Hauptschaltfläche eines Bildes wird dorthin verlegt.
#[derive(Clone, Default)]
pub struct MainAction {
    pub label: String,
    pub enabled: bool,
    pub pass: u64,
}

fn main_ids() -> (egui::Id, egui::Id) {
    (egui::Id::new("m2s_main_action"), egui::Id::new("m2s_main_click"))
}

/// Hauptaktion aus dem letzten Bild (für die Kopfleiste).
pub fn main_action(ctx: &egui::Context) -> Option<MainAction> {
    let a: Option<MainAction> = ctx.data(|d| d.get_temp(main_ids().0));
    let now = ctx.cumulative_pass_nr();
    a.filter(|a| a.pass + 2 >= now)
}

/// Kopfleiste meldet: Hauptaktion angeklickt (wird im nächsten Durchlauf ausgelöst).
pub fn click_main_action(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp(main_ids().1, true));
    ctx.request_repaint();
}

/// Hauptschaltfläche (Akzentfarbe). Die erste breite Hauptschaltfläche im Bild erscheint
/// nicht im Bereich selbst, sondern oben rechts in der Kopfleiste.
pub fn primary(ui: &mut Ui, t: &Tok, text: &str, enabled: bool, full: bool) -> Click {
    let ctx = ui.ctx().clone();
    let now = ctx.cumulative_pass_nr();
    let (aid, cid) = main_ids();
    if full {
        let prev: Option<MainAction> = ctx.data(|d| d.get_temp(aid));
        if prev.as_ref().map(|p| p.pass) != Some(now) {
            ctx.data_mut(|d| d.insert_temp(aid, MainAction { label: text.to_string(), enabled, pass: now }));
            let clicked = ctx.data_mut(|d| d.remove_temp::<bool>(cid)).unwrap_or(false);
            return Click(clicked && enabled);
        }
    }
    Click(primary_button(ui, t, text, enabled, full).clicked())
}

/// Grüne Schaltfläche (wird auch von der Kopfleiste benutzt).
pub fn primary_button(ui: &mut Ui, t: &Tok, text: &str, enabled: bool, full: bool) -> Response {
    let font = FontId::new(13.5, semi());
    let tw = ui.fonts(|f| f.layout_no_wrap(text.to_string(), font.clone(), Color32::WHITE).size().x);
    let w = if full { ui.available_width() } else { tw + 32.0 };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, if full { 36.0 } else { 30.0 }), if enabled { Sense::click() } else { Sense::hover() });
    let mut c = t.accent;
    if !enabled {
        c = c.gamma_multiply(0.4);
    } else if resp.hovered() {
        c = lerp_color(c, Color32::BLACK, 0.08);
    }
    ui.painter().rect_filled(rect, 6.0, c);
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, text, font, Color32::WHITE);
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// Zweitrangige Schaltfläche (Rahmen).
pub fn ghost(ui: &mut Ui, t: &Tok, text: &str, enabled: bool) -> Response {
    let font = FontId::proportional(13.0);
    let tw = ui.fonts(|f| f.layout_no_wrap(text.to_string(), font.clone(), t.text).size().x);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(tw + 24.0, 30.0), if enabled { Sense::click() } else { Sense::hover() });
    let fill = if resp.hovered() && enabled { t.card2 } else { Color32::TRANSPARENT };
    ui.painter().rect_filled(rect, 6.0, fill);
    ui.painter().rect_stroke(rect, 6.0, Stroke::new(1.0, t.line), egui::StrokeKind::Inside);
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, text, font, if enabled { t.text } else { t.faint });
    resp
}

/// Symbol einer Aktionskarte.
#[derive(Clone, Copy)]
pub enum ActIcon {
    Slicer,
    Prepare,
    Step,
    Save,
    Check,
    Repair,
    Split,
    Texture,
}

impl ActIcon {
    pub fn color(self) -> Color32 {
        match self {
            ActIcon::Slicer => Color32::from_rgb(56, 189, 248),
            ActIcon::Prepare => Color32::from_rgb(251, 191, 36),
            ActIcon::Step => Color32::from_rgb(167, 139, 250),
            ActIcon::Save => Color32::from_rgb(74, 222, 128),
            ActIcon::Check => Color32::from_rgb(45, 212, 191),
            ActIcon::Repair => Color32::from_rgb(248, 113, 113),
            ActIcon::Split => Color32::from_rgb(244, 114, 182),
            ActIcon::Texture => Color32::from_rgb(251, 191, 36),
        }
    }
}

fn draw_act_icon(p: &egui::Painter, icon: ActIcon, c: Pos2, col: Color32) {
    let at = |x: f32, y: f32| Pos2::new(c.x + x, c.y + y);
    match icon {
        ActIcon::Slicer => {
            for i in 0..4 {
                let y = -6.0 + i as f32 * 4.0;
                p.rect_filled(Rect::from_min_max(at(-7.0, y), at(7.0, y + 2.2)), 1.0, col);
            }
        }
        ActIcon::Prepare => {
            p.add(egui::Shape::convex_polygon(vec![at(0.0, -8.0), at(6.0, -2.0), at(-6.0, -2.0)], col, Stroke::NONE));
            p.rect_filled(Rect::from_min_max(at(-2.0, -2.5), at(2.0, 3.0)), 0.0, col);
            p.rect_filled(Rect::from_min_max(at(-8.0, 5.0), at(8.0, 8.0)), 1.0, col);
        }
        ActIcon::Step => {
            let dark = |k: f32| lerp_color(col, Color32::BLACK, k);
            p.add(egui::Shape::convex_polygon(vec![at(0.0, -8.0), at(7.0, -4.0), at(0.0, 0.0), at(-7.0, -4.0)], col, Stroke::NONE));
            p.add(egui::Shape::convex_polygon(vec![at(-7.0, -4.0), at(0.0, 0.0), at(0.0, 8.0), at(-7.0, 4.0)], dark(0.25), Stroke::NONE));
            p.add(egui::Shape::convex_polygon(vec![at(7.0, -4.0), at(7.0, 4.0), at(0.0, 8.0), at(0.0, 0.0)], dark(0.45), Stroke::NONE));
        }
        ActIcon::Check => {
            p.circle_stroke(at(-1.5, -1.5), 5.5, Stroke::new(2.0, col));
            p.line_segment([at(2.5, 2.5), at(7.5, 7.5)], Stroke::new(2.6, col));
        }
        ActIcon::Repair => {
            p.add(egui::Shape::convex_polygon(vec![at(-8.0, 3.0), at(3.0, -8.0), at(8.0, -3.0), at(-3.0, 8.0)], col, Stroke::NONE));
            p.rect_filled(Rect::from_center_size(at(0.0, 0.0), Vec2::splat(4.0)), 1.0, lerp_color(col, Color32::WHITE, 0.55));
        }
        ActIcon::Texture => {
            for i in 0..3 {
                for j in 0..3 {
                    let c = at(-5.5 + i as f32 * 5.5, -5.5 + j as f32 * 5.5);
                    p.add(egui::Shape::convex_polygon(vec![c + Vec2::new(0.0, -2.6), c + Vec2::new(2.6, 0.0), c + Vec2::new(0.0, 2.6), c + Vec2::new(-2.6, 0.0)], col, Stroke::NONE));
                }
            }
        }
        ActIcon::Split => {
            p.rect_filled(Rect::from_min_max(at(-8.0, -6.0), at(-1.5, 6.0)), 1.5, col);
            p.rect_filled(Rect::from_min_max(at(1.5, -6.0), at(8.0, 6.0)), 1.5, lerp_color(col, Color32::BLACK, 0.3));
        }
        ActIcon::Save => {
            p.add(egui::Shape::convex_polygon(vec![at(0.0, 3.0), at(-6.0, -3.0), at(6.0, -3.0)], col, Stroke::NONE));
            p.rect_filled(Rect::from_min_max(at(-2.0, -8.0), at(2.0, -2.5)), 0.0, col);
            p.rect_filled(Rect::from_min_max(at(-8.0, 5.0), at(8.0, 8.0)), 1.0, col);
        }
    }
}

/// Aktionskarte: farbiges Symbol, Titel, kurze Erklärung, Pfeil – für „Weiter mit …“-Schritte.
pub fn action_card(ui: &mut Ui, t: &Tok, icon: ActIcon, title: &str, sub: &str, enabled: bool) -> Response {
    let col = icon.color();
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 56.0), if enabled { Sense::click() } else { Sense::hover() });
    let hov = enabled && resp.hovered();
    let p = ui.painter();
    let fill = if hov { t.card2 } else { t.card };
    let stroke = if hov { lerp_color(col, t.line, 0.55) } else { t.line };
    p.rect_filled(rect, 8.0, fill);
    p.rect_stroke(rect, 8.0, Stroke::new(1.0, stroke), egui::StrokeKind::Inside);
    let fade = |c: Color32| if enabled { c } else { c.gamma_multiply(0.45) };
    let tile = Rect::from_center_size(Pos2::new(rect.left() + 28.0, rect.center().y), Vec2::splat(32.0));
    p.rect_filled(tile, 9.0, fade(lerp_color(col, t.card, 0.80)));
    draw_act_icon(p, icon, tile.center(), fade(col));
    let tx = tile.right() + 12.0;
    let max_w = rect.right() - 30.0 - tx;
    let title_g = p.layout(title.to_string(), FontId::new(13.5, semi()), fade(t.text), max_w);
    let sub_g = p.layout(sub.to_string(), FontId::proportional(11.0), fade(t.muted), max_w);
    let th = title_g.size().y + 2.0 + if sub.is_empty() { 0.0 } else { sub_g.size().y };
    let ty = rect.center().y - th / 2.0;
    p.galley(Pos2::new(tx, ty), title_g.clone(), t.text);
    if !sub.is_empty() {
        p.galley(Pos2::new(tx, ty + title_g.size().y + 2.0), sub_g, t.muted);
    }
    let cx = rect.right() - 16.0;
    let cy = rect.center().y;
    let cc = if hov { col } else { t.faint };
    p.line_segment([Pos2::new(cx - 3.0, cy - 5.0), Pos2::new(cx + 2.0, cy)], Stroke::new(2.0, cc));
    p.line_segment([Pos2::new(cx + 2.0, cy), Pos2::new(cx - 3.0, cy + 5.0)], Stroke::new(2.0, cc));
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// Kleine Überschrift über einer Gruppe von Aktionskarten.
pub fn group_label(ui: &mut Ui, t: &Tok, text: &str) {
    ui.label(RichText::new(text.to_uppercase()).family(semi()).size(10.5).color(t.faint));
}

#[derive(Clone, Copy)]
pub enum BadgeKind {
    Ok,
    Warn,
    Info,
    Muted,
    Err,
}

pub fn badge(ui: &mut Ui, t: &Tok, text: &str, kind: BadgeKind) {
    let (fg, bg) = match kind {
        BadgeKind::Ok => (t.ok, t.ok.gamma_multiply(0.16)),
        BadgeKind::Warn => (t.warn, t.warn.gamma_multiply(0.16)),
        BadgeKind::Err => (t.err, t.err.gamma_multiply(0.16)),
        BadgeKind::Info => (t.accent, t.accent_soft),
        BadgeKind::Muted => (t.muted, t.card2),
    };
    let font = FontId::new(10.5, semi());
    let tw = ui.fonts(|f| f.layout_no_wrap(text.to_string(), font.clone(), fg).size().x);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(tw + 14.0, 18.0), Sense::hover());
    ui.painter().rect_filled(rect, 9.0, bg);
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, text, font, fg);
}

pub fn chip(ui: &mut Ui, t: &Tok, text: &str) {
    let font = FontId::proportional(11.0);
    let tw = ui.fonts(|f| f.layout_no_wrap(text.to_string(), font.clone(), t.muted).size().x);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(tw + 14.0, 18.0), Sense::hover());
    ui.painter().rect_filled(rect, 9.0, t.card2);
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, text, font, t.muted);
}

pub fn swatch(ui: &mut Ui, c: [u8; 3], size: f32) -> Response {
    let (r, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    ui.painter().rect_filled(r, size * 0.28, Color32::from_rgb(c[0], c[1], c[2]));
    ui.painter().rect_stroke(r, size * 0.28, Stroke::new(1.0, Color32::from_rgba_unmultiplied(128, 128, 128, 110)), egui::StrokeKind::Inside);
    resp
}

/// Kleine Hinweiszeile.
pub fn hint(ui: &mut Ui, t: &Tok, text: &str) {
    ui.label(RichText::new(text).size(11.5).color(t.faint));
}

/// Beschriftung + Wert rechts (für Regler).
pub fn label_value(ui: &mut Ui, t: &Tok, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(12.5).color(t.text));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Wert in einem Feld wie bei den Prozess-Einstellungen
            let font = FontId::proportional(12.5);
            let tw = ui.fonts(|f| f.layout_no_wrap(value.to_string(), font.clone(), t.text).size().x);
            let (r, _) = ui.allocate_exact_size(Vec2::new((tw + 20.0).max(84.0), 24.0), Sense::hover());
            ui.painter().rect_filled(r, 4.0, if t.dark { t.bg } else { t.panel });
            ui.painter().rect_stroke(r, 4.0, Stroke::new(1.0, t.line), egui::StrokeKind::Inside);
            ui.painter().text(Pos2::new(r.left() + 10.0, r.center().y), egui::Align2::LEFT_CENTER, value, font, t.text);
        });
    });
}

/// Ganze Zeile: Text links, beliebiges Element rechts.
pub fn row<R>(ui: &mut Ui, t: &Tok, label: &str, right: impl FnOnce(&mut Ui) -> R) -> R {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).color(t.text));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), right).inner
    })
    .inner
}

/// Kleines Schaubild auf einer Auswahlkarte (wie bei split3mf).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Pict {
    Patch,
    Interlock,
    ConnNone,
    ConnTri,
    ConnCyl,
    ConnBox,
    SidePart,
    SideBody,
}

/// Konvexes Vieleck mit 45°-Schraffur füllen und umranden.
fn hatch_poly(p: &egui::Painter, poly: &[Pos2], hatch: Color32, outline: Color32) {
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for q in poly {
        lo = lo.min(q.x + q.y);
        hi = hi.max(q.x + q.y);
    }
    let mut k = lo + 2.5;
    while k < hi {
        // Schnittpunkte der Linie x + y = k mit den Kanten
        let mut pts: Vec<Pos2> = Vec::new();
        for i in 0..poly.len() {
            let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
            let (fa, fb) = (a.x + a.y - k, b.x + b.y - k);
            if (fa <= 0.0) != (fb <= 0.0) {
                let s = fa / (fa - fb);
                pts.push(Pos2::new(a.x + (b.x - a.x) * s, a.y + (b.y - a.y) * s));
            }
        }
        if pts.len() >= 2 {
            p.line_segment([pts[0], pts[1]], Stroke::new(1.0, hatch));
        }
        k += 4.5;
    }
    p.add(egui::Shape::closed_line(poly.to_vec(), Stroke::new(1.3, outline)));
}

fn draw_pict(p: &egui::Painter, t: &Tok, pict: Pict, c: Pos2, bg: Color32) {
    let at = |x: f32, y: f32| Pos2::new(c.x + x, c.y + y);
    let (h, o) = (t.muted, t.text);
    let line = |a: Pos2, b: Pos2| p.line_segment([a, b], Stroke::new(1.3, o));
    let rect_poly = |x0: f32, y0: f32, x1: f32, y1: f32| vec![at(x0, y0), at(x1, y0), at(x1, y1), at(x0, y1)];
    match pict {
        Pict::Patch => {
            hatch_poly(p, &[at(-20.0, 15.0), at(-7.0, -15.0), at(15.0, -15.0), at(2.0, 15.0)], h, o);
            line(at(-30.0, 15.0), at(-20.0, 15.0));
            line(at(15.0, -15.0), at(26.0, -15.0));
        }
        Pict::Interlock => {
            hatch_poly(p, &[at(-20.0, 15.0), at(-7.0, -15.0), at(15.0, -15.0), at(22.0, -3.0), at(2.0, 15.0)], h, o);
            line(at(-30.0, 15.0), at(-20.0, 15.0));
        }
        Pict::ConnNone | Pict::ConnTri | Pict::ConnCyl | Pict::ConnBox => {
            hatch_poly(p, &rect_poly(-26.0, -14.0, 0.0, 14.0), h, o);
            p.add(egui::Shape::closed_line(rect_poly(-26.0, -14.0, 26.0, 14.0), Stroke::new(1.3, o)));
            match pict {
                Pict::ConnTri => {
                    p.add(egui::Shape::convex_polygon(vec![at(0.0, -8.0), at(12.0, 0.0), at(0.0, 8.0)], bg, Stroke::new(1.3, o)));
                }
                Pict::ConnCyl => {
                    p.circle(at(5.0, 0.0), 7.0, bg, Stroke::new(1.3, o));
                }
                Pict::ConnBox => {
                    p.rect(Rect::from_min_max(at(0.0, -7.0), at(11.0, 7.0)), 0.0, bg, Stroke::new(1.3, o), egui::StrokeKind::Middle);
                }
                _ => {}
            }
        }
        Pict::SidePart => {
            hatch_poly(p, &rect_poly(-26.0, -14.0, 0.0, 14.0), h, o);
            hatch_poly(p, &rect_poly(0.0, -6.0, 9.0, 6.0), h, o);
            p.add(egui::Shape::closed_line(rect_poly(-26.0, -14.0, 26.0, 14.0), Stroke::new(1.3, o)));
        }
        Pict::SideBody => {
            hatch_poly(p, &rect_poly(-26.0, -14.0, 0.0, 14.0), h, o);
            p.rect(Rect::from_min_max(at(-9.0, -6.0), at(0.0, 6.0)), 0.0, bg, Stroke::new(1.3, o), egui::StrokeKind::Middle);
            p.add(egui::Shape::closed_line(rect_poly(-26.0, -14.0, 26.0, 14.0), Stroke::new(1.3, o)));
        }
    }
}

/// Auswahlkarte mit Titel und Schaubild. `w` = Breite.
pub fn option_card(ui: &mut Ui, t: &Tok, label: &str, selected: bool, pict: Pict, w: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 78.0), Sense::click());
    let bg = if selected || resp.hovered() { t.card2 } else { t.card };
    let p = ui.painter();
    p.rect_filled(rect, 10.0, bg);
    p.rect_stroke(rect, 10.0, Stroke::new(if selected { 2.0 } else { 1.0 }, if selected { t.accent } else { t.line }), egui::StrokeKind::Inside);
    p.text(Pos2::new(rect.center().x, rect.top() + 15.0), egui::Align2::CENTER_CENTER, label, FontId::new(12.0, semi()), t.text);
    draw_pict(p, t, pict, Pos2::new(rect.center().x, rect.top() + 49.0), bg);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Zwei Auswahlkarten nebeneinander je Zeile. Rückgabe: Index der angeklickten Karte.
pub fn option_grid(ui: &mut Ui, t: &Tok, items: &[(&str, Pict)], selected: usize) -> Option<usize> {
    let gap = 8.0;
    let w = ((ui.available_width() - gap) / 2.0).floor();
    let mut clicked = None;
    for row in 0..items.len().div_ceil(2) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for i in row * 2..(row * 2 + 2).min(items.len()) {
                if option_card(ui, t, items[i].0, i == selected, items[i].1, w).clicked() {
                    clicked = Some(i);
                }
            }
        });
        ui.add_space(gap - ui.spacing().item_spacing.y);
    }
    clicked
}

/// Ablagefläche (gestrichelt). Rückgabe: angeklickt.
pub fn drop_zone(ui: &mut Ui, t: &Tok, hovering: bool, title: &str, sub: &str, h: f32) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::click());
    let active = hovering || resp.hovered();
    ui.painter().rect_filled(rect, 8.0, if active { t.accent_soft } else { Color32::TRANSPARENT });
    let col = if active { t.accent } else { t.line.gamma_multiply(1.6) };
    dashed_rect(ui.painter(), rect.shrink(1.0), 8.0, col);
    ui.painter().text(rect.center() - Vec2::new(0.0, 9.0), egui::Align2::CENTER_CENTER, title, FontId::new(14.0, semi()), t.text);
    ui.painter().text(rect.center() + Vec2::new(0.0, 11.0), egui::Align2::CENTER_CENTER, sub, FontId::proportional(12.0), t.muted);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}

fn dashed_rect(p: &egui::Painter, r: Rect, rad: f32, c: Color32) {
    // Umriss als Polygon mit gerundeten Ecken, gestrichelt gezeichnet
    let mut pts = Vec::new();
    let corners = [
        (Pos2::new(r.right() - rad, r.top() + rad), -90.0f32),
        (Pos2::new(r.right() - rad, r.bottom() - rad), 0.0),
        (Pos2::new(r.left() + rad, r.bottom() - rad), 90.0),
        (Pos2::new(r.left() + rad, r.top() + rad), 180.0),
    ];
    for (c0, a0) in corners {
        for k in 0..=6 {
            let a = (a0 + k as f32 * 15.0).to_radians();
            pts.push(c0 + Vec2::new(a.cos(), a.sin()) * rad);
        }
    }
    pts.push(pts[0]);
    p.extend(egui::Shape::dashed_line(&pts, Stroke::new(1.5, c), 6.0, 4.0));
}

// ---------------------------------------------------------------------------
// Symbole (gezeichnet)

pub enum Icon {
    Convert,
    Split,
    Vector,
    Repair,
    Prepare,
    Check,
    Texture,
    Settings,
    Plus,
    Minus,
    Reset,
}

pub fn icon(p: &egui::Painter, r: Rect, which: Icon, c: Color32) {
    let s = Stroke::new(1.8, c);
    let cx = r.center();
    let u = r.width().min(r.height()) / 24.0;
    let pt = |x: f32, y: f32| Pos2::new(cx.x + (x - 12.0) * u, cx.y + (y - 12.0) * u);
    match which {
        Icon::Convert => {
            p.add(egui::Shape::line(vec![pt(5.0, 3.0), pt(14.0, 3.0), pt(19.0, 8.0), pt(19.0, 21.0), pt(5.0, 21.0), pt(5.0, 3.0)], s));
            p.add(egui::Shape::line(vec![pt(14.0, 3.0), pt(14.0, 8.0), pt(19.0, 8.0)], s));
        }
        Icon::Split => {
            // Schere schneidet entlang der Strichlinie zwischen zwei Teilen
            let seg = |a: Pos2, b: Pos2| {
                p.line_segment([a, b], s);
                p.circle_filled(a, s.width * 0.5, c);
                p.circle_filled(b, s.width * 0.5, c);
            };
            let part = |y0: f32, y1: f32| {
                p.rect_stroke(Rect::from_min_max(pt(11.0, y0), pt(21.0, y1)), 1.6 * u, s, egui::StrokeKind::Middle);
            };
            part(3.5, 8.5);
            part(15.5, 20.5);
            seg(pt(15.0, 12.0), pt(16.6, 12.0));
            seg(pt(18.6, 12.0), pt(20.2, 12.0));
            p.circle_stroke(pt(4.8, 8.6), 2.3 * u, s);
            p.circle_stroke(pt(4.8, 15.4), 2.3 * u, s);
            seg(pt(6.7, 10.0), pt(12.4, 13.6));
            seg(pt(6.7, 14.0), pt(12.4, 10.4));
        }
        Icon::Vector => {
            let mut curve = Vec::new();
            for k in 0..=16 {
                let tt = k as f32 / 16.0;
                let m = 1.0 - tt;
                let x = m * m * m * 4.0 + 3.0 * m * m * tt * 6.0 + 3.0 * m * tt * tt * 18.0 + tt * tt * tt * 20.0;
                let y = m * m * m * 19.0 + 3.0 * m * m * tt * 7.0 + 3.0 * m * tt * tt * 7.0 + tt * tt * tt * 19.0;
                curve.push(pt(x, y));
            }
            p.add(egui::Shape::line(curve, s));
            p.rect_stroke(Rect::from_center_size(pt(4.0, 19.0), Vec2::splat(3.6 * u)), 0.5, s, egui::StrokeKind::Middle);
            p.rect_stroke(Rect::from_center_size(pt(20.0, 19.0), Vec2::splat(3.6 * u)), 0.5, s, egui::StrokeKind::Middle);
            p.circle_stroke(pt(8.0, 7.0), 1.7 * u, s);
            p.circle_stroke(pt(16.0, 7.0), 1.7 * u, s);
        }
        Icon::Repair => {
            // Pflaster: schräge Kapsel mit Polster in der Mitte
            let (dx, dy) = (std::f32::consts::FRAC_1_SQRT_2, -std::f32::consts::FRAC_1_SQRT_2);
            let (nx, ny) = (-dy, dx);
            let at = |a: f32, b: f32| Pos2::new(cx.x + (dx * a + nx * b) * u, cx.y + (dy * a + ny * b) * u);
            let (half, r) = (6.5, 4.0);
            let mut cap = Vec::new();
            for k in 0..=12 {
                let a = -std::f32::consts::FRAC_PI_2 + k as f32 / 12.0 * std::f32::consts::PI;
                cap.push(at(half + a.cos() * r, a.sin() * r));
            }
            for k in 0..=12 {
                let a = std::f32::consts::FRAC_PI_2 + k as f32 / 12.0 * std::f32::consts::PI;
                cap.push(at(-half + a.cos() * r, a.sin() * r));
            }
            cap.push(cap[0]);
            p.add(egui::Shape::line(cap, s));
            p.add(egui::Shape::line(vec![at(-2.6, -2.6), at(2.6, -2.6), at(2.6, 2.6), at(-2.6, 2.6), at(-2.6, -2.6)], s));
        }
        Icon::Texture => {
            // Fläche mit Rändelmuster
            p.rect_stroke(Rect::from_min_max(pt(4.0, 4.0), pt(20.0, 20.0)), 3.0 * u, s, egui::StrokeKind::Middle);
            for k in [-4.0f32, 0.0, 4.0] {
                p.line_segment([pt(8.0 + k, 4.5), pt(19.5, 16.0 - k)], Stroke::new(1.2, c));
                p.line_segment([pt(16.0 - k, 4.5), pt(4.5, 16.0 - k)], Stroke::new(1.2, c));
            }
        }
        Icon::Check => {
            // Lupe mit Haken
            p.circle_stroke(pt(10.5, 10.5), 6.5 * u, s);
            p.line_segment([pt(15.3, 15.3), pt(20.0, 20.0)], Stroke::new(2.4, c));
            p.add(egui::Shape::line(vec![pt(7.5, 10.8), pt(9.8, 13.0), pt(13.8, 8.4)], s));
        }
        Icon::Prepare => {
            // Druckkopf über dem Druckbett
            p.line_segment([pt(4.0, 20.0), pt(20.0, 20.0)], s);
            p.rect_stroke(Rect::from_min_max(pt(8.0, 4.0), pt(16.0, 9.0)), 1.0 * u, s, egui::StrokeKind::Middle);
            p.add(egui::Shape::line(vec![pt(10.0, 9.0), pt(12.0, 13.0), pt(14.0, 9.0)], s));
            p.line_segment([pt(7.0, 16.5), pt(17.0, 16.5)], s);
        }
        Icon::Settings => {
            p.circle_stroke(cx, 3.2 * u, s);
            let mut outer = Vec::new();
            for k in 0..=48 {
                let a = k as f32 / 48.0 * std::f32::consts::TAU;
                let tooth = ((a * 8.0 / std::f32::consts::TAU * 2.0).floor() as i32) % 2 == 0;
                let rr = if tooth { 9.0 } else { 7.2 } * u;
                outer.push(cx + Vec2::new(a.cos(), a.sin()) * rr);
            }
            p.add(egui::Shape::line(outer, s));
        }
        Icon::Plus => {
            p.line_segment([pt(6.0, 12.0), pt(18.0, 12.0)], s);
            p.line_segment([pt(12.0, 6.0), pt(12.0, 18.0)], s);
        }
        Icon::Minus => {
            p.line_segment([pt(6.0, 12.0), pt(18.0, 12.0)], s);
        }
        Icon::Reset => {
            let mut arc = Vec::new();
            for k in 0..=20 {
                let a = (40.0 + k as f32 * 14.0).to_radians();
                arc.push(cx + Vec2::new(a.cos(), a.sin()) * 7.0 * u);
            }
            let end = *arc.last().unwrap();
            p.add(egui::Shape::line(arc, s));
            p.line_segment([end, end + Vec2::new(0.0, -4.0 * u)], s);
            p.line_segment([end, end + Vec2::new(4.0 * u, 0.0)], s);
        }
    }
}

/// Runde Schaltfläche mit Symbol (für die Vorschau).
pub fn icon_button(ui: &mut Ui, t: &Tok, which: Icon, tip: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(34.0), Sense::click());
    let fill = if resp.hovered() { t.card2 } else { t.panel.gamma_multiply(0.92) };
    ui.painter().rect_filled(rect, 9.0, fill);
    ui.painter().rect_stroke(rect, 9.0, Stroke::new(1.0, t.line), egui::StrokeKind::Inside);
    icon(ui.painter(), rect.shrink(9.0), which, if resp.hovered() { t.text } else { t.muted });
    resp.on_hover_text(tip)
}

/// App-Logo (Würfel auf Verlauf).
/// App-Logo (Würfel: Netz, geteilte Fläche, Vektorkurve) aus der eingebetteten PNG.
pub fn logo(p: &egui::Painter, r: Rect) {
    let id = egui::Id::new("nz_logo_tex");
    let ctx = p.ctx().clone();
    let tex = ctx.data_mut(|d| d.get_temp::<egui::TextureHandle>(id)).or_else(|| {
        let img = image::load_from_memory(include_bytes!("../assets/logo.png")).ok()?.to_rgba8();
        let (w, h) = img.dimensions();
        let ci = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], img.as_raw());
        let t = ctx.load_texture("nz_logo", ci, egui::TextureOptions::LINEAR);
        ctx.data_mut(|d| d.insert_temp(id, t.clone()));
        Some(t)
    });
    match tex {
        Some(t) => {
            p.image(t.id(), r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        }
        None => {
            p.rect_filled(r, r.width() * 0.27, Color32::from_rgb(92, 112, 246));
        }
    }
}

pub fn c32(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// Deutscher Name für eine Farbe (nächstliegende Grundfarbe).
pub fn color_name(c: [u8; 3]) -> String {
    const NAMES: [([u8; 3], &str); 22] = [
        ([0, 0, 0], "Schwarz"),
        ([255, 255, 255], "Weiß"),
        ([128, 128, 128], "Grau"),
        ([192, 192, 192], "Hellgrau"),
        ([64, 64, 64], "Dunkelgrau"),
        ([220, 40, 40], "Rot"),
        ([128, 0, 0], "Dunkelrot"),
        ([255, 140, 0], "Orange"),
        ([255, 215, 0], "Gelb"),
        ([245, 230, 190], "Beige"),
        ([139, 90, 43], "Braun"),
        ([40, 160, 70], "Grün"),
        ([20, 90, 50], "Dunkelgrün"),
        ([150, 210, 80], "Hellgrün"),
        ([20, 184, 166], "Türkis"),
        ([56, 189, 248], "Hellblau"),
        ([40, 90, 210], "Blau"),
        ([20, 40, 110], "Dunkelblau"),
        ([139, 92, 246], "Lila"),
        ([236, 72, 153], "Pink"),
        ([255, 182, 193], "Rosa"),
        ([212, 175, 55], "Gold"),
    ];
    let d = |a: [u8; 3], b: [u8; 3]| {
        let (x, y, z) = (a[0] as f64 - b[0] as f64, a[1] as f64 - b[1] as f64, a[2] as f64 - b[2] as f64);
        2.0 * x * x + 4.0 * y * y + 3.0 * z * z
    };
    NAMES.iter().min_by(|a, b| d(a.0, c).partial_cmp(&d(b.0, c)).unwrap()).map(|x| x.1.to_string()).unwrap_or_default()
}
