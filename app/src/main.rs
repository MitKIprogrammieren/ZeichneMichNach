#![cfg_attr(windows, windows_subsystem = "windows")]
//! ZeichneMichNach – Fotos und Logos lokal in saubere Vektorgrafiken umwandeln.

mod dialog;
mod fonts;
mod ocr;
mod pipeline;
mod settings_page;
mod theme;
mod vectortab;

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use std::path::{Path, PathBuf};
use theme::{Icon, Settings, Tok};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Trace,
    Settings,
}

pub fn reveal(path: &Path) {
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg("-R").arg(path).spawn();
    }
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer").arg(format!("/select,{}", path.display())).spawn();
    }
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        if let Some(d) = path.parent() {
            let _ = std::process::Command::new("xdg-open").arg(d).spawn();
        }
    }
}

// ---------------------------------------------------------------------------
// gemeinsame Bausteine

pub fn side_frame(t: &Tok) -> egui::Frame {
    egui::Frame::new().fill(t.panel).stroke(Stroke::new(1.0, t.line))
}

pub fn right_frame(t: &Tok) -> egui::Frame {
    egui::Frame::new().fill(t.bg).stroke(Stroke::new(1.0, t.line))
}

pub fn foot_frame(t: &Tok) -> egui::Frame {
    egui::Frame::new().fill(t.panel).inner_margin(egui::Margin { left: 16, right: 16, top: 14, bottom: 14 }).stroke(Stroke::new(1.0, t.line))
}

/// Farben des Vorschau-Hintergrunds (Mitte, Rand).
pub fn preview_bg(t: &Tok, s: &Settings) -> (Color32, Color32) {
    match s.preview_bg {
        theme::PreviewBg::Gradient => {
            if t.dark {
                (Color32::from_rgb(50, 52, 57), Color32::from_rgb(34, 36, 40))
            } else {
                (Color32::from_rgb(244, 245, 247), Color32::from_rgb(222, 224, 228))
            }
        }
        theme::PreviewBg::Dark => (Color32::from_rgb(26, 29, 35), Color32::from_rgb(26, 29, 35)),
        theme::PreviewBg::Light => (Color32::from_rgb(242, 243, 246), Color32::from_rgb(242, 243, 246)),
    }
}

pub fn paint_bg(painter: &egui::Painter, rect: Rect, center: Color32, edge: Color32) {
    let n = 12usize;
    let mut m = egui::Mesh::default();
    let c = rect.center() - Vec2::new(0.0, rect.height() * 0.1);
    let maxd = (rect.width() * rect.width() + rect.height() * rect.height()).sqrt() * 0.55;
    for j in 0..=n {
        for i in 0..=n {
            let p = Pos2::new(rect.left() + rect.width() * i as f32 / n as f32, rect.top() + rect.height() * j as f32 / n as f32);
            let k = ((p - c).length() / maxd).clamp(0.0, 1.0);
            let k = k * k * (3.0 - 2.0 * k);
            let l = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * k) as u8;
            m.colored_vertex(p, Color32::from_rgb(l(center.r(), edge.r()), l(center.g(), edge.g()), l(center.b(), edge.b())));
        }
    }
    for j in 0..n as u32 {
        for i in 0..n as u32 {
            let a = j * (n as u32 + 1) + i;
            let b = a + 1;
            let cc = a + n as u32 + 1;
            let d = cc + 1;
            m.add_triangle(a, b, d);
            m.add_triangle(a, d, cc);
        }
    }
    painter.add(egui::Shape::mesh(m));
}

pub fn file_icon(ui: &mut egui::Ui, t: &Tok, tag: &str, size: f32) {
    let (r, _) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
    ui.painter().rect_filled(r, size * 0.26, t.accent_soft);
    ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, tag, egui::FontId::new(size * 0.3, theme::semi()), t.accent);
}

/// Schwebende Leiste (abgerundet, halbtransparent) – Inhalt wird zentriert.
pub fn floating_bar(ui: &mut egui::Ui, t: &Tok, add: impl FnOnce(&mut egui::Ui)) {
    let max = ui.max_rect();
    let frame = egui::Frame::new()
        .fill(if t.dark { Color32::from_rgba_unmultiplied(52, 53, 58, 240) } else { Color32::from_rgba_unmultiplied(255, 255, 255, 245) })
        .stroke(Stroke::new(1.0, t.line))
        .corner_radius(8.0)
        .inner_margin(egui::Margin::same(4));
    let id = ui.id().with("floating_w");
    let w_prev: f32 = ui.ctx().data(|d| d.get_temp(id)).unwrap_or(max.width());
    let x0 = (max.center().x - w_prev / 2.0).max(max.left() + 8.0);
    let area = Rect::from_min_size(Pos2::new(x0, max.top()), Vec2::new(w_prev.max(10.0), max.height()));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(area).layout(egui::Layout::left_to_right(egui::Align::Center)));
    let r = frame.show(&mut child, |ui| {
        ui.horizontal(|ui| add(ui));
    });
    let w = r.response.rect.width();
    if (w - w_prev).abs() > 0.5 {
        ui.ctx().data_mut(|d| d.insert_temp(id, w));
        ui.ctx().request_repaint();
    }
}

