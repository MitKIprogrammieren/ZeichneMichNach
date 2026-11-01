//! Datei-Dialoge ohne Blockieren der Ereignisschleife.
//!
//! Blockierende Dialoge (rfd::FileDialog) laufen auf dem Mac in einer eigenen modalen
//! Schleife. Kommt währenddessen ein Drag-and-drop-Ereignis an, ruft macOS die
//! Fensterbehandlung erneut auf, während sie noch beschäftigt ist – die App stürzt ab.
//! Deshalb hier: asynchrone Dialoge in einem Hilfsthread, das Ergebnis wird im nächsten
//! Bild abgeholt.

use eframe::egui;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::sync::Mutex;

struct Pending {
    tag: &'static str,
    rx: Receiver<Vec<PathBuf>>,
    done: Option<Vec<PathBuf>>,
}

static SLOT: Mutex<Option<Pending>> = Mutex::new(None);

/// Ein Dialog ist geöffnet (dann keinen zweiten öffnen).
#[allow(dead_code)]
pub fn busy() -> bool {
    SLOT.lock().map(|s| s.is_some()).unwrap_or(false)
}

/// Dialog öffnen; `f` läuft im Hilfsthread und liefert die gewählten Pfade (leer = abgebrochen).
pub fn open(tag: &'static str, ctx: &egui::Context, f: impl FnOnce() -> Vec<PathBuf> + Send + 'static) {
    let mut slot = match SLOT.lock() {
        Ok(s) => s,
        Err(_) => return,
    };
    if slot.is_some() {
        return;
    }
    let (tx, rx) = channel();
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let r = f();
        let _ = tx.send(r);
        ctx.request_repaint();
    });
    *slot = Some(Pending { tag, rx, done: None });
}

/// Ergebnis eines Dialogs mit diesem Kennzeichen abholen (einmalig).
/// `Some(leer)` = abgebrochen, `None` = (noch) nichts da.
pub fn take(tag: &'static str) -> Option<Vec<PathBuf>> {
    let mut slot = SLOT.lock().ok()?;
    let p = slot.as_mut()?;
    if p.done.is_none() {
        match p.rx.try_recv() {
            Ok(v) => p.done = Some(v),
            Err(std::sync::mpsc::TryRecvError::Empty) => return None,
            Err(_) => p.done = Some(Vec::new()),
        }
    }
    if p.tag != tag {
        return None;
    }
    let r = p.done.take();
    *slot = None;
    r
}

fn one(h: Option<rfd::FileHandle>) -> Vec<PathBuf> {
    h.map(|h| vec![h.path().to_path_buf()]).unwrap_or_default()
}

pub fn pick_file(tag: &'static str, ctx: &egui::Context, name: &'static str, exts: &'static [&'static str]) {
    open(tag, ctx, move || one(pollster::block_on(rfd::AsyncFileDialog::new().add_filter(name, exts).pick_file())));
}

pub fn pick_files(tag: &'static str, ctx: &egui::Context, name: &'static str, exts: &'static [&'static str]) {
    open(tag, ctx, move || {
        pollster::block_on(rfd::AsyncFileDialog::new().add_filter(name, exts).pick_files())
            .map(|v| v.into_iter().map(|h| h.path().to_path_buf()).collect())
            .unwrap_or_default()
    });
}

pub fn pick_folder(tag: &'static str, ctx: &egui::Context, dir: Option<PathBuf>) {
    open(tag, ctx, move || {
        let mut d = rfd::AsyncFileDialog::new();
        if let Some(dir) = dir {
            d = d.set_directory(dir);
        }
        one(pollster::block_on(d.pick_folder()))
    });
}

pub fn save_file(tag: &'static str, ctx: &egui::Context, filter: (String, Vec<String>), name: String, dir: Option<PathBuf>) {
    open(tag, ctx, move || {
        let exts: Vec<&str> = filter.1.iter().map(|s| s.as_str()).collect();
        let mut d = rfd::AsyncFileDialog::new().add_filter(filter.0.clone(), exts.as_slice()).set_file_name(name.clone());
        if let Some(dir) = dir {
            d = d.set_directory(dir);
        }
        one(pollster::block_on(d.save_file()))
    });
}
