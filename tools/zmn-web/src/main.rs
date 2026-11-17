//! zmn-web – ZeichneMichNach im Browser (für den Container auf dem NUC).
//!
//!   zmn-web [--port 8094] [--daten /daten]
//!
//! Gleicher Kern und gleicher Ablauf wie Mac-App und Kommandozeile. Texterkennung über
//! den Helfer `zmn-ocr` (Tesseract) neben dem Programm, Schriften aus /usr/share/fonts.
#[path = "../../../app/src/fonts.rs"]
mod fonts;
#[path = "../../../app/src/ocr.rs"]
mod ocr;
#[path = "../../../app/src/pipeline.rs"]
mod pipeline;

use nz_core::hand::HandOpts;
use nz_core::json::quote;
use nz_core::relief::Build3d;
use nz_core::vector::{self, Doc, Opts};
use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tiny_http::{Header, Method, Request, Response, Server};

const INDEX: &str = include_str!("index.html");
const ICON: &[u8] = include_bytes!("../../../packaging/icon.png");
/// höchstens so viele Ergebnisse im Speicher (für Speichern in anderen Formaten)
const KEEP: usize = 12;
const MAX_UPLOAD: usize = 40 * 1024 * 1024;

struct Session {
    doc: Doc,
    o: Opts,
    h: HandOpts,
    name: String,
    used: Instant,
}

type Store = Arc<Mutex<HashMap<String, Session>>>;

fn query(url: &str) -> HashMap<String, String> {
    let mut m = HashMap::new();
    if let Some((_, q)) = url.split_once('?') {
        for kv in q.split('&') {
            let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
            m.insert(unescape(k), unescape(v));
        }
    }
    m
}

fn unescape(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => {
                if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                    out.push(v);
                    i += 2;
                } else {
                    out.push(b'%');
                }
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn num(q: &HashMap<String, String>, k: &str, d: f64) -> f64 {
    q.get(k).and_then(|v| v.replace(',', ".").parse().ok()).unwrap_or(d)
}

fn flag(q: &HashMap<String, String>, k: &str, d: bool) -> bool {
    q.get(k).map(|v| v == "1" || v == "true" || v == "ja").unwrap_or(d)
}

fn header(k: &str, v: &str) -> Header {
    Header::from_bytes(k.as_bytes(), v.as_bytes()).unwrap()
}

fn json(req: Request, code: u16, body: String) {
    let _ = req.respond(Response::from_string(body).with_status_code(code).with_header(header("Content-Type", "application/json; charset=utf-8")));
}

fn fehler(req: Request, code: u16, msg: &str) {
    json(req, code, format!("{{\"fehler\":{}}}", quote(msg)));
}

/// Druckeinstellungen aus der Anfrage. Düse bestimmt Mindestbreite (2 × Düse)
/// und Eckenradius (halbe Düse), wenn nicht ausdrücklich anders angegeben.
fn hand_opts(q: &HashMap<String, String>) -> HandOpts {
    let mut h = HandOpts::default();
    h.bg_cut = flag(q, "freistellen", true);
    h.bg_inner = flag(q, "innen", false);
    h.bg_tol = num(q, "toleranz", h.bg_tol);
    h.kind = num(q, "bildart", 0.0).clamp(0.0, 2.0) as u8;
    h.simplify = num(q, "vereinfachen", 0.0).clamp(0.0, 2.0) as u8;
    h.text = flag(q, "text", true);
    h.lines = flag(q, "linien", true);
    h.photo = flag(q, "foto", true);
    h.gradients = flag(q, "verlauf", true);
    h.print = flag(q, "druck", false);
    let nozzle = num(q, "duese", 0.4).clamp(0.1, 1.2);
    h.print_width_mm = num(q, "breite", 80.0).clamp(5.0, 2000.0);
    h.print_min_mm = num(q, "min", 2.0 * nozzle).clamp(0.1, 10.0);
    h.print_round_mm = num(q, "ecken", nozzle / 2.0).clamp(0.0, 10.0);
    h.print_colors = num(q, "farben_druck", 4.0).clamp(1.0, 16.0) as usize;
    h.print_photo = num(q, "fotobereiche", 0.0).clamp(0.0, 2.0) as u8;
    if let Some(p) = q.get("palette") {
        h.print_palette = p
            .split(',')
            .filter_map(|c| {
                let v = u32::from_str_radix(c.trim().trim_start_matches('#'), 16).ok()?;
                Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
            })
            .collect();
    }
    h
}

fn decode(bytes: &[u8]) -> Result<(usize, usize, Vec<u8>), String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("Bild nicht lesbar: {e}"))?;
    // sehr große Fotos verkleinern – mehr Auflösung bringt beim Nachzeichnen nichts
    let img = if img.width().max(img.height()) > 3000 { img.resize(3000, 3000, image::imageops::FilterType::Lanczos3) } else { img };
    let rgba = img.to_rgba8();
    Ok((rgba.width() as usize, rgba.height() as usize, rgba.into_raw()))
}