// ---------------------------------------------------------------------------

struct App {
    page: Page,
    settings: Settings,
    applied: Option<Settings>,
    vector: vectortab::VectorTab,
    shot: Option<(f64, PathBuf)>,
}

impl App {
    fn top_bar(&mut self, ctx: &egui::Context, t: &Tok) {
        let mac = cfg!(target_os = "macos");
        let h = if mac { 40.0 } else { 46.0 };
        egui::TopBottomPanel::top("top").exact_height(h).frame(egui::Frame::new().fill(t.top)).show(ctx, |ui| {
            let rect = ui.max_rect();
            let drag = ui.interact(rect, ui.id().with("drag"), Sense::click_and_drag());
            if drag.drag_started() {
                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            if drag.double_clicked() {
                let max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
            }
            let cy = rect.center().y;
            let left = if mac { 80.0 } else { 14.0 };
            let lr = Rect::from_center_size(Pos2::new(rect.left() + left + 11.0, cy), Vec2::splat(22.0));
            theme::logo(ui.painter(), lr);
            let on_dark = Color32::from_rgb(205, 208, 212);
            ui.painter().text(Pos2::new(lr.right() + 10.0, cy), egui::Align2::LEFT_CENTER, "ZeichneMichNach", egui::FontId::new(14.0, theme::semi()), Color32::WHITE);
            let tw = ui.fonts(|f| f.layout_no_wrap("ZeichneMichNach".into(), egui::FontId::new(14.0, theme::semi()), Color32::WHITE).size().x);
            // Öffnen
            let open_r = Rect::from_center_size(Pos2::new(lr.right() + 10.0 + tw + 26.0, cy), Vec2::splat(28.0));
            let open = ui.interact(open_r, ui.id().with("open"), Sense::click());
            if open.hovered() {
                ui.painter().rect_filled(open_r, 5.0, Color32::from_rgba_unmultiplied(255, 255, 255, 24));
            }
            {
                let p = ui.painter();
                let c = open_r.center();
                let st = Stroke::new(1.5, on_dark);
                let pts = vec![
                    c + Vec2::new(-8.0, -6.0),
                    c + Vec2::new(-3.0, -6.0),
                    c + Vec2::new(-1.0, -4.0),
                    c + Vec2::new(8.0, -4.0),
                    c + Vec2::new(8.0, 6.0),
                    c + Vec2::new(-8.0, 6.0),
                    c + Vec2::new(-8.0, -6.0),
                ];
                p.add(egui::Shape::line(pts, st));
            }
            if open.on_hover_text("Bild öffnen (⌘O)").clicked() {
                self.page = Page::Trace;
                self.vector.open_dialog(ctx);
            }
            // rechts: Einstellungen und Hauptaktion
            let gear = Rect::from_center_size(Pos2::new(rect.right() - 26.0, cy), Vec2::splat(30.0));
            let g = ui.interact(gear, ui.id().with("gear"), Sense::click());
            if g.hovered() || self.page == Page::Settings {
                ui.painter().rect_filled(gear, 5.0, Color32::from_rgba_unmultiplied(255, 255, 255, 26));
            }
            theme::icon(ui.painter(), gear.shrink(6.0), Icon::Settings, if self.page == Page::Settings { t.accent } else { on_dark });
            if g.on_hover_text("Einstellungen").on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                self.page = if self.page == Page::Settings { Page::Trace } else { Page::Settings };
            }
            if self.page == Page::Trace {
                if let Some(a) = theme::main_action(ctx) {
                    let font = egui::FontId::new(13.0, theme::semi());
                    let w = ui.fonts(|f| f.layout_no_wrap(a.label.clone(), font.clone(), Color32::WHITE).size().x) + 34.0;
                    let br = Rect::from_min_max(Pos2::new(gear.left() - 12.0 - w, cy - 14.0), Pos2::new(gear.left() - 12.0, cy + 14.0));
                    let resp = ui.interact(br, ui.id().with("main_action"), if a.enabled { Sense::click() } else { Sense::hover() });
                    let mut c = t.accent;
                    if !a.enabled {
                        c = theme::lerp_color(c, t.top, 0.55);
                    } else if resp.hovered() {
                        c = theme::lerp_color(c, Color32::WHITE, 0.10);
                    }
                    ui.painter().rect_filled(br, 6.0, c);
                    ui.painter().text(br.center(), egui::Align2::CENTER_CENTER, &a.label, font, if a.enabled { Color32::WHITE } else { Color32::from_rgb(200, 205, 210) });
                    if a.enabled && resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        theme::click_main_action(ctx);
                    }
                }
            } else {
                let br = Rect::from_min_max(Pos2::new(gear.left() - 12.0 - 90.0, cy - 14.0), Pos2::new(gear.left() - 12.0, cy + 14.0));
                let resp = ui.interact(br, ui.id().with("back"), Sense::click());
                ui.painter().rect_stroke(br, 6.0, Stroke::new(1.0, Color32::from_rgb(90, 95, 100)), egui::StrokeKind::Inside);
                ui.painter().text(br.center(), egui::Align2::CENTER_CENTER, "Zurück", egui::FontId::new(13.0, theme::semi()), on_dark);
                if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                    self.page = Page::Trace;
                }
            }
        });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.applied.as_ref() != Some(&self.settings) {
            theme::apply(ctx, &self.settings);
            if self.applied.is_some() {
                self.settings.save();
            }
            self.applied = Some(self.settings.clone());
        }
        // ⌘O öffnen, ⇧⌘D hell/dunkel, ⌘, Einstellungen
        let (open, flip, prefs) = ctx.input(|i| {
            (
                i.modifiers.command && i.key_pressed(egui::Key::O),
                i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::D),
                i.modifiers.command && i.key_pressed(egui::Key::Comma),
            )
        });
        if open {
            self.page = Page::Trace;
            self.vector.open_dialog(ctx);
        }
        if flip {
            let dark = ctx.style().visuals.dark_mode;
            self.settings.theme = if dark { theme::ThemeMode::Light } else { theme::ThemeMode::Dark };
        }
        if prefs {
            self.page = Page::Settings;
        }
        let dark = ctx.style().visuals.dark_mode;
        let t = theme::tok_for(dark, &self.settings);

        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        if let Some(f) = dropped.into_iter().find(|p| vectortab::is_image(p)) {
            self.page = Page::Trace;
            self.vector.load(f, ctx);
        }
        let hovering = ctx.input(|i| !i.raw.hovered_files.is_empty());

        self.top_bar(ctx, &t);
        match self.page {
            Page::Trace => self.vector.ui(ctx, &mut self.settings, hovering),
            Page::Settings => settings_page::ui(ctx, &mut self.settings),
        }

        // Bildschirmfoto für die automatische Prüfung
        if let Some((at, _)) = self.shot.clone() {
            if ctx.input(|i| i.time) >= at {
                self.shot = None;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            } else {
                ctx.request_repaint();
            }
        }
        let shots: Vec<std::sync::Arc<egui::ColorImage>> =
            ctx.input(|i| i.raw.events.iter().filter_map(|e| if let egui::Event::Screenshot { image, .. } = e { Some(image.clone()) } else { None }).collect());
        for img in shots {
            if let Ok(p) = std::env::var("NZ_SHOT") {
                let rgba: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
                let png = nz_core::vector::encode_png(img.size[0], img.size[1], &rgba);
                let _ = std::fs::write(&p, png);
                if std::env::var("NZ_SHOT_QUIT").is_ok() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut page = Page::Trace;
    let mut files = Vec::new();
    for a in &args {
        if a == "--einstellungen" {
            page = Page::Settings;
            continue;
        }
        let p = PathBuf::from(a);
        if p.exists() {
            files.push(p);
        }
    }
    let mut settings = Settings::load();
    match std::env::var("NZ_THEME").as_deref() {
        Ok("hell") => settings.theme = theme::ThemeMode::Light,
        Ok("dunkel") => settings.theme = theme::ThemeMode::Dark,
        _ => {}
    }
    let shot = std::env::var("NZ_SHOT").ok().map(|p| (std::env::var("NZ_SHOT_AT").ok().and_then(|v| v.parse().ok()).unwrap_or(6.0), PathBuf::from(p)));
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("ZeichneMichNach")
        .with_inner_size([1360.0, 860.0])
        .with_min_inner_size([1100.0, 640.0])
        .with_drag_and_drop(true);
    if let Ok(img) = image::load_from_memory(include_bytes!("../assets/icon.png")) {
        let img = img.to_rgba8();
        let (width, height) = img.dimensions();
        viewport = viewport.with_icon(std::sync::Arc::new(egui::IconData { rgba: img.into_raw(), width, height }));
    }
    if cfg!(target_os = "macos") {
        viewport = viewport.with_fullsize_content_view(true).with_titlebar_shown(false).with_title_shown(false);
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native(
        "ZeichneMichNach",
        options,
        Box::new(move |cc| {
            theme::setup_fonts(&cc.egui_ctx);
            let mut app = App { page, settings, applied: None, vector: vectortab::VectorTab::default(), shot };
            if let Some(f) = files.first() {
                app.vector.load(f.clone(), &cc.egui_ctx);
            } else if std::env::var("NZ_SAMPLE").is_ok() {
                app.vector.load_sample(&cc.egui_ctx);
            }
            Ok(Box::new(app))
        }),
    )
}
