//! Einstellungsseite: Erscheinungsbild und Vorschau.

use crate::theme::{self, PreviewBg, Settings, ThemeMode, Tok, ACCENTS};
use eframe::egui::{self, RichText, Sense, Stroke, Vec2};

fn section(ui: &mut egui::Ui, t: &Tok, title: &str, sub: &str, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new().fill(t.panel).stroke(Stroke::new(1.0, t.line)).corner_radius(14.0).inner_margin(18.0).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(theme::strong(title, 14.5).color(t.text));
        ui.label(RichText::new(sub).size(12.0).color(t.faint));
        ui.add_space(10.0);
        add(ui);
    });
}

fn line(ui: &mut egui::Ui, t: &Tok, label: &str, right: impl FnOnce(&mut egui::Ui)) {
    let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter().line_segment([r.left_center(), r.right_center()], Stroke::new(1.0, t.line));
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.set_min_height(34.0);
        ui.label(RichText::new(label).color(t.text));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), right);
    });
    ui.add_space(2.0);
}

pub fn ui(ctx: &egui::Context, s: &mut Settings) {
    let t = theme::tok_for(ctx.style().visuals.dark_mode, s);
    egui::CentralPanel::default().frame(egui::Frame::new().fill(t.bg).inner_margin(egui::Margin { left: 40, right: 40, top: 28, bottom: 20 })).show(ctx, |ui| {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.label(theme::strong("Einstellungen", 22.0).color(t.text));
            ui.label(RichText::new("Aussehen von ZeichneMichNach – wird automatisch gespeichert").color(t.muted));
            ui.add_space(18.0);
            ui.columns(2, |cols| {
                section(&mut cols[0], &t, "Erscheinungsbild", "Hell, dunkel oder wie das System (⇧⌘D schaltet um)", |ui| {
                    line(ui, &t, "Design", |ui| {
                        theme::segmented(ui, &t, &mut s.theme, &[(ThemeMode::Light, "Hell"), (ThemeMode::Dark, "Dunkel"), (ThemeMode::System, "System")], false);
                    });
                    line(ui, &t, "Akzentfarbe", |ui| {
                        for c in ACCENTS.iter().rev() {
                            let (r, resp) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::click());
                            let sel = s.accent == *c;
                            if sel {
                                ui.painter().circle_stroke(r.center(), 14.5, Stroke::new(2.0, t.text));
                            }
                            ui.painter().circle_filled(r.center(), if resp.hovered() || sel { 11.5 } else { 10.5 }, theme::c32(*c));
                            if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                                s.accent = *c;
                            }
                        }
                    });
                    line(ui, &t, "Textgröße", |ui| {
                        theme::segmented(ui, &t, &mut s.text_size, &[(0u8, "Klein"), (1u8, "Normal"), (2u8, "Groß")], false);
                    });
                });
                section(&mut cols[1], &t, "Vorschau", "Hintergrund hinter dem Bild", |ui| {
                    line(ui, &t, "Hintergrund", |ui| {
                        theme::segmented(ui, &t, &mut s.preview_bg, &[(PreviewBg::Gradient, "Verlauf"), (PreviewBg::Dark, "Dunkel"), (PreviewBg::Light, "Hell")], false);
                    });
                });
            });
            ui.add_space(14.0);
            ui.label(RichText::new(format!("ZeichneMichNach {} · arbeitet komplett lokal, ohne Internet", env!("CARGO_PKG_VERSION"))).size(12.0).color(t.faint));
        });
    });
}