fn id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    format!("{:x}", t ^ (std::process::id() as u128) << 64)
}

fn nachzeichnen(mut req: Request, store: &Store) {
    let q = query(req.url());
    let mut body = Vec::new();
    if req.as_reader().take(MAX_UPLOAD as u64 + 1).read_to_end(&mut body).is_err() || body.is_empty() {
        return fehler(req, 400, "Kein Bild empfangen");
    }
    if body.len() > MAX_UPLOAD {
        return fehler(req, 413, "Bild ist größer als 40 MB");
    }
    let (w, h, rgba) = match decode(&body) {
        Ok(x) => x,
        Err(e) => return fehler(req, 400, &e),
    };
    let ho = hand_opts(&q);
    let k = num(&q, "farben", 0.0).clamp(0.0, 32.0) as usize;
    let mut o = Opts::default();
    // Ausgabe wie bei vectorizer.ai: 0 = allgemein, 1 = gestapelt (nach Farbe), 2 = nur Konturen (Schneiden/Gravieren)
    match num(&q, "stil", 0.0) as u8 {
        1 => o.stack = true,
        2 => {
            o.stroke = true;
            o.stroke_only = true;
        }
        _ => {}
    }
    let mut doc = Doc::new(w, h, rgba);
    // Texterkennung braucht eine Datei
    let tmp = std::env::temp_dir().join(format!("zmn-web-{}.png", id()));
    let _ = std::fs::write(&tmp, vector::encode_png(w, h, &doc.src));
    let info = pipeline::run(&mut doc, Some(&tmp), &o, &ho, k, &|_, _| {});
    let _ = std::fs::remove_file(&tmp);
    let svg = doc.build_svg(&o, false);
    let (formen, knoten) = doc.stats();
    let farben: Vec<String> = doc.layers.iter().map(|l| quote(&vector::hex(doc.gcol(l.idx)))).collect();
    let texte: Vec<String> = info
        .matches
        .iter()
        .map(|m| format!("{{\"text\":{},\"schrift\":{},\"schnitt\":{},\"treffer\":{:.2}}}", quote(&m.text), quote(&m.family), quote(&m.styles.join("/")), m.score))
        .collect();
    let name = q.get("name").cloned().unwrap_or_else(|| "nachgezeichnet".into());
    let name = name.rsplit_once('.').map(|x| x.0.to_string()).unwrap_or(name);
    let sid = id();
    let out = format!(
        "{{\"id\":{},\"svg\":{},\"breite\":{w},\"hoehe\":{h},\"farben\":[{}],\"formen\":{formen},\"knoten\":{knoten},\"linien\":{},\"verlaeufe\":{},\"texte\":[{}],\"hinweis\":{},\"sekunden\":{:.2},\"freigestellt\":{},\"bildart\":{},\"ki\":{}}}",
        quote(&sid),
        quote(&svg),
        farben.join(","),
        doc.hand.lines.len(),
        doc.hand.grads.len(),
        texte.join(","),
        info.note.as_deref().map(quote).unwrap_or_else(|| "null".into()),
        info.secs,
        doc.data.chunks(4).any(|p| p[3] == 0),
        quote(if doc.hand.illu { "Illustration" } else if doc.hand.flat { "flache Grafik" } else { "Logo/Grafik" }),
        !doc.hand.kanten.is_empty()
    );
    {
        let mut s = store.lock().unwrap();
        if s.len() >= KEEP {
            if let Some(old) = s.iter().min_by_key(|e| e.1.used).map(|e| e.0.clone()) {
                s.remove(&old);
            }
        }
        s.insert(sid, Session { doc, o, h: ho, name, used: Instant::now() });
    }
    json(req, 200, out);
}

