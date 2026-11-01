//! Externe Engines: stl2step (mitgeliefert), breptile (eigene Python-Umgebung über uv)
//! und FreeCAD (eigene Kopie oder vorhandene Installation).
//!
//! Erkennen, Installieren, Aktualisieren und Umwandeln. Netzwerkzugriffe laufen über die
//! Systemprogramme curl, tar, hdiutil und codesign (SHA-256 eingebaut) – der Kern bleibt ohne Abhängigkeiten.

use crate::json::{self, Json};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EngineId {
    /// eigene Engine (immer vorhanden)
    Mesh2Step,
    Stl2Step,
    Breptile,
    FreeCad,
}

impl EngineId {
    pub const ALL: [EngineId; 4] = [EngineId::Mesh2Step, EngineId::Stl2Step, EngineId::Breptile, EngineId::FreeCad];
    pub fn name(self) -> &'static str {
        match self {
            EngineId::Mesh2Step => "Mesh2STEP",
            EngineId::Stl2Step => "stl2step",
            EngineId::Breptile => "breptile",
            EngineId::FreeCad => "FreeCAD",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            EngineId::Mesh2Step => "mesh2step",
            EngineId::Stl2Step => "stl2step",
            EngineId::Breptile => "breptile",
            EngineId::FreeCad => "freecad",
        }
    }
    pub fn from_key(s: &str) -> EngineId {
        match s {
            "stl2step" => EngineId::Stl2Step,
            "breptile" => EngineId::Breptile,
            "freecad" => EngineId::FreeCad,
            _ => EngineId::Mesh2Step,
        }
    }
    /// Kurzbeschreibung für die Auswahl
    pub fn blurb(self) -> &'static str {
        match self {
            EngineId::Mesh2Step => "Alle Flächenarten, auch Freiform",
            EngineId::Stl2Step => "Genaue Zylinder, Rundungen und Fasen",
            EngineId::Breptile => "Ebenen, Zylinder und Kugeln (Python)",
            EngineId::FreeCad => "Robust bei kaputten Netzen",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    BuiltIn,
    /// in der App mitgeliefert
    Bundled,
    /// von Mesh2STEP im Benutzerordner installiert
    UserDir,
    /// separat auf dem Rechner installiert
    System,
    Missing,
}

#[derive(Clone, Debug)]
pub struct EngineInfo {
    pub id: EngineId,
    pub available: bool,
    pub version: Option<String>,
    pub exe: Option<PathBuf>,
    pub source: Source,
    /// Grund, warum die Engine nicht nutzbar ist (z. B. „nur Apple Silicon“)
    pub note: Option<String>,
}

impl EngineInfo {
    pub fn source_de(&self) -> &'static str {
        match self.source {
            Source::BuiltIn => "eingebaut",
            Source::Bundled => "mitgeliefert",
            Source::UserDir => "installiert",
            Source::System => "auf dem Mac gefunden",
            Source::Missing => "nicht installiert",
        }
    }
}

// ---------------------------------------------------------------------------
// Ordner

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

/// Ordner für von Mesh2STEP installierte Engines.
pub fn data_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("M2S_ENGINES_DIR") {
        return PathBuf::from(d);
    }
    if cfg!(target_os = "macos") {
        home().join("Library/Application Support/Mesh2STEP/engines")
    } else if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(home).join("Mesh2STEP/engines")
    } else {
        home().join(".local/share/mesh2step/engines")
    }
}

/// In der App mitgelieferte Engines (Mesh2STEP.app/Contents/Resources/engines).
pub fn bundled_dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("M2S_BUNDLED_ENGINES") {
        return Some(PathBuf::from(d));
    }
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    for c in [dir.join("../Resources/engines"), dir.join("engines")] {
        if c.is_dir() {
            return Some(c);
        }
    }
    None
}

fn is_exec(p: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        p.is_file() && std::fs::metadata(p).map(|m| m.permissions().mode() & 0o111 != 0).unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        p.is_file()
    }
}

/// Ausführbare Datei mit diesem Namen unterhalb von `dir` suchen (höchstens 4 Ebenen tief).
fn find_exe(dir: &Path, name: &str) -> Option<PathBuf> {
    fn walk(d: &Path, name: &str, depth: u32) -> Option<PathBuf> {
        let rd = std::fs::read_dir(d).ok()?;
        let mut subs = Vec::new();
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                subs.push(p);
            } else if p.file_name().map(|f| f == name || f == format!("{name}.exe").as_str()).unwrap_or(false) && is_exec(&p) {
                return Some(p);
            }
        }
        if depth == 0 {
            return None;
        }
        subs.sort();
        subs.iter().find_map(|s| walk(s, name, depth - 1))
    }
    walk(dir, name, 4)
}

pub fn version_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let p = |s: &str| -> Vec<u64> { s.trim_start_matches(['v', 'V']).split(|c: char| !c.is_ascii_digit()).filter(|x| !x.is_empty()).map(|x| x.parse().unwrap_or(0)).collect() };
    p(a).cmp(&p(b))
}

fn read_version(dir: &Path) -> Option<String> {
    std::fs::read_to_string(dir.join("VERSION")).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn macos_version() -> Option<String> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let o = Command::new("sw_vers").arg("-productVersion").output().ok()?;
    Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn apple_silicon() -> bool {
    cfg!(all(target_os = "macos", target_arch = "aarch64"))
}

// ---------------------------------------------------------------------------
// Erkennen

pub fn detect(id: EngineId) -> EngineInfo {
    match id {
        EngineId::Mesh2Step => EngineInfo { id, available: true, version: Some(env!("CARGO_PKG_VERSION").to_string()), exe: None, source: Source::BuiltIn, note: None },
        EngineId::Stl2Step => detect_stl2step(),
        EngineId::Breptile => detect_breptile(),
        EngineId::FreeCad => detect_freecad(),
    }
}

pub fn detect_all() -> Vec<EngineInfo> {
    EngineId::ALL.iter().map(|&i| detect(i)).collect()
}

fn missing(id: EngineId, note: Option<String>) -> EngineInfo {
    EngineInfo { id, available: false, version: None, exe: None, source: Source::Missing, note }
}

fn detect_stl2step() -> EngineInfo {
    if let Some(p) = std::env::var_os("M2S_STL2STEP") {
        let p = PathBuf::from(p);
        return EngineInfo { id: EngineId::Stl2Step, available: is_exec(&p), version: None, exe: Some(p), source: Source::System, note: None };
    }
    if cfg!(target_os = "macos") && !apple_silicon() {
        return missing(EngineId::Stl2Step, Some("Das mitgelieferte stl2step läuft nur auf Apple Silicon.".into()));
    }
    let mut cands: Vec<(String, PathBuf, Source)> = Vec::new();
    // im Benutzerordner installierte Versionen
    if let Ok(rd) = std::fs::read_dir(data_dir().join("stl2step")) {
        for e in rd.flatten() {
            let d = e.path();
            if d.is_dir() && !d.to_string_lossy().ends_with(".tmp") {
                if let (Some(v), Some(x)) = (read_version(&d), find_exe(&d, "stl2step")) {
                    cands.push((v, x, Source::UserDir));
                }
            }
        }
    }
    if let Some(b) = bundled_dir() {
        let d = b.join("stl2step");
        if let Some(x) = find_exe(&d, "stl2step") {
            cands.push((read_version(&d).unwrap_or_else(|| "?".into()), x, Source::Bundled));
        }
    }
    cands.sort_by(|a, b| version_cmp(&b.0, &a.0));
    match cands.into_iter().next() {
        Some((v, x, s)) => {
            let note = match macos_version() {
                Some(mv) if version_cmp(&mv, "26.0") == std::cmp::Ordering::Less => Some(format!("stl2step braucht macOS 26 (dieser Mac: {mv}).")),
                _ => None,
            };
            EngineInfo { id: EngineId::Stl2Step, available: note.is_none(), version: Some(v), exe: Some(x), source: s, note }
        }
        None => missing(EngineId::Stl2Step, None),
    }
}

fn breptile_dir() -> PathBuf {
    data_dir().join("breptile")
}

fn detect_breptile() -> EngineInfo {
    let own = venv_bin(&breptile_dir().join("venv"), "breptile");
    if is_exec(&own) {
        return EngineInfo { id: EngineId::Breptile, available: true, version: read_version(&breptile_dir()), exe: Some(own), source: Source::UserDir, note: None };
    }
    let h = home();
    let mut cands = vec![h.join(".local/bin/breptile"), PathBuf::from("/opt/homebrew/bin/breptile"), PathBuf::from("/usr/local/bin/breptile")];
    if let Ok(rd) = std::fs::read_dir(h.join("Library/Python")) {
        for e in rd.flatten() {
            cands.push(e.path().join("bin/breptile"));
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        for d in std::env::split_paths(&path) {
            cands.push(d.join("breptile"));
        }
    }
    match cands.into_iter().find(|p| is_exec(p)) {
        Some(x) => EngineInfo { id: EngineId::Breptile, available: true, version: None, exe: Some(x), source: Source::System, note: None },
        None => missing(EngineId::Breptile, None),
    }
}

fn freecad_dir() -> PathBuf {
    data_dir().join("freecad")
}

/// freecadcmd in einer FreeCAD.app (verschiedene Versionen legen es unterschiedlich ab)
fn freecad_cmd_in(app: &Path) -> Option<PathBuf> {
    ["Contents/Resources/bin/freecadcmd", "Contents/Resources/bin/FreeCADCmd", "Contents/MacOS/FreeCADCmd", "Contents/MacOS/freecadcmd"]
        .iter()
        .map(|r| app.join(r))
        .find(|p| is_exec(p))
}

fn plist_version(app: &Path) -> Option<String> {
    let t = std::fs::read_to_string(app.join("Contents/Info.plist")).ok()?;
    let k = t.find("CFBundleShortVersionString")?;
    let rest = &t[k..];
    let a = rest.find("<string>")? + 8;
    let b = rest[a..].find("</string>")?;
    Some(rest[a..a + b].trim().to_string())
}

fn detect_freecad() -> EngineInfo {
    if let Some(p) = std::env::var_os("M2S_FREECADCMD") {
        let p = PathBuf::from(p);
        return EngineInfo { id: EngineId::FreeCad, available: is_exec(&p), version: None, exe: Some(p), source: Source::System, note: None };
    }
    let own = freecad_dir().join("FreeCAD.app");
    if let Some(x) = freecad_cmd_in(&own) {
        let v = read_version(&freecad_dir()).or_else(|| plist_version(&own));
        return EngineInfo { id: EngineId::FreeCad, available: true, version: v, exe: Some(x), source: Source::UserDir, note: None };
    }
    for app in [PathBuf::from("/Applications/FreeCAD.app"), home().join("Applications/FreeCAD.app")] {
        if let Some(x) = freecad_cmd_in(&app) {
            return EngineInfo { id: EngineId::FreeCad, available: true, version: plist_version(&app), exe: Some(x), source: Source::System, note: None };
        }
    }
    for p in ["/usr/bin/freecadcmd", "/usr/local/bin/freecadcmd", "/usr/bin/FreeCADCmd"] {
        let p = PathBuf::from(p);
        if is_exec(&p) {
            return EngineInfo { id: EngineId::FreeCad, available: true, version: None, exe: Some(p), source: Source::System, note: None };
        }
    }
    missing(EngineId::FreeCad, None)
}

// ---------------------------------------------------------------------------
// Prozesse

pub struct RunOut {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// Programm ausführen; bricht ab, wenn `cancel` gesetzt wird oder die Zeit abläuft.
pub fn run(exe: &Path, args: &[&str], envs: &[(&str, String)], cwd: Option<&Path>, cancel: Option<&AtomicBool>, timeout: Duration) -> Result<RunOut, String> {
    let mut cmd = Command::new(exe);
    no_window(&mut cmd);
    cmd.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    for (k, v) in envs {
        cmd.env(k, v);
    }
    if let Some(d) = cwd {
        cmd.current_dir(d);
    }
    let mut child = cmd.spawn().map_err(|e| format!("{} konnte nicht gestartet werden: {e}", exe.display()))?;
    let mut so = child.stdout.take().unwrap();
    let mut se = child.stderr.take().unwrap();
    let t_out = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = so.read_to_string(&mut s);
        s
    });
    let t_err = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = se.read_to_string(&mut s);
        s
    });
    let start = Instant::now();
    let status = loop {
        if let Some(st) = child.try_wait().map_err(|e| e.to_string())? {
            break Some(st);
        }
        if cancel.map(|c| c.load(Ordering::Relaxed)).unwrap_or(false) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Abgebrochen".into());
        }
        if start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("{} hat zu lange gebraucht und wurde beendet.", exe.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default()));
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    Ok(RunOut { code: status.and_then(|s| s.code()), stdout: t_out.join().unwrap_or_default(), stderr: t_err.join().unwrap_or_default() })
}