/// Schichthöhe passend zur Düse (halbe Düse) – Höhen werden darauf gerundet.
fn snap(v: f64, layer: f64) -> f64 {
    ((v / layer).round().max(1.0)) * layer
}

fn speichern(req: Request, store: &Store) {
    let q = query(req.url());
    let Some(sid) = q.get("id") else { return fehler(req, 400, "id fehlt") };
    let fmt = q.get("format").map(|s| s.to_lowercase()).unwrap_or_else(|| "svg".into());
    let mut s = store.lock().unwrap();
    let Some(sess) = s.get_mut(sid) else {
        drop(s);
        return fehler(req, 404, "Ergebnis nicht mehr im Speicher – bitte neu nachzeichnen");
    };
    sess.used = Instant::now();
    // Lückenfüllung (Kontur unter jeder Fläche): nur fürs Anschauen nötig, verdoppelt die
    // SVG – für 3D-Druck und Weiterbearbeitung abschaltbar
    let mut o_neu = sess.o.clone();
    if let Some(v) = q.get("luecke") {
        o_neu.gap = v == "1";
    }
    let (doc, o) = (&sess.doc, &o_neu);
    let nozzle = num(&q, "duese", 0.4).clamp(0.1, 1.2);
    let layer = (nozzle / 2.0).clamp(0.05, 0.4);
    let mut b3 = Build3d::default();
    b3.width_mm = num(&q, "breite", if sess.h.print { sess.h.print_width_mm } else { 80.0 });
    let base = num(&q, "grund", 1.6);
    b3.base = base > 0.0;
    b3.base_mm = snap(base.max(layer), layer);
    let rim = num(&q, "rand", 1.0);
    b3.rim = rim > 0.0;
    // Rand nie schmaler als druckbar (2 × Düse)
    b3.rim_w_mm = rim.max(2.0 * nozzle);
    b3.rim_h_mm = b3.base_mm;
    b3.uniform_mm = snap(num(&q, "hoehe", 0.6), layer);
    if let Some(c) = q.get("grundfarbe").and_then(|c| u32::from_str_radix(c.trim_start_matches('#'), 16).ok()) {
        b3.base_color = [(c >> 16) as u8, (c >> 8) as u8, c as u8];
    }
    if let Some(c) = q.get("randfarbe").and_then(|c| u32::from_str_radix(c.trim_start_matches('#'), 16).ok()) {
        b3.rim_color = [(c >> 16) as u8, (c >> 8) as u8, c as u8];
    }
    let name = sess.name.clone();
    let (data, mime): (Vec<u8>, &str) = match fmt.as_str() {
        "png" => (doc.build_png(o), "image/png"),
        "pdf" => (doc.build_pdf(o), "application/pdf"),
        "eps" => (doc.build_eps(o).into_bytes(), "application/postscript"),
        "dxf" => (doc.build_dxf(o).into_bytes(), "application/dxf"),
        "3mf" => {
            let (parts, _) = nz_core::relief::build_parts(doc, o, &b3);
            (nz_core::export3mf::write_3mf(&parts, &[name.clone()], &nz_core::export3mf::ExportOpts::default()), "model/3mf")
        }
        "step" | "stp" => {
            let bodies = nz_core::relief::bodies(doc, o, &b3);
            (nz_core::relief::step_from_bodies(&bodies, b3.width_mm, doc.src_w, doc.src_h, &name, &format!("{name}.step")).into_bytes(), "model/step")
        }
        _ => (doc.build_svg(o, false).into_bytes(), "image/svg+xml"),
    };
    drop(s);
    let ext = if fmt == "stp" { "step" } else { fmt.as_str() };
    let file = format!("{name}.{ext}");
    let ascii: String = file.chars().map(|c| if c.is_ascii_alphanumeric() || ".-_".contains(c) { c } else { '_' }).collect();
    let _ = req.respond(
        Response::from_data(data)
            .with_header(header("Content-Type", mime))
            .with_header(header("Content-Disposition", &format!("attachment; filename=\"{ascii}\"; filename*=UTF-8''{}", pct(&file)))),
    );
}

fn pct(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

struct Pruef {
    running: bool,
    started: Option<Instant>,
}

fn pruefstand_bin() -> Option<PathBuf> {
    let p = std::env::current_exe().ok()?.parent()?.join("zmn-pruefstand");
    p.exists().then_some(p)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut port = std::env::var("ZMN_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8094u16);
    let mut daten = PathBuf::from(std::env::var("ZMN_DATEN").unwrap_or_else(|_| "/daten".into()));
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--port" => {
                i += 1;
                port = args[i].parse().unwrap_or(port);
            }
            "--daten" => {
                i += 1;
                daten = PathBuf::from(&args[i]);
            }
            _ => {}
        }
        i += 1;
    }
    let _ = std::fs::create_dir_all(&daten);
    let server = Arc::new(Server::http(("0.0.0.0", port)).expect("Port belegt"));
    eprintln!("ZeichneMichNach läuft auf http://0.0.0.0:{port} (Texterkennung: {})", if ocr::available() { "ja" } else { "nein" });
    let store: Store = Arc::new(Mutex::new(HashMap::new()));
    let pruef = Arc::new(Mutex::new(Pruef { running: false, started: None }));
    // Nachzeichnen ist rechenintensiv: wenige Arbeiter, Rest wartet in der Schlange
    let workers = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2).clamp(2, 4);
    let mut hs = Vec::new();
    for _ in 0..workers {
        let (server, store, pruef, daten) = (server.clone(), store.clone(), pruef.clone(), daten.clone());
        hs.push(std::thread::spawn(move || {
            for req in server.incoming_requests() {
                let url = req.url().to_string();
                let path = url.split('?').next().unwrap_or("/").to_string();
                match (req.method(), path.as_str()) {
                    (Method::Get, "/") | (Method::Get, "/index.html") => {
                        let _ = req.respond(Response::from_string(INDEX).with_header(header("Content-Type", "text/html; charset=utf-8")));
                    }
                    (Method::Get, "/icon.png") => {
                        let _ = req.respond(Response::from_data(ICON).with_header(header("Content-Type", "image/png")).with_header(header("Cache-Control", "max-age=86400")));
                    }
                    (Method::Get, "/api/status") => {
                        let p = pruef.lock().unwrap();
                        let bericht = daten.join("pruefstand/bericht.html").exists();
                        let zus = std::fs::read_to_string(daten.join("pruefstand/zusammenfassung.txt")).unwrap_or_default();
                        let body = format!(
                            "{{\"version\":{},\"texterkennung\":{},\"pruefstand\":{},\"pruefstand_laeuft\":{},\"pruefstand_sekunden\":{},\"bericht\":{bericht},\"zusammenfassung\":{}}}",
                            quote(env!("CARGO_PKG_VERSION")),
                            ocr::available(),
                            pruefstand_bin().is_some(),
                            p.running,
                            p.started.map(|t| t.elapsed().as_secs()).unwrap_or(0),
                            quote(&zus)
                        );
                        drop(p);
                        json(req, 200, body);
                    }
                    (Method::Post, "/api/nachzeichnen") => nachzeichnen(req, &store),
                    (Method::Get, "/api/speichern") => speichern(req, &store),
                    (Method::Post, "/api/pruefstand") => {
                        let Some(bin) = pruefstand_bin() else { return_err(req, "Prüfstand ist in diesem Container nicht enthalten"); continue };
                        let mut p = pruef.lock().unwrap();
                        if p.running {
                            drop(p);
                            json(req, 200, "{\"laeuft\":true}".into());
                            continue;
                        }
                        p.running = true;
                        p.started = Some(Instant::now());
                        drop(p);
                        let anzahl = num(&query(&url), "anzahl", 2.0).clamp(1.0, 10.0) as u32;
                        let (pr, dir) = (pruef.clone(), daten.join("pruefstand"));
                        std::thread::spawn(move || {
                            let _ = std::process::Command::new(bin).arg(&dir).arg("--anzahl").arg(anzahl.to_string()).arg("--referenzen").arg(dir.join("referenzen")).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
                            pr.lock().unwrap().running = false;
                        });
                        json(req, 200, "{\"laeuft\":true}".into());
                    }
                    (Method::Post, "/api/referenz") => {
                        // eigene Referenzbilder in den Prüfstand legen: einzelne Datei (Name aus der
                        // Anfrage) oder ZIP mit beliebig vielen Bildern/SVGs (Namen aus dem Archiv)
                        let mut req = req;
                        let q = query(&url);
                        let mut body = Vec::new();
                        let _ = req.as_reader().take(400 * 1024 * 1024).read_to_end(&mut body);
                        if body.is_empty() {
                            fehler(req, 400, "Keine Datei empfangen");
                            continue;
                        }
                        let dir = daten.join("pruefstand/referenzen");
                        let _ = std::fs::create_dir_all(&dir);
                        let mut gespeichert: Vec<String> = Vec::new();
                        let mut ablegen = |name: &str, data: &[u8]| {
                            let datei = name.rsplit(['/', '\\']).next().unwrap_or(name);
                            if datei.starts_with('.') {
                                return;
                            }
                            let (stem, ext) = datei.rsplit_once('.').unwrap_or((datei, ""));
                            let ext = match ext.to_lowercase().as_str() {
                                "svg" => "svg",
                                "jpg" | "jpeg" => "jpg",
                                "png" => "png",
                                "webp" => "webp",
                                _ => return,
                            };
                            let stem: String = stem.chars().filter(|c| c.is_alphanumeric() || "-_ ".contains(*c)).collect::<String>().trim().replace(' ', "_");
                            if stem.is_empty() {
                                return;
                            }
                            let f = format!("{stem}.{ext}");
                            if std::fs::write(dir.join(&f), data).is_ok() {
                                gespeichert.push(f);
                            }
                        };
                        let name = q.get("name").cloned().unwrap_or_default();
                        if name.to_lowercase().ends_with(".zip") || body.starts_with(b"PK\x03\x04") {
                            match nz_core::zip::ZipArchive::read(&body) {
                                Ok(z) => {
                                    for (n, d) in &z.files {
                                        if !n.starts_with("__MACOSX") && !n.ends_with('/') {
                                            ablegen(n, d);
                                        }
                                    }
                                }
                                Err(e) => {
                                    fehler(req, 400, &e);
                                    continue;
                                }
                            }
                        } else {
                            ablegen(&name, &body);
                        }
                        drop(ablegen);
                        if gespeichert.is_empty() {
                            fehler(req, 400, "Keine Bilder oder SVGs gefunden (PNG, JPG, WebP, SVG)");
                        } else {
                            json(req, 200, format!("{{\"ok\":true,\"dateien\":[{}]}}", gespeichert.iter().map(|f| quote(f)).collect::<Vec<_>>().join(",")));
                        }
                    }
                    (Method::Delete, "/api/referenz") => {
                        let q = query(&url);
                        let dir = daten.join("pruefstand/referenzen");
                        let mut n = 0;
                        if q.get("alle").is_some() {
                            if let Ok(rd) = std::fs::read_dir(&dir) {
                                for e in rd.flatten() {
                                    if std::fs::remove_file(e.path()).is_ok() {
                                        n += 1;
                                    }
                                }
                            }
                        } else if let Some(d) = q.get("datei") {
                            // nur Dateinamen im Referenzordner, keine Pfade
                            if !d.contains('/') && !d.contains('\\') && !d.starts_with('.') && std::fs::remove_file(dir.join(d)).is_ok() {
                                n = 1;
                            }
                        }
                        json(req, 200, format!("{{\"geloescht\":{n}}}"));
                    }
                    (Method::Get, "/api/referenzen") => {
                        let mut names: Vec<String> = std::fs::read_dir(daten.join("pruefstand/referenzen")).map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect()).unwrap_or_default();
                        names.sort();
                        json(req, 200, format!("[{}]", names.iter().map(|n| quote(n)).collect::<Vec<_>>().join(",")));
                    }
                    (Method::Get, "/pruefstand") => match std::fs::read(daten.join("pruefstand/bericht.html")) {
                        Ok(d) => {
                            let _ = req.respond(Response::from_data(d).with_header(header("Content-Type", "text/html; charset=utf-8")));
                        }
                        Err(_) => {
                            let _ = req.respond(Response::from_string("Noch kein Prüfstand-Bericht – in den Einstellungen starten.").with_status_code(404));
                        }
                    },
                    _ => {
                        let _ = req.respond(Response::from_string("Nicht gefunden").with_status_code(404));
                    }
                }
            }
        }));
    }
    for h in hs {
        let _ = h.join();
    }
}

fn return_err(req: Request, msg: &str) {
    fehler(req, 404, msg);
}