/// Unter Windows kein Konsolenfenster öffnen (sonst blitzt bei jedem curl/tar/
/// Engine-Aufruf ein schwarzes Fenster auf).
pub fn no_window(cmd: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

fn sys(name: &str) -> PathBuf {
    // Windows: die mitgelieferten curl.exe/tar.exe aus System32 (nicht z. B. GNU-tar aus
    // Git, das keine .zip und keine „C:\…“-Pfade versteht)
    if cfg!(windows) {
        let root = std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
        let p = root.join("System32").join(format!("{name}.exe"));
        if p.exists() {
            return p;
        }
    }
    for d in ["/usr/bin", "/bin", "/usr/sbin", "/sbin", "/opt/homebrew/bin", "/usr/local/bin"] {
        let p = Path::new(d).join(name);
        if p.exists() {
            return p;
        }
    }
    PathBuf::from(name)
}

fn http_get(url: &str) -> Result<String, String> {
    let o = run(&sys("curl"), &["-fsSL", "--max-time", "60", "-H", "User-Agent: Mesh2STEP", url], &[], None, None, Duration::from_secs(90))?;
    if o.code != Some(0) {
        return Err(format!("Keine Verbindung zu {} ({})", host_of(url), o.stderr.trim()));
    }
    Ok(o.stdout)
}

fn host_of(url: &str) -> String {
    url.split("//").nth(1).and_then(|r| r.split('/').next()).unwrap_or(url).to_string()
}

/// Datei herunterladen; `progress(0..1)` anhand der Dateigröße.
fn download(url: &str, dest: &Path, size: Option<u64>, progress: &dyn Fn(f32, &str), cancel: Option<&AtomicBool>) -> Result<(), String> {
    let _ = std::fs::remove_file(dest);
    let mut child = no_window(&mut Command::new(sys("curl")))
        .args(["-fsSL", "-H", "User-Agent: Mesh2STEP", "-o"])
        .arg(dest)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("curl konnte nicht gestartet werden: {e}"))?;
    loop {
        if let Some(st) = child.try_wait().map_err(|e| e.to_string())? {
            if !st.success() {
                let mut e = String::new();
                if let Some(mut s) = child.stderr.take() {
                    let _ = s.read_to_string(&mut e);
                }
                return Err(format!("Herunterladen fehlgeschlagen ({})", e.trim()));
            }
            break;
        }
        if cancel.map(|c| c.load(Ordering::Relaxed)).unwrap_or(false) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_file(dest);
            return Err("Abgebrochen".into());
        }
        let got = std::fs::metadata(dest).map(|m| m.len()).unwrap_or(0);
        match size {
            Some(s) if s > 0 => progress((got as f32 / s as f32).min(1.0) * 0.8, &format!("Herunterladen {} / {} MB", got / 1_000_000, s / 1_000_000)),
            _ => progress(0.3, &format!("Herunterladen {} MB", got / 1_000_000)),
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    Ok(())
}

/// SHA-256 einer Datei – eingebaut, damit es auch unter Windows (ohne `shasum`) geht.
fn sha256(p: &Path) -> Result<String, String> {
    use std::io::Read;
    let mut f = std::fs::File::open(p).map_err(|e| format!("{} nicht lesbar: {e}", p.display()))?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut buf).map_err(|e| format!("{} nicht lesbar: {e}", p.display()))?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.hex())
}

struct Sha256 {
    st: [u32; 8],
    buf: Vec<u8>,
    len: u64,
}

impl Sha256 {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];

    fn new() -> Self {
        Sha256 { st: [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19], buf: Vec::with_capacity(64), len: 0 }
    }

    fn block(&mut self, b: &[u8]) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([b[4 * i], b[4 * i + 1], b[4 * i + 2], b[4 * i + 3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let [mut a, mut b_, mut c, mut d, mut e, mut f, mut g, mut h] = self.st;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = h.wrapping_add(s1).wrapping_add(ch).wrapping_add(Self::K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b_) ^ (a & c) ^ (b_ & c);
            let t2 = s0.wrapping_add(maj);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b_;
            b_ = a;
            a = t1.wrapping_add(t2);
        }
        for (s, v) in self.st.iter_mut().zip([a, b_, c, d, e, f, g, h]) {
            *s = s.wrapping_add(v);
        }
    }

    fn update(&mut self, mut data: &[u8]) {
        self.len += data.len() as u64;
        if !self.buf.is_empty() {
            let take = (64 - self.buf.len()).min(data.len());
            self.buf.extend_from_slice(&data[..take]);
            data = &data[take..];
            if self.buf.len() == 64 {
                let b = std::mem::take(&mut self.buf);
                self.block(&b);
            }
        }
        while data.len() >= 64 {
            self.block(&data[..64]);
            data = &data[64..];
        }
        self.buf.extend_from_slice(data);
    }

    fn hex(mut self) -> String {
        let bits = self.len.wrapping_mul(8);
        let mut pad = vec![0x80u8];
        while (self.buf.len() + pad.len()) % 64 != 56 {
            pad.push(0);
        }
        pad.extend_from_slice(&bits.to_be_bytes());
        let len = self.len;
        self.update(&pad);
        self.len = len;
        self.st.iter().map(|x| format!("{x:08x}")).collect()
    }
}

#[cfg(test)]
mod sha_tests {
    use super::Sha256;

    fn hash(b: &[u8]) -> String {
        let mut h = Sha256::new();
        h.update(b);
        h.hex()
    }

    #[test]
    fn sha256_known() {
        assert_eq!(hash(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(hash(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        let big = vec![b'a'; 1_000_000];
        assert_eq!(hash(&big), "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0");
        // in Stücken gleich wie am Stück
        let mut h = Sha256::new();
        for c in big.chunks(777) {
            h.update(c);
        }
        assert_eq!(h.hex(), hash(&big));
    }
}

fn tmp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("mesh2step-{tag}-{}-{}", std::process::id(), Instant::now().elapsed().as_nanos() ^ (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0))));
    let _ = std::fs::create_dir_all(&d);
    d
}

// ---------------------------------------------------------------------------
// Updates

#[derive(Clone, Debug)]
pub struct Update {
    pub id: EngineId,
    pub current: Option<String>,
    pub latest: String,
    pub url: String,
    pub size: Option<u64>,
    pub sha256: Option<String>,
    pub sha_url: Option<String>,
    pub min_os: Option<String>,
    pub team_id: Option<String>,
}

impl Update {
    pub fn is_newer(&self) -> bool {
        match &self.current {
            None => true,
            Some(c) => version_cmp(&self.latest, c) == std::cmp::Ordering::Greater,
        }
    }
}

/// Neueste verfügbare Version abfragen (auch wenn noch nichts installiert ist).
pub fn latest(id: EngineId) -> Result<Update, String> {
    let cur = detect(id);
    // Bei vorhandenen System-Installationen zählt für das Update die eigene Kopie
    let current = match cur.source {
        Source::UserDir | Source::Bundled => cur.version.clone(),
        _ => None,
    };
    match id {
        EngineId::Mesh2Step => Err("Mesh2STEP wird mit der App aktualisiert.".into()),
        EngineId::Stl2Step => {
            let rel = json::parse(&http_get("https://api.github.com/repos/BlinkingSun/stl2step/releases/latest")?)?;
            let asset_url = |name: &str| rel.get("assets").map(|a| a.arr()).unwrap_or(&[]).iter().find(|x| x.get("name").and_then(|n| n.str()) == Some(name)).and_then(|x| x.get("browser_download_url")).and_then(|u| u.str()).map(|s| s.to_string());
            let man_url = asset_url("engine-manifest.json").ok_or("Diese stl2step-Version liefert kein Engine-Paket.")?;
            let man = json::parse(&http_get(&man_url)?)?;
            let key = if cfg!(target_os = "windows") { "windows-x64" } else { "macos-arm64" };
            let a = man.path(&format!("assets/{key}")).ok_or("Kein stl2step-Paket für dieses System.")?;
            let name = a.get("name").and_then(|x| x.str()).ok_or("Engine-Paket ohne Namen")?;
            Ok(Update {
                id,
                current,
                latest: man.get("engineVersion").and_then(|x| x.str()).unwrap_or("?").to_string(),
                url: asset_url(name).ok_or("Engine-Paket fehlt in der Veröffentlichung")?,
                size: a.get("size").and_then(|x| x.num()).map(|x| x as u64),
                sha256: a.get("sha256").and_then(|x| x.str()).map(|s| s.to_lowercase()),
                sha_url: None,
                min_os: a.get("minOS").and_then(|x| x.str()).map(|s| s.to_string()),
                team_id: a.get("teamId").and_then(|x| x.str()).map(|s| s.to_string()),
            })
        }
        EngineId::Breptile => {
            let j = json::parse(&http_get("https://pypi.org/pypi/breptile/json")?)?;
            let v = j.path("info/version").and_then(|x| x.str()).ok_or("PyPI liefert keine Version")?.to_string();
            Ok(Update { id, current, latest: v, url: String::new(), size: None, sha256: None, sha_url: None, min_os: None, team_id: None })
        }
        EngineId::FreeCad => {
            // Die neueste Veröffentlichung hat anfangs oft nur den Quelltext (Pakete werden noch
            // gebaut): die neueste nehmen, die ein Paket für diesen Mac enthält.
            let list = json::parse(&http_get("https://api.github.com/repos/FreeCAD/FreeCAD/releases?per_page=15")?)?;
            let arch = if apple_silicon() { "arm64" } else { "x86_64" };
            for rel in list.arr() {
                if rel.get("prerelease").and_then(|x| x.bool()) == Some(true) || rel.get("draft").and_then(|x| x.bool()) == Some(true) {
                    continue;
                }
                let tag = rel.get("tag_name").and_then(|x| x.str()).unwrap_or("?").to_string();
                let assets = rel.get("assets").map(|a| a.arr()).unwrap_or(&[]);
                let Some(a) = assets.iter().find(|x| {
                    let n = x.get("name").and_then(|n| n.str()).unwrap_or("").to_lowercase();
                    n.contains("macos") && n.contains(arch) && n.ends_with(".dmg")
                }) else {
                    continue;
                };
                let name = a.get("name").and_then(|x| x.str()).unwrap_or("");
                let sha_url = assets
                    .iter()
                    .find(|x| {
                        let n = x.get("name").and_then(|n| n.str()).unwrap_or("");
                        n.starts_with(name) && n.to_lowercase().contains("sha256")
                    })
                    .and_then(|x| x.get("browser_download_url"))
                    .and_then(|u| u.str())
                    .map(|s| s.to_string());
                return Ok(Update {
                    id,
                    current,
                    latest: tag.trim_start_matches('v').to_string(),
                    url: a.get("browser_download_url").and_then(|u| u.str()).unwrap_or("").to_string(),
                    size: a.get("size").and_then(|x| x.num()).map(|x| x as u64),
                    sha256: None,
                    sha_url,
                    min_os: None,
                    team_id: None,
                });
            }
            Err("Keine FreeCAD-Version mit Paket für diesen Mac gefunden.".into())
        }
    }
}

/// Installiert bzw. aktualisiert die Engine. Rückgabe: installierte Version.
pub fn install(u: &Update, progress: &dyn Fn(f32, &str), cancel: Option<&AtomicBool>) -> Result<String, String> {
    match u.id {
        EngineId::Mesh2Step => Err("Mesh2STEP wird mit der App aktualisiert.".into()),
        EngineId::Stl2Step => install_stl2step(u, progress, cancel),
        EngineId::Breptile => install_breptile(u, progress, cancel),
        EngineId::FreeCad => install_freecad(u, progress, cancel),
    }
}

fn install_stl2step(u: &Update, progress: &dyn Fn(f32, &str), cancel: Option<&AtomicBool>) -> Result<String, String> {
    if let (Some(min), Some(mv)) = (&u.min_os, macos_version()) {
        if version_cmp(&mv, min) == std::cmp::Ordering::Less {
            return Err(format!("stl2step {} braucht macOS {min} (dieser Mac: {mv}).", u.latest));
        }
    }
    let tmp = tmp_dir("stl2step");
    let pkg = tmp.join("engine.tar.gz");
    download(&u.url, &pkg, u.size, progress, cancel)?;
    progress(0.82, "Prüfsumme kontrollieren");
    if let Some(want) = &u.sha256 {
        let got = sha256(&pkg)?;
        if &got != want {
            let _ = std::fs::remove_dir_all(&tmp);
            return Err("Die Prüfsumme des heruntergeladenen Pakets stimmt nicht – Update abgebrochen.".into());
        }
    }
    progress(0.88, "Entpacken");
    let base = data_dir().join("stl2step");
    let dest_tmp = base.join(format!("{}.tmp", u.latest));
    let _ = std::fs::remove_dir_all(&dest_tmp);
    std::fs::create_dir_all(&dest_tmp).map_err(|e| e.to_string())?;
    let o = run(&sys("tar"), &["-xf", &pkg.to_string_lossy(), "-C", &dest_tmp.to_string_lossy()], &[], None, cancel, Duration::from_secs(600))?;
    if o.code != Some(0) {
        return Err(format!("Entpacken fehlgeschlagen: {}", o.stderr.trim()));
    }
    let exe = find_exe(&dest_tmp, "stl2step").ok_or("Im Paket wurde kein stl2step gefunden.")?;
    // Signatur des Herstellers prüfen (Apple Developer ID)
    if let Some(team) = &u.team_id {
        if cfg!(target_os = "macos") {
            progress(0.93, "Signatur prüfen");
            let o = run(&sys("codesign"), &["-dv", "--verbose=2", &exe.to_string_lossy()], &[], None, None, Duration::from_secs(60))?;
            let txt = format!("{}{}", o.stdout, o.stderr);
            if !txt.contains(&format!("TeamIdentifier={team}")) {
                let _ = std::fs::remove_dir_all(&dest_tmp);
                return Err("Die Signatur von stl2step passt nicht zum Hersteller – Update abgebrochen.".into());
            }
        }
    }
    // kurzer Funktionstest
    progress(0.96, "Funktion testen");
    let t = run(&exe, &["--help"], &[], exe.parent(), None, Duration::from_secs(60))?;
    if t.code.is_none() || (t.stdout.is_empty() && t.stderr.is_empty()) {
        return Err("Das neue stl2step startet nicht.".into());
    }
    std::fs::write(dest_tmp.join("VERSION"), &u.latest).map_err(|e| e.to_string())?;
    let dest = base.join(&u.latest);
    let _ = std::fs::remove_dir_all(&dest);
    std::fs::rename(&dest_tmp, &dest).map_err(|e| e.to_string())?;
    // ältere eigene Versionen entfernen
    if let Ok(rd) = std::fs::read_dir(&base) {
        for e in rd.flatten() {
            let p = e.path();
            if p != dest && p.is_dir() {
                let _ = std::fs::remove_dir_all(p);
            }
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
    progress(1.0, "Fertig");
    Ok(u.latest.clone())
}

/// Programm in einer Python-Umgebung (Windows: Scripts/x.exe, sonst bin/x).
fn venv_bin(venv: &Path, name: &str) -> PathBuf {
    if cfg!(windows) {
        venv.join("Scripts").join(format!("{name}.exe"))
    } else {
        venv.join("bin").join(name)
    }
}

/// uv (Python-Paketwerkzeug als einzelne Datei) bereitstellen.
fn ensure_uv(progress: &dyn Fn(f32, &str), cancel: Option<&AtomicBool>) -> Result<PathBuf, String> {
    let dir = data_dir().join("uv");
    if let Some(x) = find_exe(&dir, "uv") {
        return Ok(x);
    }
    let target = if cfg!(target_os = "macos") {
        if apple_silicon() { "aarch64-apple-darwin" } else { "x86_64-apple-darwin" }
    } else if cfg!(windows) {
        if cfg!(target_arch = "aarch64") { "aarch64-pc-windows-msvc" } else { "x86_64-pc-windows-msvc" }
    } else if cfg!(target_arch = "aarch64") {
        "aarch64-unknown-linux-gnu"
    } else {
        "x86_64-unknown-linux-gnu"
    };
    // Windows-Pakete von uv sind .zip, die anderen .tar.gz
    let ext = if cfg!(windows) { "zip" } else { "tar.gz" };
    let url = format!("https://github.com/astral-sh/uv/releases/latest/download/uv-{target}.{ext}");
    let tmp = tmp_dir("uv");
    let pkg = tmp.join(format!("uv.{ext}"));
    download(&url, &pkg, None, &|p, m| progress(p * 0.3, m), cancel)?;
    // Prüfsumme der Veröffentlichung (uv-<target>.tar.gz.sha256)
    if let Ok(sum) = http_get(&format!("{url}.sha256")) {
        if let Some(want) = sum.split_whitespace().next() {
            if sha256(&pkg)? != want.to_lowercase() {
                return Err("Die Prüfsumme von uv stimmt nicht.".into());
            }
        }
    }
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let o = run(&sys("tar"), &["-xf", &pkg.to_string_lossy(), "-C", &dir.to_string_lossy()], &[], None, cancel, Duration::from_secs(300))?;
    if o.code != Some(0) {
        return Err(format!("uv konnte nicht entpackt werden: {}", o.stderr.trim()));
    }
    let _ = std::fs::remove_dir_all(&tmp);
    find_exe(&dir, "uv").ok_or_else(|| "uv wurde nicht gefunden.".into())
}

fn install_breptile(u: &Update, progress: &dyn Fn(f32, &str), cancel: Option<&AtomicBool>) -> Result<String, String> {
    let uv = ensure_uv(progress, cancel)?;
    let base = breptile_dir();
    let uvd = data_dir().join("uv");
    let envs = vec![("UV_PYTHON_INSTALL_DIR", uvd.join("python").to_string_lossy().to_string()), ("UV_CACHE_DIR", uvd.join("cache").to_string_lossy().to_string()), ("UV_NO_CONFIG", "1".to_string())];
    let venv = base.join("venv");
    if !venv_bin(&venv, "python").exists() {
        progress(0.35, "Python-Umgebung einrichten");
        std::fs::create_dir_all(&base).map_err(|e| e.to_string())?;
        let o = run(&uv, &["venv", "--python", "3.12", &venv.to_string_lossy()], &envs, None, cancel, Duration::from_secs(1200))?;
        if o.code != Some(0) {
            return Err(format!("Python-Umgebung konnte nicht eingerichtet werden: {}", last_line(&o.stderr)));
        }
    }
    progress(0.6, &format!("breptile {} installieren", u.latest));
    let py = venv_bin(&venv, "python");
    let spec = format!("breptile=={}", u.latest);
    let o = run(&uv, &["pip", "install", "--python", &py.to_string_lossy(), "--upgrade", &spec], &envs, None, cancel, Duration::from_secs(1800))?;
    if o.code != Some(0) {
        return Err(format!("breptile konnte nicht installiert werden: {}", last_line(&o.stderr)));
    }
    std::fs::write(base.join("VERSION"), &u.latest).map_err(|e| e.to_string())?;
    progress(1.0, "Fertig");
    Ok(u.latest.clone())
}

fn install_freecad(u: &Update, progress: &dyn Fn(f32, &str), cancel: Option<&AtomicBool>) -> Result<String, String> {
    if !cfg!(target_os = "macos") {
        return Err("FreeCAD wird hier nur auf dem Mac automatisch installiert.".into());
    }
    let tmp = tmp_dir("freecad");
    let dmg = tmp.join("FreeCAD.dmg");
    download(&u.url, &dmg, u.size, progress, cancel)?;
    if let Some(su) = &u.sha_url {
        progress(0.82, "Prüfsumme kontrollieren");
        if let Ok(t) = http_get(su) {
            if let Some(want) = t.split_whitespace().find(|w| w.len() == 64 && w.chars().all(|c| c.is_ascii_hexdigit())) {
                if sha256(&dmg)? != want.to_lowercase() {
                    let _ = std::fs::remove_dir_all(&tmp);
                    return Err("Die Prüfsumme des FreeCAD-Pakets stimmt nicht – abgebrochen.".into());
                }
            }
        }
    }
    progress(0.86, "Paket öffnen");
    let mnt = tmp.join("mnt");
    let _ = std::fs::create_dir_all(&mnt);
    let o = run(&sys("hdiutil"), &["attach", "-nobrowse", "-readonly", "-noautoopen", "-mountpoint", &mnt.to_string_lossy(), &dmg.to_string_lossy()], &[], None, cancel, Duration::from_secs(600))?;
    if o.code != Some(0) {
        return Err(format!("Das FreeCAD-Paket ließ sich nicht öffnen: {}", last_line(&o.stderr)));
    }
    let result = (|| -> Result<(), String> {
        let app = std::fs::read_dir(&mnt).map_err(|e| e.to_string())?.flatten().map(|e| e.path()).find(|p| p.extension().map(|e| e == "app").unwrap_or(false)).ok_or("Im Paket wurde keine FreeCAD.app gefunden.")?;
        progress(0.9, "FreeCAD kopieren");
        let base = freecad_dir();
        std::fs::create_dir_all(&base).map_err(|e| e.to_string())?;
        let new = base.join("FreeCAD.app.neu");
        let _ = std::fs::remove_dir_all(&new);
        let o = run(&sys("ditto"), &[&app.to_string_lossy(), &new.to_string_lossy()], &[], None, cancel, Duration::from_secs(1800))?;
        if o.code != Some(0) {
            return Err(format!("Kopieren fehlgeschlagen: {}", last_line(&o.stderr)));
        }
        if freecad_cmd_in(&new).is_none() {
            let _ = std::fs::remove_dir_all(&new);
            return Err("In diesem FreeCAD fehlt freecadcmd.".into());
        }
        let cur = base.join("FreeCAD.app");
        let _ = std::fs::remove_dir_all(&cur);
        std::fs::rename(&new, &cur).map_err(|e| e.to_string())?;
        std::fs::write(base.join("VERSION"), u.latest.trim_start_matches('v')).map_err(|e| e.to_string())?;
        Ok(())
    })();
    let _ = run(&sys("hdiutil"), &["detach", "-quiet", &mnt.to_string_lossy()], &[], None, None, Duration::from_secs(120));
    let _ = std::fs::remove_dir_all(&tmp);
    result?;
    progress(1.0, "Fertig");
    Ok(u.latest.trim_start_matches('v').to_string())
}

fn last_line(s: &str) -> String {
    s.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_string()
}

/// Eigene Installation einer Engine entfernen (nicht die mitgelieferte, nicht System-Installationen).
pub fn uninstall(id: EngineId) -> Result<(), String> {
    let d = match id {
        EngineId::Stl2Step => data_dir().join("stl2step"),
        EngineId::Breptile => breptile_dir(),
        EngineId::FreeCad => freecad_dir(),
        EngineId::Mesh2Step => return Ok(()),
    };
    if d.exists() {
        std::fs::remove_dir_all(&d).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Befehl zum Selbst-Aktualisieren einer System-Installation (zur Anzeige)
pub fn system_update_hint(id: EngineId) -> &'static str {
    match id {
        EngineId::Breptile => "pipx upgrade breptile",
        EngineId::FreeCad => "Neue Version von freecad.org installieren",
        _ => "",
    }
}

// ---------------------------------------------------------------------------
// Umwandeln und Lesen mit externen Engines

#[derive(Clone, Debug)]
pub struct ExtOptions {
    /// stl2step: TrueForm statt Verbatim
    pub trueform: bool,
    /// breptile: auto, prismatic oder tessellated
    pub breptile_mode: String,
    /// FreeCAD: ebene Flächen zusammenfassen (removeSplitter)
    pub freecad_refine: bool,
    /// FreeCAD: Startwert der Vernäh-Toleranz (mm)
    pub sew_tol: f64,
}

impl Default for ExtOptions {
    fn default() -> Self {
        ExtOptions { trueform: true, breptile_mode: "auto".into(), freecad_refine: true, sew_tol: 0.01 }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ExtOutcome {
    pub warnings: Vec<String>,
    pub notes: Vec<String>,
    pub watertight: Option<bool>,
    pub volume_delta_pct: Option<f64>,
    pub open_shells: usize,
}

/// Skript für freecadcmd: Umwandeln (M2S_MODE=convert) oder STEP lesen (M2S_MODE=mesh).
pub const FREECAD_SCRIPT: &str = r#"
import os, sys, json, struct
res = {"ok": False}
try:
    import FreeCAD, Part
    mode = os.environ.get("M2S_MODE", "convert")
    src = os.environ["M2S_IN"]; dst = os.environ["M2S_OUT"]
    if mode == "mesh":
        shape = Part.read(src)
        defl = float(os.environ.get("M2S_DEFL", "0.01"))
        pts, tris = shape.tessellate(defl)
        with open(dst, "wb") as f:
            f.write(b"\0" * 80); f.write(struct.pack("<I", len(tris)))
            for a, b, c in tris:
                f.write(struct.pack("<3f", 0, 0, 0))
                for i in (a, b, c):
                    p = pts[i]; f.write(struct.pack("<3f", p.x, p.y, p.z))
                f.write(b"\0\0")
        res.update(ok=True, faces=len(shape.Faces), solids=len(shape.Solids), triangles=len(tris))
    else:
        import Mesh
        m = Mesh.Mesh(src)
        refine = os.environ.get("M2S_REFINE", "1") == "1"
        tol0 = float(os.environ.get("M2S_SEW", "0.01"))
        best = None
        for tol in (tol0, tol0 * 5, tol0 * 25):
            shape = Part.Shape()
            shape.makeShapeFromMesh(m.Topology, tol)
            try:
                shell = Part.Shell(shape.Faces)
            except Exception:
                shell = None
            if shell is not None and shell.isClosed():
                solid = Part.Solid(shell)
                if refine:
                    try:
                        solid = solid.removeSplitter()
                    except Exception:
                        pass
                best = (solid, True, tol)
                break
            if best is None:
                best = (shell if shell is not None else shape, False, tol)
        out, closed, tol = best
        out.exportStep(dst)
        res.update(ok=True, closed=closed, sew_tol=tol, faces=len(out.Faces), valid=bool(out.isValid()),
                   volume=(out.Volume if closed else None))
except Exception as e:
    res["error"] = str(e)
sys.stdout.write("M2S_RESULT " + json.dumps(res) + "\n"); sys.stdout.flush()
"#;

fn freecad_run(info: &EngineInfo, mode: &str, src: &Path, dst: &Path, extra: &[(&str, String)], cancel: Option<&AtomicBool>) -> Result<Json, String> {
    let exe = info.exe.as_ref().ok_or("FreeCAD ist nicht installiert.")?;
    let tmp = tmp_dir("fc");
    let script = tmp.join("mesh2step_freecad.py");
    std::fs::write(&script, FREECAD_SCRIPT).map_err(|e| e.to_string())?;
    let mut envs = vec![("M2S_MODE", mode.to_string()), ("M2S_IN", src.to_string_lossy().to_string()), ("M2S_OUT", dst.to_string_lossy().to_string())];
    envs.extend_from_slice(extra);
    let o = run(exe, &[&script.to_string_lossy()], &envs, Some(&tmp), cancel, Duration::from_secs(3600))?;
    let _ = std::fs::remove_dir_all(&tmp);
    let line = o.stdout.lines().rev().find_map(|l| l.strip_prefix("M2S_RESULT ")).ok_or_else(|| format!("FreeCAD hat kein Ergebnis geliefert. {}", last_line(&o.stderr)))?;
    let j = json::parse(line)?;
    if j.get("ok").and_then(|x| x.bool()) != Some(true) {
        return Err(format!("FreeCAD: {}", j.get("error").and_then(|x| x.str()).unwrap_or("unbekannter Fehler")));
    }
    Ok(j)
}

/// Netz (STL) mit einer externen Engine in STEP umwandeln.
pub fn convert_file(info: &EngineInfo, stl: &Path, out: &Path, o: &ExtOptions, cancel: Option<&AtomicBool>) -> Result<ExtOutcome, String> {
    if !info.available {
        return Err(info.note.clone().unwrap_or_else(|| format!("{} ist nicht installiert.", info.id.name())));
    }
    let _ = std::fs::remove_file(out);
    let mut r = ExtOutcome::default();
    match info.id {
        EngineId::Mesh2Step => return Err("intern".into()),
        EngineId::Stl2Step => {
            let exe = info.exe.as_ref().unwrap();
            let (s, d) = (stl.to_string_lossy().to_string(), out.to_string_lossy().to_string());
            let mut args = vec![s.as_str(), "-o", d.as_str(), "--quiet"];
            args.extend_from_slice(if o.trueform { &["--engine", "trueform"] } else { &["--engine", "verbatim"] });
            let ro = run(exe, &args, &[], exe.parent(), cancel, Duration::from_secs(3600))?;
            let line = ro.stdout.lines().rev().find_map(|l| l.strip_prefix("RESULT ")).ok_or_else(|| format!("stl2step hat kein Ergebnis geliefert. {}", last_line(&ro.stderr)))?;
            let j = json::parse(line)?;
            if j.get("ok").and_then(|x| x.bool()) != Some(true) {
                return Err(format!("stl2step: {}", j.get("error").and_then(|x| x.str()).unwrap_or("unbekannter Fehler")));
            }
            r.watertight = j.get("watertight").and_then(|x| x.bool());
            r.volume_delta_pct = j.get("volumeDeltaPct").and_then(|x| x.num()).filter(|v| *v >= 0.0);
            r.open_shells = j.get("openShells").and_then(|x| x.num()).unwrap_or(0.0) as usize;
            for w in j.get("warnings").map(|w| w.arr()).unwrap_or(&[]) {
                if let Some(s) = w.str() {
                    r.warnings.push(format!("stl2step: {s}"));
                }
            }
            if o.trueform {
                if let Some(n) = j.get("smoothSkippedComponents").and_then(|x| x.num()) {
                    if n > 0.0 {
                        r.warnings.push(format!("{} Teil(e) waren zu unsauber für TrueForm und wurden facettiert.", n as usize));
                    }
                }
            }
        }
        EngineId::Breptile => {
            let exe = info.exe.as_ref().unwrap();
            let tmp = tmp_dir("bt");
            let rep = tmp.join("bericht.json");
            let (s, d, rp) = (stl.to_string_lossy().to_string(), out.to_string_lossy().to_string(), rep.to_string_lossy().to_string());
            let args = [s.as_str(), d.as_str(), "--mode", o.breptile_mode.as_str(), "--report", rp.as_str(), "--force"];
            let ro = run(exe, &args, &[], Some(&tmp), cancel, Duration::from_secs(3600))?;
            if ro.code != Some(0) || !out.exists() {
                let _ = std::fs::remove_dir_all(&tmp);
                return Err(format!("breptile: {}", last_line(&ro.stderr).chars().take(300).collect::<String>()));
            }
            if let Ok(t) = std::fs::read_to_string(&rep) {
                if let Ok(j) = json::parse(&t) {
                    let fb = j.path("regions/fallback").and_then(|x| x.num()).unwrap_or(0.0) as usize;
                    if fb > 0 {
                        r.warnings.push(format!("breptile: {fb} Bereich(e) passten auf keine Grundform und wurden facettiert"));
                    }
                    if j.get("valid_brep").and_then(|x| x.bool()) == Some(false) {
                        r.warnings.push("breptile meldet die Form als nicht ganz gültig – bitte im CAD prüfen.".into());
                    }
                    r.watertight = j.get("watertight").and_then(|x| x.bool());
                    if r.watertight == Some(false) {
                        r.open_shells = 1;
                    }
                }
            }
            let _ = std::fs::remove_dir_all(&tmp);
        }
        EngineId::FreeCad => {
            let j = freecad_run(info, "convert", stl, out, &[("M2S_REFINE", if o.freecad_refine { "1" } else { "0" }.to_string()), ("M2S_SEW", format!("{}", o.sew_tol))], cancel)?;
            let closed = j.get("closed").and_then(|x| x.bool()).unwrap_or(false);
            r.watertight = Some(closed);
            if !closed {
                r.open_shells = 1;
                r.warnings.push("FreeCAD konnte die Hülle nicht schließen – als offene Fläche gespeichert.".into());
            } else if let Some(t) = j.get("sew_tol").and_then(|x| x.num()) {
                if t > o.sew_tol * 1.01 {
                    r.notes.push(format!("Vernäh-Toleranz automatisch auf {} mm erhöht", format!("{t:.3}").replace('.', ",")));
                }
            }
            if j.get("valid").and_then(|x| x.bool()) == Some(false) {
                r.warnings.push("FreeCAD meldet die Form als nicht ganz gültig – bitte im CAD prüfen.".into());
            }
        }
    }
    if !out.exists() {
        return Err(format!("{} hat keine STEP-Datei geschrieben.", info.id.name()));
    }
    Ok(r)
}

/// STEP-Datei mit einer externen Engine in ein Netz (binäre STL) umwandeln.
pub fn step_to_stl(info: &EngineInfo, step: &Path, out_stl: &Path, defl: f64, cancel: Option<&AtomicBool>) -> Result<(), String> {
    if !info.available {
        return Err(info.note.clone().unwrap_or_else(|| format!("{} ist nicht installiert.", info.id.name())));
    }
    match info.id {
        EngineId::Stl2Step => {
            let exe = info.exe.as_ref().unwrap();
            let (s, d) = (step.to_string_lossy().to_string(), out_stl.to_string_lossy().to_string());
            let ro = run(exe, &["--mesh", &s, "-o", &d, "--quiet"], &[], exe.parent(), cancel, Duration::from_secs(1800))?;
            let line = ro.stdout.lines().rev().find_map(|l| l.strip_prefix("MESH_RESULT ")).ok_or_else(|| format!("stl2step hat kein Ergebnis geliefert. {}", last_line(&ro.stderr)))?;
            let j = json::parse(line)?;
            if j.get("ok").and_then(|x| x.bool()) != Some(true) {
                return Err(format!("stl2step: {}", j.get("error").and_then(|x| x.str()).unwrap_or("unbekannter Fehler")));
            }
        }
        EngineId::FreeCad => {
            freecad_run(info, "mesh", step, out_stl, &[("M2S_DEFL", format!("{defl}"))], cancel)?;
        }
        _ => return Err(format!("{} kann keine STEP-Dateien lesen.", info.id.name())),
    }
    if !out_stl.exists() {
        return Err("Es wurde kein Netz erzeugt.".into());
    }
    Ok(())
}

/// Ob eine Engine STEP lesen kann (für den Maßvergleich).
pub fn can_read_step(id: EngineId) -> bool {
    matches!(id, EngineId::Mesh2Step | EngineId::Stl2Step | EngineId::FreeCad)
}

/// Grenzen von FreeCAD – werden in der Oberfläche angezeigt.
pub const FREECAD_LIMITS: [&str; 3] = [
    "Bohrungen und Rundungen bleiben eckig (viele schmale Flächen). Echte Zylinder liefern Mesh2STEP oder stl2step.",
    "Mesh2STEP erledigt vorher automatisch, was sonst Handarbeit ist: Netz reparieren, sehr dichte Netze ausdünnen, Vernäh-Toleranz anpassen.",
    "Bleibt die Hülle trotzdem offen, wird das gemeldet und als offene Fläche gespeichert.",
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versionen() {
        assert_eq!(version_cmp("1.10.0", "1.9.3"), std::cmp::Ordering::Greater);
        assert_eq!(version_cmp("v1.7.0", "1.7.0"), std::cmp::Ordering::Equal);
        assert_eq!(version_cmp("26.0.1", "26.0"), std::cmp::Ordering::Greater);
    }
}
