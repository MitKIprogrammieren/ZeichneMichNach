// Skizze → Zeichnung – Home-Assistant-Karte
// Foto einer Handskizze → KI (ai_task) liest Maße → Fertigungszeichnung (SVG, A4) für Flach-, Dreh-, Biege- und Frästeile
const SZ_VERSION = "4.2.0";

/* ---------------- Einstellungen & Modell ---------------- */
const SZ_SET_DEFAULT = {
  firma: "", zeichner: "", toleranz: "ISO 2768-m", werkstoff: "Stahlblech verzinkt", werkstoff_dreh: "", werkstoff_fraes: "",
  lochd: 6.6, massstab: "auto", typ: "auto", entity: "", zusatz: "", kfaktor: "", fertigung: "auto",
};
const SZ_DEFAULT = {
  typ: "flach", name: "Lasche", material: "Stahlblech verzinkt", oberflaeche: "", stueck: 1, toleranz: "", nr: "",
  dicke: 2, laenge: 150, breite: 40, eckradius: 0, bohrungen: [{ x: 30, y: 20, d: 6.6 }],
  aussen: [{ l: 20, d: 60, tol: "", gewinde: "" }, { l: 40, d: 30, tol: "h7", gewinde: "" }],
  innen: [{ l: 60, d: 16, tol: "H7" }], fase_links: 1, fase_rechts: 1,
  lochkreis: { anzahl: 0, lk: 0, d: 0, gewinde: "", tiefe: 0 },
  ri: null, massart: "aussen", schenkel: [{ l: 40 }, { l: 60 }, { l: 40 }],
  biegungen: [{ winkel: 90, richtung: "links" }, { winkel: 90, richtung: "links" }],
  hoehe: 20, nuten: [], fbohrungen: [],
  hinweise: [], unsicher: [],
};
const fmt = (v) => {
  const n = Number(v);
  if (!isFinite(n)) return "";
  return (Math.round(n * 100) / 100).toString().replace(".", ",");
};
const esc = (s) => String(s ?? "").replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
const num = (v, d) => { const x = parseFloat(String(v ?? "").replace(",", ".")); return isFinite(x) ? x : d; };

function szNormalize(m, st = SZ_SET_DEFAULT) {
  const n = num;
  const o = Object.assign({}, SZ_DEFAULT, m || {});
  o.typ = ["dreh", "biege", "fraes"].includes(o.typ) ? o.typ : "flach";
  ["name", "material", "oberflaeche", "toleranz", "nr"].forEach((k) => (o[k] = String(o[k] ?? "")));
  o.stueck = Math.max(1, Math.round(n(o.stueck, 1)));
  // Flachteil
  o.dicke = n(o.dicke, 2);
  o.laenge = Math.max(1, n(o.laenge, 100)); o.breite = Math.max(1, n(o.breite, 40));
  o.eckradius = Math.max(0, Math.min(n(o.eckradius, 0), Math.min(o.laenge, o.breite) / 2));
  o.bohrungen = (Array.isArray(o.bohrungen) ? o.bohrungen : []).map((b) => {
    const d = n(b.d, null);
    return { x: n(b.x, 0), y: n(b.y, 0), d: d == null ? n(st.lochd, 6.6) : d, angenommen: !!b.angenommen || d == null,
      schenkel: Math.max(0, Math.round(n(b.schenkel, 0))), a: n(b.a, 0) };
  });
  // Drehteil
  const segs = (a) => (Array.isArray(a) ? a : []).map((s) => ({
    l: Math.max(0, n(s.l, 0)), d: Math.max(0, n(s.d, 0)), tol: String(s.tol ?? ""), gewinde: String(s.gewinde ?? ""),
  })).filter((s) => s.l > 0);
  o.aussen = segs(o.aussen).filter((s) => s.d > 0);
  if (!o.aussen.length) o.aussen = [{ l: 50, d: 30, tol: "", gewinde: "" }];
  const Ld = o.aussen.reduce((a, s) => a + s.l, 0);
  let acc = 0;
  o.innen = segs(o.innen).filter((s) => { if (acc >= Ld - 1e-9) return false; s.l = Math.min(s.l, Ld - acc); acc += s.l; return true; });
  o.fase_links = Math.max(0, n(o.fase_links, 0)); o.fase_rechts = Math.max(0, n(o.fase_rechts, 0));
  const lk = o.lochkreis || {};
  o.lochkreis = { anzahl: Math.max(0, Math.round(n(lk.anzahl, 0))), lk: Math.max(0, n(lk.lk, 0)), d: Math.max(0, n(lk.d, 0)), gewinde: String(lk.gewinde ?? ""), tiefe: Math.max(0, n(lk.tiefe, 0)) };
  // Biegeteil
  o.ri = Math.max(0, n(o.ri, o.dicke));
  o.massart = o.massart === "innen" ? "innen" : "aussen";
  o.schenkel = (Array.isArray(o.schenkel) ? o.schenkel : []).map((v) => ({ l: Math.max(0.1, n(v && typeof v === "object" ? v.l : v, 10)) }));
  if (o.schenkel.length < 2) o.schenkel = [{ l: 40 }, { l: 60 }];
  const bg = Array.isArray(o.biegungen) ? o.biegungen : [];
  o.biegungen = o.schenkel.slice(1).map((_, i) => {
    const b = bg[i] || {};
    return { winkel: Math.min(179, Math.max(1, n(b.winkel, 90))), richtung: b.richtung === "rechts" ? "rechts" : "links" };
  });
  // Frästeil
  o.hoehe = Math.max(0.1, n(o.hoehe, 20));
  o.nuten = (Array.isArray(o.nuten) ? o.nuten : []).map((u) => ({
    seite: ["unten", "oben", "vorne", "hinten"].includes(u.seite) ? u.seite : "unten",
    abstand: Math.max(0, n(u.abstand, 0)), breite: Math.max(0.1, n(u.breite, 5)), tiefe: Math.max(0.1, n(u.tiefe, 5)),
  }));
  o.fbohrungen = (Array.isArray(o.fbohrungen) ? o.fbohrungen : []).map((b) => {
    const d = n(b.d, null);
    return { richtung: ["x", "y", "z"].includes(b.richtung) ? b.richtung : "z", d: d == null ? n(st.lochd, 6.6) : d,
      angenommen: !!b.angenommen || (d == null && !b.gewinde), gewinde: String(b.gewinde ?? ""), a: n(b.a, 0), b: n(b.b, 0), tiefe: Math.max(0, n(b.tiefe, 0)) };
  });
  o.hinweise = Array.isArray(o.hinweise) ? o.hinweise.filter(Boolean).map(String) : String(o.hinweise || "").split("\n").filter(Boolean);
  o.unsicher = Array.isArray(o.unsicher) ? o.unsicher.filter(Boolean).map(String) : [];
  return o;
}

/* ---------------- Zeichen-Grundlagen ---------------- */
function szCtx() {
  const out = [];
  const add = (s) => out.push(s);
  const ln = (x1, y1, x2, y2, c = "thin") => add(`<line class="${c}" x1="${x1}" y1="${y1}" x2="${x2}" y2="${y2}"/>`);
  const tx = (x, y, s, o = {}) => add(`<text x="${x}" y="${y}" font-size="${o.size || 3.5}" text-anchor="${o.anchor || "middle"}"${o.rot ? ` transform="rotate(${o.rot} ${x} ${y})"` : ""}${o.weight ? ` font-weight="${o.weight}"` : ""}${o.fill ? ` fill="${o.fill}"` : ""}${o.mid ? ` dominant-baseline="central"` : ""}>${esc(s)}</text>`);
  const dimLine = (x1, y1, x2, y2) => add(`<line class="thin" x1="${x1}" y1="${y1}" x2="${x2}" y2="${y2}" marker-start="url(#a)" marker-end="url(#a)"/>`);
  const arr = (x1, y1, x2, y2) => add(`<line class="thin" x1="${x1}" y1="${y1}" x2="${x2}" y2="${y2}" marker-end="url(#a)"/>`);
  // waagerechtes Maß; kurze Strecke → Pfeile außen, Text zur Seite side (1 rechts / -1 links)
  const hDim = (x1, x2, y, label, side = 1) => {
    const a = Math.min(x1, x2), b = Math.max(x1, x2);
    const tw = String(label).length * 2.1 + 2;
    if (b - a >= tw + 1) { dimLine(a, y, b, y); tx((a + b) / 2, y - 1.2, label); return; }
    arr(a - 5, y, a, y); arr(b + 5, y, b, y); ln(a, y, b, y);
    if (side > 0) tx(b + 5.5, y + 1.2, label, { anchor: "start" }); else tx(a - 5.5, y + 1.2, label, { anchor: "end" });
  };
  // senkrechtes Maß; side 1 = Text unterhalb, -1 = oberhalb
  const vDim = (xl, y1, y2, label, side = 1) => {
    const a = Math.min(y1, y2), b = Math.max(y1, y2);
    const tw = String(label).length * 2.1 + 2;
    if (b - a >= tw + 1) { dimLine(xl, b, xl, a); tx(xl - 1.2, (a + b) / 2, label, { rot: -90 }); return; }
    arr(xl, a - 5, xl, a); arr(xl, b + 5, xl, b); ln(xl, a, xl, b);
    if (side > 0) tx(xl - 1.2, b + 5.5, label, { rot: -90, anchor: "end" }); else tx(xl - 1.2, a - 5.5, label, { rot: -90, anchor: "start" });
  };
  return { out, add, ln, tx, dimLine, arr, hDim, vDim };
}

const SZ_SCALES = [5, 2, 1, 0.5, 0.2, 0.1, 0.05, 0.02];
function szPickScale(fit, st) {
  const f = String(st.massstab || "auto");
  const mt = f.match(/^(\d+(?:[.,]\d+)?):(\d+(?:[.,]\d+)?)$/);
  if (mt) return num(mt[1], 1) / num(mt[2], 1);
  return SZ_SCALES.find(fit) || SZ_SCALES[SZ_SCALES.length - 1];
}
const szScaleLabel = (s) => (s >= 1 ? `${fmt(s)}:1` : `1:${fmt(Math.round((1 / s) * 100) / 100)}`);

/* ---------------- Flachteil-Kern (auch für Abwicklungen) ---------------- */
function szFlatTiers(L, B, holes) {
  const uniq = (a) => [...new Set(a.map((v) => Math.round(v * 100) / 100))].sort((p, q) => p - q);
  const leftX = uniq(holes.filter((b) => b.x <= L / 2).map((b) => b.x));
  const rightX = uniq(holes.filter((b) => b.x > L / 2).map((b) => L - b.x));
  const yBot = uniq(holes.filter((b) => b.y <= B / 2).map((b) => b.y));
  const yTop = uniq(holes.filter((b) => b.y > B / 2).map((b) => B - b.y));
  return { leftX, rightX, yBot, yTop, xTiers: Math.max(leftX.length, rightX.length), yTiers: Math.max(yBot.length, yTop.length) };
}

// zeichnet Kontur, Bohrungen und Bemaßung eines Rechteckteils; P = {L, B, R, holes:[{x,y,d}]}
function szFlatCore(c, P, s, ox, oyTop, opts = {}) {
  const { add, ln, tx, dimLine, hDim } = c;
  const { L, B, R = 0, holes } = P;
  const T = szFlatTiers(L, B, holes);
  const { leftX, rightX, yBot, yTop } = T;
  const W = L * s, H = B * s, oyB = oyTop + H;
  const X = (x) => ox + x * s, Y = (y) => oyB - y * s;
  if (R > 0) {
    const r = R * s;
    add(`<path class="thick" d="M${ox + r},${oyTop} H${ox + W - r} A${r},${r} 0 0 1 ${ox + W},${oyTop + r} V${oyB - r} A${r},${r} 0 0 1 ${ox + W - r},${oyB} H${ox + r} A${r},${r} 0 0 1 ${ox},${oyB - r} V${oyTop + r} A${r},${r} 0 0 1 ${ox + r},${oyTop} Z"/>`);
  } else add(`<rect class="thick" x="${ox}" y="${oyTop}" width="${W}" height="${H}"/>`);
  // Biegelinien
  const bends = opts.bends || [];
  bends.forEach((b) => {
    ln(X(b.x), oyTop - 2, X(b.x), oyB + 2, "bl");
    tx(X(b.x) - 1.2, (oyTop + oyB) / 2, b.label, { rot: -90, size: 2.6 });
  });
  for (const b of holes) {
    const cx = X(b.x), cy = Y(b.y), r = (b.d / 2) * s, e = r + 2;
    add(`<circle class="thick" cx="${cx}" cy="${cy}" r="${r}"/>`);
    ln(cx - e, cy, cx + e, cy, "ctr"); ln(cx, cy - e, cx, cy + e, "ctr");
  }
  // Gesamtmaße
  const yL = oyTop - 10, yTopExt = bends.length ? oyTop - 18 : yL;
  ln(ox, oyTop - 1.5, ox, yTopExt - 2); ln(ox + W, oyTop - 1.5, ox + W, yTopExt - 2);
  dimLine(ox, yL, ox + W, yL); tx(ox + W / 2, yL - 1.2, opts.lLabel || fmt(L));
  if (bends.length) {
    const xs = [0, ...bends.map((b) => b.x), L];
    bends.forEach((b) => ln(X(b.x), oyTop - 2, X(b.x), oyTop - 20, "thin"));
    for (let i = 0; i < xs.length - 1; i++) hDim(X(xs[i]), X(xs[i + 1]), oyTop - 18, fmt(Math.round((xs[i + 1] - xs[i]) * 10) / 10), i === 0 ? -1 : 1);
  }
  const xB = ox - 10;
  ln(ox - 1.5, oyTop, xB - 2, oyTop); ln(ox - 1.5, oyB, xB - 2, oyB);
  dimLine(xB, oyB, xB, oyTop); tx(xB - 1.2, (oyTop + oyB) / 2, fmt(B), { rot: -90 });
  // x-Maße
  const hx = (x1, x2, tier, label) => {
    const yl = oyB + 10 + tier * 8;
    ln(x1, oyB + 1.5, x1, yl + 2); ln(x2, oyB + 1.5, x2, yl + 2);
    hDim(x1, x2, yl, label, x1 <= ox + 0.01 ? 1 : -1);
  };
  const r1 = (v) => fmt(Math.round(v * 10) / 10);
  const lowest = (xv) => holes.filter((b) => Math.abs(b.x - xv) < 0.01).sort((p, q) => p.y - q.y)[0];
  leftX.forEach((v, i) => { const h = lowest(v); hx(ox, X(v), i, r1(v)); ln(X(v), Y(h.y) + (h.d / 2) * s + 1.5, X(v), oyB + 1.5); });
  rightX.forEach((v, i) => { const xv = L - v, h = lowest(xv); hx(X(xv), ox + W, i, r1(v)); ln(X(xv), Y(h.y) + (h.d / 2) * s + 1.5, X(xv), oyB + 1.5); });
  if (holes.length === 2 && leftX.length === 1 && rightX.length === 1) {
    const yl = oyB + 10;
    tx((X(leftX[0]) + X(L - rightX[0])) / 2, yl - 1.2, `(${r1(L - rightX[0] - leftX[0])})`);
    dimLine(X(leftX[0]), yl, X(L - rightX[0]), yl);
  }
  // y-Maße
  const vy = (yFrom, yTo, tier, label, holeX) => {
    const xl = ox + W + 10 + tier * 8;
    ln(holeX, yTo, xl + 2, yTo); ln(ox + W + 1.5, yFrom, xl + 2, yFrom);
    c.vDim(xl, yFrom, yTo, label, yFrom >= oyB - 0.01 ? -1 : 1);
  };
  const rightmost = (yv) => holes.filter((b) => Math.abs(b.y - yv) < 0.01).sort((p, q) => q.x - p.x)[0];
  yBot.forEach((v, i) => { const h = rightmost(v); vy(oyB, Y(v), i, r1(v), X(h.x) + (h.d / 2) * s + 2); });
  yTop.forEach((v, i) => { const yv = B - v, h = rightmost(yv); vy(oyTop, Y(yv), i, r1(v), X(h.x) + (h.d / 2) * s + 2); });
  // Durchmesser
  const groups = {};
  holes.forEach((b) => { const k = fmt(b.d); (groups[k] = groups[k] || []).push(b); });
  let gi = 0;
  for (const [k, hs] of Object.entries(groups)) {
    const b = hs.slice().sort((p, q) => p.x - q.x)[0];
    const cx = X(b.x), cy = Y(b.y), r = (b.d / 2) * s;
    const ex = cx + 14 + gi * 4, ey = Math.max(oyTop - 3, cy - 14) - gi * 5;
    add(`<line class="thin" x1="${ex}" y1="${ey}" x2="${cx + r * 0.707}" y2="${cy - r * 0.707}" marker-end="url(#a)"/>`);
    const label = `${hs.length > 1 ? hs.length + "× " : ""}Ø${k}`;
    ln(ex, ey, ex + label.length * 2.2, ey);
    tx(ex + 0.5, ey - 1, label, { anchor: "start" });
    gi++;
  }
  if (R > 0) {
    const r = R * s, px = ox + W - r + r * 0.707, py = oyTop + r - r * 0.707;
    add(`<line class="thin" x1="${px + 8}" y1="${py - 6}" x2="${px}" y2="${py}" marker-end="url(#a)"/>`);
    tx(px + 8.5, py - 7, `4× R${fmt(R)}`, { anchor: "start" });
  }
  return { W, H, oyB, T };
}

/* ---------------- Flachteil ---------------- */
function szDrawFlach(c, m, st) {
  const { add, ln, tx } = c;
  const L = m.laenge, B = m.breite, T = m.dicke;
  const holes = m.bohrungen.map((b) => ({ x: b.x, y: b.y, d: b.d }));
  const tiers = szFlatTiers(L, B, holes);
  const maxW = 200 - tiers.yTiers * 8, maxH = 92 - tiers.xTiers * 8;
  const s = szPickScale((k) => L * k <= maxW && B * k <= maxH, st);
  const W = L * s, H = B * s;
  const ox = 38 + Math.max(0, (maxW - W) / 2), oyTop = 32 + Math.max(0, (maxH - H) / 2);
  const Y = (y) => oyTop + H - y * s;
  szFlatCore(c, { L, B, R: m.eckradius, holes }, s, ox, oyTop);
  // Seitenansicht
  const sx = ox + W + 18 + tiers.yTiers * 8, tw = Math.max(T * s, 0.6);
  add(`<rect class="thick" x="${sx}" y="${oyTop}" width="${tw}" height="${H}"/>`);
  for (const b of holes) {
    const r = (b.d / 2) * s;
    ln(sx - 2.5, Y(b.y), sx + tw + 2.5, Y(b.y), "ctr");
    ln(sx, Y(b.y) - r, sx + tw, Y(b.y) - r, "hid"); ln(sx, Y(b.y) + r, sx + tw, Y(b.y) + r, "hid");
  }
  const yt = oyTop - 8;
  ln(sx, oyTop - 1.5, sx, yt - 2); ln(sx + tw, oyTop - 1.5, sx + tw, yt - 2);
  c.arr(sx - 7, yt, sx, yt); c.arr(sx + tw + 7, yt, sx + tw, yt);
  tx(sx + tw + 8, yt + 1.2, fmt(T), { anchor: "start" });
  return {
    scale: s, titel: `${m.name} ${fmt(L)} × ${fmt(B)} × ${fmt(T)}`, f2Label: "Dicke", f2: `t = ${fmt(T)} mm`,
    notes: ["Alle Kanten entgratet, Bohrungen gratfrei"],
  };
}

/* ---------------- Biegeteil ---------------- */
// k-Faktor nach DIN 6935 (oder fest aus den Einstellungen)
function szK(ri, t, st) {
  const ov = num(st.kfaktor, NaN);
  if (isFinite(ov) && ov > 0) return Math.min(1, ov);
  const q = ri / t;
  if (q >= 5) return 1;
  if (q <= 0) return 0.5;
  return Math.max(0.5, Math.min(1, 0.65 + 0.5 * Math.log10(q)));
}
// Abwicklung + Geometrie
function szBendCalc(m, st) {
  const t = m.dicke, ri = m.ri, k = szK(ri, t, st), n = m.schenkel.length;
  const B = m.biegungen.map((b) => {
    const phi = ((180 - b.winkel) * Math.PI) / 180;
    return { ...b, phi, sgn: b.richtung === "rechts" ? -1 : 1, tanh: Math.tan(phi / 2),
      td: (ri + t) * (b.winkel >= 90 ? Math.tan(phi / 2) : 1), arc: phi * (ri + (t / 2) * k) };
  });
  const ends = (i) => [i > 0 ? B[i - 1] : null, i < n - 1 ? B[i] : null].filter(Boolean);
  const given = m.schenkel.map((g) => g.l);
  const outer = given.map((l, i) => (m.massart === "innen" ? l + ends(i).reduce((a, b) => a + t * b.tanh, 0) : l));
  const mid = outer.map((l, i) => l - ends(i).reduce((a, b) => a + (t / 2) * b.tanh, 0));
  let pos = 0;
  const legFlat = [], bendPos = [];
  outer.forEach((l, i) => {
    const tdS = i > 0 ? B[i - 1].td : 0, tdE = i < n - 1 ? B[i].td : 0;
    legFlat.push({ start: pos, tdS });
    pos += Math.max(0, l - tdS - tdE);
    if (i < n - 1) { bendPos.push(pos + B[i].arc / 2); pos += B[i].arc; }
  });
  // Mittellinie (virtuelle Ecken), mathematische Koordinaten
  const P = [[0, 0]], dirs = [];
  let th = 0;
  mid.forEach((l, i) => {
    const u = [Math.cos(th), Math.sin(th)];
    dirs.push(u);
    const p = P[P.length - 1];
    P.push([p[0] + u[0] * l, p[1] + u[1] * l]);
    if (i < n - 1) th += B[i].sgn * B[i].phi;
  });
  return { t, ri, k, n, B, given, outer, mid, Lf: pos, legFlat, bendPos, P, dirs };
}

function szDrawBiege(c, m, st) {
  const { add, ln, tx, arr } = c;
  const g = szBendCalc(m, st);
  const { t, ri, n, B, P, dirs } = g;
  const r1 = (v) => fmt(Math.round(v * 10) / 10);
  const xs = P.map((p) => p[0]), ys = P.map((p) => p[1]);
  const minx = Math.min(...xs) - t, maxx = Math.max(...xs) + t, miny = Math.min(...ys) - t, maxy = Math.max(...ys) + t;
  const Wp = maxx - minx, Hp = maxy - miny;
  const holesF = m.bohrungen.map((h) => {
    const i = Math.min(n - 1, h.schenkel), lf = g.legFlat[i];
    return { x: lf.start + h.a - lf.tdS, y: h.y, d: h.d };
  });
  const T = szFlatTiers(g.Lf, m.breite, holesF);
  const fit = (k) => Wp * k + 30 + 34 + g.Lf * k + T.yTiers * 8 + 16 <= 238 && Hp * k + 26 <= 104 && m.breite * k + 12 + T.xTiers * 8 <= 95;
  const s = szPickScale(fit, st);
  // Profil darf größer dargestellt werden, wenn die Abwicklung den Maßstab bestimmt
  let sp = s;
  if (String(st.massstab || "auto") === "auto") {
    const c2 = SZ_SCALES.find((k) => k <= s * 5 && Wp * k + 30 <= 105 && Hp * k + 26 <= 104 && Wp * k + 34 + g.Lf * s + T.yTiers * 8 + 16 <= 238);
    if (c2 && c2 > s) sp = c2;
  }
  const tw = t * sp;
  // Profil
  const totalW = Wp * sp + 34 + g.Lf * s + T.yTiers * 8 + 16;
  const px0 = 42 + Math.max(0, (238 - 30 - totalW) / 2), py0 = 30 + Math.max(0, (100 - Hp * sp) / 2);
  const PX = (x) => px0 + (x - minx) * sp, PY = (y) => py0 + (maxy - y) * sp;
  const pp = (p) => [PX(p[0]), PY(p[1])];
  const pv = (u) => [u[0], -u[1]]; // Richtung in Blattkoordinaten
  const rm = ri + t / 2;
  let d = `M${PX(P[0][0])},${PY(P[0][1])}`;
  for (let j = 1; j < n; j++) {
    const b = B[j - 1], q = P[j], ui = dirs[j - 1], uo = dirs[j];
    const td = Math.min(rm * Math.tan(b.phi / 2), g.mid[j - 1] * 0.98, g.mid[j] * 0.98);
    const a1 = [q[0] - ui[0] * td, q[1] - ui[1] * td], a2 = [q[0] + uo[0] * td, q[1] + uo[1] * td];
    const rr = td / Math.max(1e-9, Math.tan(b.phi / 2));
    d += ` L${PX(a1[0])},${PY(a1[1])}`;
    d += rr * sp > 0.05 ? ` A${rr * sp},${rr * sp} 0 0 ${b.sgn > 0 ? 0 : 1} ${PX(a2[0])},${PY(a2[1])}` : ` L${PX(a2[0])},${PY(a2[1])}`;
  }
  d += ` L${PX(P[n][0])},${PY(P[n][1])}`;
  if (tw >= 0.6) {
    add(`<path d="${d}" fill="none" stroke="#000" stroke-width="${tw + 1}" stroke-linejoin="round"/>`);
    add(`<path d="${d}" fill="none" stroke="#fff" stroke-width="${tw}" stroke-linejoin="round"/>`);
    [[P[0], dirs[0]], [P[n], dirs[n - 1]]].forEach(([p, u]) => {
      const [x, y] = pp(p), v = pv(u), nn = [-v[1], v[0]], h = tw / 2 + 0.5;
      ln(x - nn[0] * h, y - nn[1] * h, x + nn[0] * h, y + nn[1] * h, "thick");
    });
  } else add(`<path d="${d}" fill="none" stroke="#000" stroke-width="0.6" stroke-linejoin="round"/>`);
  // Bohrungen im Profil (Mittellinie + verdeckte Kanten)
  m.bohrungen.forEach((h) => {
    const i = Math.min(n - 1, h.schenkel), u = dirs[i];
    const sh = i > 0 ? (t / 2) * B[i - 1].tanh * (m.massart === "innen" ? -1 : 1) : 0;
    const pm = [P[i][0] + u[0] * (h.a - sh), P[i][1] + u[1] * (h.a - sh)];
    const [x, y] = pp(pm), v = pv(u), nn = [-v[1], v[0]];
    const e = tw / 2 + 2.5;
    ln(x - nn[0] * e, y - nn[1] * e, x + nn[0] * e, y + nn[1] * e, "ctr");
    if (h.d * sp > 0.8) [-1, 1].forEach((sg) => {
      const cx = x + v[0] * sg * (h.d / 2) * sp, cy = y + v[1] * sg * (h.d / 2) * sp;
      ln(cx - nn[0] * tw / 2, cy - nn[1] * tw / 2, cx + nn[0] * tw / 2, cy + nn[1] * tw / 2, "hid");
    });
  });
  // Schenkelmaße (Außen- bzw. Innenmaße bis zur virtuellen Ecke)
  const sgnRef = m.massart === "innen" ? -1 : 1;
  for (let i = 0; i < n; i++) {
    const u = dirs[i];
    const sS = i > 0 ? (t / 2) * B[i - 1].tanh * sgnRef : 0, sE = i < n - 1 ? (t / 2) * B[i].tanh * sgnRef : 0;
    const S = [P[i][0] - u[0] * sS, P[i][1] - u[1] * sS], E = [P[i + 1][0] + u[0] * sE, P[i + 1][1] + u[1] * sE];
    const ref = i > 0 ? B[i - 1] : B[0];
    const nm = ref.sgn > 0 ? [u[1], -u[0]] : [-u[1], u[0]]; // konvexe Seite
    const A1 = pp(S), A2 = pp(E), np = pv(nm), v = pv(u);
    const o0 = tw / 2 + 1.5, o1 = tw / 2 + 8;
    [A1, A2].forEach((p) => ln(p[0] + np[0] * o0, p[1] + np[1] * o0, p[0] + np[0] * (o1 + 2), p[1] + np[1] * (o1 + 2)));
    const D1 = [A1[0] + np[0] * o1, A1[1] + np[1] * o1], D2 = [A2[0] + np[0] * o1, A2[1] + np[1] * o1];
    const len = Math.hypot(D2[0] - D1[0], D2[1] - D1[1]);
    const label = fmt(g.given[i]);
    let ang = (Math.atan2(v[1], v[0]) * 180) / Math.PI;
    if (ang > 90.01 || ang < -90.01) ang += 180;
    if (len >= label.length * 2.1 + 3) {
      c.dimLine(D1[0], D1[1], D2[0], D2[1]);
      const tx0 = (D1[0] + D2[0]) / 2 + np[0] * 2.2, ty0 = (D1[1] + D2[1]) / 2 + np[1] * 2.2;
      tx(tx0, ty0, label, { rot: ang || 0.0001, mid: true });
    } else {
      arr(D1[0] - v[0] * 5, D1[1] - v[1] * 5, D1[0], D1[1]); arr(D2[0] + v[0] * 5, D2[1] + v[1] * 5, D2[0], D2[1]); ln(D1[0], D1[1], D2[0], D2[1]);
      tx(D2[0] + v[0] * 9 + np[0] * 2.2, D2[1] + v[1] * 9 + np[1] * 2.2, label, { rot: ang || 0.0001, mid: true });
    }
  }
  // Winkel ≠ 90°
  B.forEach((b, j) => {
    if (Math.abs(b.winkel - 90) < 0.01) return;
    const ui = pv(dirs[j]), uo = pv(dirs[j + 1]);
    const a1 = [-ui[0], -ui[1]], a2 = uo;
    let bis = [a1[0] + a2[0], a1[1] + a2[1]];
    const bl = Math.hypot(bis[0], bis[1]) || 1; bis = [bis[0] / bl, bis[1] / bl];
    const q = pp(P[j + 1]), dist = tw / 2 / Math.max(0.05, Math.sin(((b.winkel / 2) * Math.PI) / 180));
    const Q = [q[0] + bis[0] * dist, q[1] + bis[1] * dist], R = 7;
    const cr = a1[0] * a2[1] - a1[1] * a2[0];
    add(`<path class="thin" d="M${Q[0] + a1[0] * R},${Q[1] + a1[1] * R} A${R},${R} 0 0 ${cr > 0 ? 1 : 0} ${Q[0] + a2[0] * R},${Q[1] + a2[1] * R}"/>`);
    tx(Q[0] + bis[0] * (R + 4), Q[1] + bis[1] * (R + 4), `${fmt(b.winkel)}°`, { mid: true, size: 3 });
  });
  // Innenradius
  if (n > 1) {
    const b = B[0], q = pp(P[1]), ui = pv(dirs[0]), uo = pv(dirs[1]);
    let bis = [-ui[0] + uo[0], -ui[1] + uo[1]];
    const bl = Math.hypot(bis[0], bis[1]) || 1; bis = [bis[0] / bl, bis[1] / bl];
    const dc = (rm / Math.max(0.05, Math.cos(b.phi / 2))) * sp; // Abstand Ecke → Biegemittelpunkt
    const C = [q[0] + bis[0] * dc, q[1] + bis[1] * dc];
    const P1 = [C[0] - bis[0] * ri * sp, C[1] - bis[1] * ri * sp];
    const off = Math.abs(b.winkel - 90) < 0.01 ? 10 : 20;
    const L2 = [C[0] + bis[0] * off, C[1] + bis[1] * off];
    if (ri * sp > 0.3) arr(L2[0], L2[1], P1[0], P1[1]); else arr(L2[0], L2[1], q[0], q[1]);
    tx(L2[0] + (bis[0] >= 0 ? 1 : -1), L2[1] + bis[1] * 2, `Ri ${fmt(ri)}`, { anchor: bis[0] >= 0 ? "start" : "end", mid: true, size: 3 });
  }
  // Abwicklung
  if (sp !== s) tx(px0, py0 - 14, `Profil M ${szScaleLabel(sp)}`, { anchor: "start", size: 2.8 });
  const fox = px0 + Wp * sp + 34, foy = 46 + Math.max(0, (95 - 12 - m.breite * s - T.xTiers * 8) / 2);
  tx(fox, foy - 26, "Abwicklung", { anchor: "start", weight: "bold", size: 3.2 });
  szFlatCore(c, { L: g.Lf, B: m.breite, holes: holesF }, s, fox, foy, {
    lLabel: `${r1(g.Lf)}`,
    bends: g.bendPos.map((x, j) => ({ x, label: `B${j + 1} ${B[j].sgn > 0 ? "↑" : "↓"} ${fmt(B[j].winkel)}°` })),
  });
  return {
    scale: s, titel: `${m.name} ${g.given.map(fmt).join(" × ")} × B${fmt(m.breite)}`, f2Label: "Dicke", f2: `t = ${fmt(t)} mm`,
    notes: [
      `Alle Biegeradien innen R${fmt(ri)}; Schenkelmaße sind ${m.massart === "innen" ? "Innenmaße" : "Außenmaße"}`,
      `Abwicklung nach DIN 6935 (k = ${fmt(Math.round(g.k * 100) / 100)}), gestreckte Länge ${r1(g.Lf)} mm – Richtwert`,
      "Biegerichtung: ↑ = zum Betrachter, ↓ = vom Betrachter weg",
      "Alle Kanten entgratet",
    ],
  };
}

/* ---------------- Frästeil (Quader mit Nuten und Bohrungen) ---------------- */
function szDrawFraes(c, m, st) {
  const { add, ln, tx, hDim, vDim, arr } = c;
  const L = m.laenge, B = m.breite, H = m.hoehe;
  const NU = m.nuten, BO = m.fbohrungen;
  const s = szPickScale((k) => (L + B) * k <= 165 && (H + B) * k <= 70, st);
  const W = L * s;
  const ox = 52 + Math.max(0, (170 - (L + B) * s) / 2);
  const oy = 30 + Math.max(0, (70 - (H + B) * s) / 2);
  const MX = (x) => ox + x * s, MZ = (z) => oy + (H - z) * s;           // Vorderansicht (x, z)
  const ty0 = oy + H * s + 26, TY = (y) => ty0 + (B - y) * s;           // Draufsicht (x, y) – unter der Vorderansicht
  const ex0 = ox + W + 30, EY = (y) => ex0 + (B - y) * s;               // Seitenansicht von links (y, z) – rechts daneben
  const r1 = (v) => fmt(Math.round(v * 10) / 10);
  const rect = (x1, y1, x2, y2, cls) => add(`<rect class="${cls}" x="${Math.min(x1, x2)}" y="${Math.min(y1, y2)}" width="${Math.abs(x2 - x1)}" height="${Math.abs(y2 - y1)}"/>`);
  const nutYZ = (n) => n.seite === "unten" ? [n.abstand, n.abstand + n.breite, 0, n.tiefe]
    : n.seite === "oben" ? [n.abstand, n.abstand + n.breite, H - n.tiefe, H]
    : n.seite === "vorne" ? [0, n.tiefe, n.abstand, n.abstand + n.breite]
    : [B - n.tiefe, B, n.abstand, n.abstand + n.breite];

  // ---- Vorderansicht
  rect(MX(0), MZ(0), MX(L), MZ(H), "thick");
  NU.forEach((n) => {
    if (n.seite === "unten") ln(MX(0), MZ(n.tiefe), MX(L), MZ(n.tiefe), "hid");
    if (n.seite === "oben") ln(MX(0), MZ(H - n.tiefe), MX(L), MZ(H - n.tiefe), "hid");
    if (n.seite === "vorne") [n.abstand, n.abstand + n.breite].forEach((z) => ln(MX(0), MZ(z), MX(L), MZ(z), "thick"));
    if (n.seite === "hinten") [n.abstand, n.abstand + n.breite].forEach((z) => ln(MX(0), MZ(z), MX(L), MZ(z), "hid"));
  });
  // ---- Draufsicht
  rect(MX(0), TY(0), MX(L), TY(B), "thick");
  NU.forEach((n) => {
    if (n.seite === "oben") [n.abstand, n.abstand + n.breite].forEach((y) => ln(MX(0), TY(y), MX(L), TY(y), "thick"));
    if (n.seite === "unten") [n.abstand, n.abstand + n.breite].forEach((y) => ln(MX(0), TY(y), MX(L), TY(y), "hid"));
    if (n.seite === "vorne") ln(MX(0), TY(n.tiefe), MX(L), TY(n.tiefe), "hid");
    if (n.seite === "hinten") ln(MX(0), TY(B - n.tiefe), MX(L), TY(B - n.tiefe), "hid");
  });
  // ---- Seitenansicht (Profil mit Nuten)
  rect(EY(B), MZ(H), EY(0), MZ(0), "thick");
  NU.forEach((n) => {
    const [y1, y2, z1, z2] = nutYZ(n);
    const X1 = Math.min(EY(y1), EY(y2)), X2 = Math.max(EY(y1), EY(y2)), Y1 = MZ(z2), Y2 = MZ(z1);
    add(`<rect class="thick" x="${X1}" y="${Y1}" width="${X2 - X1}" height="${Y2 - Y1}" style="fill:#fff"/>`);
    const wl = (a1, b1, a2, b2) => add(`<line x1="${a1}" y1="${b1}" x2="${a2}" y2="${b2}" stroke="#fff" stroke-width="0.8"/>`);
    const e = 0.01;
    if (z1 <= e) wl(X1 + 0.25, Y2, X2 - 0.25, Y2);            // liegt an der Unterseite
    if (z2 >= H - e) wl(X1 + 0.25, Y1, X2 - 0.25, Y1);        // Oberseite
    if (Math.min(y1, y2) <= e) wl(X2, Y1 + 0.25, X2, Y2 - 0.25); // Vorderseite (rechts in der Seitenansicht)
    if (Math.max(y1, y2) >= B - e) wl(X1, Y1 + 0.25, X1, Y2 - 0.25); // Rückseite
  });

  // ---- Bohrungen
  const dn = (b) => { const g = (b.gewinde || "").match(/M\s*(\d+(?:[.,]\d+)?)/i); return g ? num(g[1], b.d) : b.d; };
  const circ = (cx, cy, b) => {
    const r = (dn(b) / 2) * s, e = r + 2;
    if (b.gewinde) { add(`<circle class="thick" cx="${cx}" cy="${cy}" r="${r * 0.85}"/>`); add(`<circle class="thin" cx="${cx}" cy="${cy}" r="${r}"/>`); }
    else add(`<circle class="thick" cx="${cx}" cy="${cy}" r="${r}"/>`);
    ln(cx - e, cy, cx + e, cy, "ctr"); ln(cx, cy - e, cx, cy + e, "ctr");
  };
  // verdeckte Bohrung als zwei Linien + Mittellinie; (p1,p2) = Achse in Blattkoordinaten, r in mm Blatt
  const hidden = (x1, y1, x2, y2, r) => {
    const L2 = Math.hypot(x2 - x1, y2 - y1) || 1, nx = -(y2 - y1) / L2, ny = (x2 - x1) / L2;
    ln(x1 + nx * r, y1 + ny * r, x2 + nx * r, y2 + ny * r, "hid"); ln(x1 - nx * r, y1 - ny * r, x2 - nx * r, y2 - ny * r, "hid");
    ln(x1 - (x2 - x1) / L2 * 2, y1 - (y2 - y1) / L2 * 2, x2 + (x2 - x1) / L2 * 2, y2 + (y2 - y1) / L2 * 2, "ctr");
  };
  BO.forEach((b) => {
    const r = (dn(b) / 2) * s;
    if (b.richtung === "z") {            // von oben: a = x, b = y
      const zEnd = b.tiefe > 0 ? Math.max(0, H - b.tiefe) : 0;
      circ(MX(b.a), TY(b.b), b);
      hidden(MX(b.a), MZ(H), MX(b.a), MZ(zEnd), r);
      hidden(EY(b.b), MZ(H), EY(b.b), MZ(zEnd), r);
    } else if (b.richtung === "y") {     // von vorne: a = x, b = z
      const yEnd = b.tiefe > 0 ? Math.min(B, b.tiefe) : B;
      circ(MX(b.a), MZ(b.b), b);
      hidden(MX(b.a), TY(0), MX(b.a), TY(yEnd), r);
      hidden(EY(0), MZ(b.b), EY(yEnd), MZ(b.b), r);
    } else {                             // von links (Stirnseite): a = y, b = z
      const xEnd = b.tiefe > 0 ? Math.min(L, b.tiefe) : L;
      circ(EY(b.a), MZ(b.b), b);
      hidden(MX(0), MZ(b.b), MX(xEnd), MZ(b.b), r);
      hidden(MX(0), TY(b.a), MX(xEnd), TY(b.a), r);
    }
  });

  // ---- Bemaßung Vorderansicht
  const yL = oy - 10;
  ln(MX(0), oy - 1.5, MX(0), yL - 2); ln(MX(L), oy - 1.5, MX(L), yL - 2);
  hDim(MX(0), MX(L), yL, r1(L));
  const xH = ox - 10;
  ln(ox - 1.5, MZ(0), xH - 2, MZ(0)); ln(ox - 1.5, MZ(H), xH - 2, MZ(H));
  vDim(xH, MZ(0), MZ(H), r1(H));
  // Bohrungen von vorne: x unten (Richtung Draufsicht), z links
  const uniq = (a) => [...new Set(a.map((v) => Math.round(v * 100) / 100))].sort((p, q) => p - q);
  const by = BO.filter((b) => b.richtung === "y");
  uniq(by.map((b) => b.a)).forEach((x, i) => {
    const yl = MZ(0) + 8 + i * 6;
    ln(MX(x), MZ(0) + 1.5, MX(x), yl + 2); ln(MX(0), MZ(0) + 1.5, MX(0), yl + 2);
    hDim(MX(0), MX(x), yl, r1(x), 1);
  });
  uniq(by.map((b) => b.b)).forEach((z, i) => {
    const xl = ox - 18 - i * 7;
    ln(MX(Math.min(...by.filter((b) => Math.abs(b.b - z) < 0.01).map((b) => b.a))) - 3, MZ(z), xl - 2, MZ(z));
    ln(ox - 11.5, MZ(0), xl - 2, MZ(0));
    vDim(xl, MZ(0), MZ(z), r1(z), -1);
  });
  // ---- Bemaßung Seitenansicht: Breite, Nuten
  const zb = MZ(0) + 10;
  ln(EY(0), MZ(0) + 1.5, EY(0), zb + 2); ln(EY(B), MZ(0) + 1.5, EY(B), zb + 2);
  hDim(EY(B), EY(0), zb, r1(B));
  let tierR = 0, tierT = 0, tierB = 0, tierU = 1;
  NU.forEach((n) => {
    const [y1, y2, z1, z2] = nutYZ(n);
    if (n.seite === "unten" || n.seite === "oben") {
      const yl = n.seite === "unten" ? MZ(0) + 4 + 0 * tierB : MZ(H) - 8 - 7 * tierT;
      const pts = uniq([0, y1, y2, B]);
      const ze = n.seite === "unten" ? MZ(0) : MZ(H);
      if (n.seite === "unten") {
        // Kette über der Breitenangabe
        const yk = MZ(0) + 4.5 + 0 * yl;
        pts.forEach((y) => ln(EY(y), ze + 0.8, EY(y), yk + 1));
        for (let i = 0; i < pts.length - 1; i++) hDim(EY(pts[i + 1]), EY(pts[i]), yk, r1(pts[i + 1] - pts[i]), i === 0 ? 1 : -1);
        tierB++;
      } else {
        pts.forEach((y) => ln(EY(y), ze - 1.5, EY(y), yl - 2));
        for (let i = 0; i < pts.length - 1; i++) hDim(EY(pts[i + 1]), EY(pts[i]), yl, r1(pts[i + 1] - pts[i]), i === 0 ? 1 : -1);
        tierT++;
      }
      const xl = EY(0) + 8 + 7 * tierR;
      const za = n.seite === "unten" ? z2 : z1, zbE = n.seite === "unten" ? 0 : H;
      ln(EY(y1) + 1, MZ(za), xl + 2, MZ(za)); ln(EY(0) + 1.5, MZ(zbE), xl + 2, MZ(zbE));
      vDim(xl, MZ(zbE), MZ(za), r1(n.tiefe), n.seite === "unten" ? -1 : 1);
      tierR++;
    } else {
      const xl = EY(0) + 8 + 7 * tierR;
      const pts = uniq([0, z1, z2, H]);
      pts.forEach((z) => ln(EY(0) + 1.5, MZ(z), xl + 2, MZ(z)));
      for (let i = 0; i < pts.length - 1; i++) vDim(xl, MZ(pts[i]), MZ(pts[i + 1]), r1(pts[i + 1] - pts[i]), i === 0 ? 1 : -1);
      tierR++;
      const yl = zb + 8 * tierU;
      const ya = n.seite === "vorne" ? 0 : B, yb2 = n.seite === "vorne" ? n.tiefe : B - n.tiefe;
      ln(EY(yb2), MZ(z1) + 1, EY(yb2), yl + 2);
      ln(EY(ya), zb + 2, EY(ya), yl + 2);
      hDim(EY(Math.max(ya, yb2)), EY(Math.min(ya, yb2)), yl, r1(n.tiefe), n.seite === "vorne" ? -1 : 1);
      tierU++;
    }
  });
  // Stirnbohrungen in der Seitenansicht bemaßen
  BO.filter((b) => b.richtung === "x").forEach((b, i) => {
    const xl = EY(0) + 8 + 7 * (tierR + i);
    ln(EY(b.a) + 2, MZ(b.b), xl + 2, MZ(b.b)); ln(EY(0) + 1.5, MZ(0), xl + 2, MZ(0));
    vDim(xl, MZ(0), MZ(b.b), r1(b.b), -1);
    const yl = zb + 8 * (tierU + i);
    ln(EY(b.a), MZ(b.b) + 2, EY(b.a), yl + 2); ln(EY(0), zb + 2, EY(0), yl + 2);
    hDim(EY(b.a), EY(0), yl, r1(b.a), -1);
  });
  // ---- Bemaßung Draufsicht: Bohrungen von oben
  const bz = BO.filter((b) => b.richtung === "z");
  uniq(bz.map((b) => b.a)).forEach((x, i) => {
    const yl = TY(0) + 10 + i * 7;
    ln(MX(0), TY(0) + 1.5, MX(0), yl + 2); ln(MX(x), TY(0) + 1.5, MX(x), yl + 2);
    hDim(MX(0), MX(x), yl, r1(x), 1);
  });
  uniq(bz.map((b) => b.b)).forEach((y, i) => {
    const xl = MX(L) + 8 + i * 7;
    ln(MX(L) + 1.5, TY(0), xl + 2, TY(0)); ln(MX(Math.max(...bz.filter((b) => Math.abs(b.b - y) < 0.01).map((b) => b.a))) + 3, TY(y), xl + 2, TY(y));
    vDim(xl, TY(0), TY(y), r1(y), -1);
  });
  // Draufsicht Breite links
  const xB = ox - 10;
  ln(ox - 1.5, TY(0), xB - 2, TY(0)); ln(ox - 1.5, TY(B), xB - 2, TY(B));
  vDim(xB, TY(0), TY(B), r1(B));
  // ---- Bohrungsangaben (Hinweislinie an der Ansicht, in der die Bohrung als Kreis erscheint)
  const groups = {};
  BO.forEach((b) => { const k = `${b.richtung}|${b.gewinde || fmt(b.d)}|${b.tiefe}`; (groups[k] = groups[k] || []).push(b); });
  let gi = 0;
  Object.values(groups).forEach((hs) => {
    const b = hs[0], r = (dn(b) / 2) * s;
    const [cx, cy] = b.richtung === "z" ? [MX(b.a), TY(b.b)] : b.richtung === "y" ? [MX(b.a), MZ(b.b)] : [EY(b.a), MZ(b.b)];
    const ex = cx + 12 + gi * 3, ey = cy - 11 - gi * 4;
    const lab = `${hs.length > 1 ? hs.length + "× " : ""}${b.gewinde || "Ø" + fmt(b.d)}${b.tiefe > 0 ? " ↧" + fmt(b.tiefe) : ""}`;
    arr(ex, ey, cx + r * 0.707, cy - r * 0.707); ln(ex, ey, ex + lab.length * 2.1, ey);
    tx(ex + 0.5, ey - 1, lab, { anchor: "start" });
    gi++;
  });
  // Ansichtsbezeichnungen
  tx(MX(0), MZ(H) - 18, "Vorderansicht", { anchor: "start", size: 2.6, fill: "#555" });
  tx(MX(0), TY(B) - 3, "Draufsicht", { anchor: "start", size: 2.6, fill: "#555" });
  tx(EY(B), MZ(H) - 18, "Seitenansicht", { anchor: "start", size: 2.6, fill: "#555" });
  return {
    scale: s, titel: `${m.name} ${fmt(L)} × ${fmt(B)} × ${fmt(H)}`, f2Label: "Rohmaß", f2: `${fmt(L)}×${fmt(B)}×${fmt(H)}`,
    notes: ["Alle Kanten gebrochen 0,2–0,5 mm", "Innenecken der Nuten mit Fräserradius zulässig", "Darstellung nach Projektionsmethode 1"],
  };
}

/* ---------------- Fertigbarkeit Biegeteil (Faustregeln) ---------------- */
function szBendChecks(m, st) {
  const g = szBendCalc(m, st), t = g.t, out = [];
  const bmin = 4 * t + g.ri;
  g.outer.forEach((l, i) => {
    const isEnd = i === 0 || i === g.n - 1;
    const lim = isEnd ? bmin : 2 * bmin;
    if (l < lim) out.push(`Schenkel ${i + 1} (${fmt(l)} mm) ist für t = ${fmt(t)} mm sehr kurz – zum Abkanten etwa ≥ ${fmt(Math.round(lim))} mm nötig (Faustregel).`);
  });
  for (let i = 1; i < g.n - 1; i++) {
    const b1 = g.B[i - 1], b2 = g.B[i];
    if (b1.sgn === b2.sgn && b1.winkel <= 100 && b2.winkel <= 100) {
      const inner = g.outer[i] - 2 * t;
      const hMax = Math.max(g.outer[i - 1], g.outer[i + 1]);
      if (inner < Math.max(3 * t, hMax * 0.6)) out.push(`U-Form an Schenkel ${i + 1}: Innenweite ≈ ${fmt(Math.round(inner * 10) / 10)} mm bei ${fmt(hMax)} mm hohen Schenkeln – passt evtl. nicht über das Oberwerkzeug.`);
    }
  }
  if (t > 8) out.push(`Blechdicke ${fmt(t)} mm – Kanten nur mit großer Abkantpresse möglich.`);
  if (g.ri < t * 0.5 && t > 3) out.push(`Innenradius ${fmt(g.ri)} mm ist bei t = ${fmt(t)} mm sehr klein – Rissgefahr, üblich ≥ 1 × t.`);
  return out;
}

/* ---------------- Drehteil (Halbschnitt) ---------------- */
function szDrawDreh(c, m, st) {
  const { add, ln, tx, hDim, arr } = c;
  const A = m.aussen, lk = m.lochkreis;
  const xs = [0];
  A.forEach((g) => xs.push(xs[xs.length - 1] + g.l));
  const L = xs[xs.length - 1], Dmax = Math.max(...A.map((g) => g.d)), Rmax = Dmax / 2;
  const s = szPickScale((k) => L * k + Dmax * k <= 172 && Dmax * k <= 84, st);
  const W = L * s, Dp = Dmax * s;
  const used = W + 30 + Dp;
  const ox = Math.max(40, 30 + (215 - used) / 2);
  const ay = 34 + (84 - Dp) / 2 + Dp / 2;
  const X = (x) => ox + x * s, Yu = (r) => ay - r * s, Yl = (r) => ay + r * s;
  const fl = Math.min(m.fase_links, A[0].l, (A[0].d / 2) * 0.8);
  const fr = Math.min(m.fase_rechts, A[A.length - 1].l, (A[A.length - 1].d / 2) * 0.8);

  // Außenkontur (x, r) von links nach rechts
  const op = [];
  A.forEach((g, i) => {
    const r = g.d / 2, x0 = xs[i], x1 = xs[i + 1];
    if (i === 0 && fl > 0) { op.push([x0, r - fl]); op.push([x0 + fl, r]); } else op.push([x0, r]);
    if (i === A.length - 1 && fr > 0) { op.push([x1 - fr, r]); op.push([x1, r - fr]); } else op.push([x1, r]);
  });
  // Innenkontur
  const ins = [];
  let xi = 0;
  m.innen.forEach((g) => { ins.push({ x0: xi, x1: xi + g.l, r: g.d / 2, g }); xi += g.l; });
  if (xi < L - 1e-9) ins.push({ x0: xi, x1: L, r: 0, g: null });
  const ip = [];
  ins.forEach((q) => { ip.push([q.x0, q.r]); ip.push([q.x1, q.r]); });
  const hasBore = ins.some((q) => q.r > 0);

  // obere Hälfte: Schnitt
  const pts = [...op.map(([x, r]) => `${X(x)},${Yu(r)}`), ...ip.slice().reverse().map(([x, r]) => `${X(x)},${Yu(r)}`)];
  add(`<polygon points="${pts.join(" ")}" fill="url(#h)" class="thick" style="fill:url(#h)"/>`);
  // untere Hälfte: Ansicht
  add(`<polyline class="thick" points="${[[0, 0], ...op, [L, 0]].map(([x, r]) => `${X(x)},${Yl(r)}`).join(" ")}"/>`);
  for (let i = 1; i < A.length; i++) ln(X(xs[i]), ay, X(xs[i]), Yl(Math.max(A[i - 1].d, A[i].d) / 2), "thick");
  if (fl > 0) ln(X(fl), ay, X(fl), Yl(A[0].d / 2), "thick");
  if (fr > 0) ln(X(L - fr), ay, X(L - fr), Yl(A[A.length - 1].d / 2), "thick");
  // Gewinde (Kerndurchmesser dünn)
  A.forEach((g, i) => {
    if (!g.gewinde) return;
    const rc = (g.d / 2) * 0.85, x0 = xs[i] + (i === 0 ? fl : 0), x1 = xs[i + 1] - (i === A.length - 1 ? fr : 0);
    ln(X(x0), Yl(rc), X(x1), Yl(rc)); ln(X(x0), Yu(rc), X(x1), Yu(rc));
  });
  // Mittellinie
  ln(X(0) - 4, ay, X(L) + 4, ay, "ctr");

  // Außendurchmesser
  A.forEach((g, i) => {
    const xm = X(xs[i] + g.l * (hasBore ? 0.32 : 0.5)), r = g.d / 2;
    const label = g.gewinde ? g.gewinde : `Ø${fmt(g.d)}${g.tol ? " " + g.tol : ""}`;
    c.dimLine(xm, Yl(r), xm, Yu(r));
    tx(xm - 1.2, (ay + Yl(r)) / 2, label, { rot: -90 });
  });
  // Innendurchmesser (Halbmaß)
  ins.forEach((q) => {
    if (q.r <= 0) return;
    const xm = X(q.x0 + (q.x1 - q.x0) * 0.68);
    arr(xm, ay + 5, xm, Yu(q.r));
    tx(xm - 1.2, (ay + Yu(q.r)) / 2, `Ø${fmt(q.r * 2)}${q.g && q.g.tol ? " " + q.g.tol : ""}`, { rot: -90 });
  });
  // Längen unten (Absätze + Gesamt)
  const yb = Yl(Rmax) + 10;
  const extDown = (x, r, y) => ln(X(x), Yl(r) + 1.5, X(x), y + 2);
  for (let i = 0; i <= A.length; i++) {
    const r = Math.max(i > 0 ? A[i - 1].d / 2 : 0, i < A.length ? A[i].d / 2 : 0);
    extDown(xs[i], r - (i === 0 ? fl : 0) - (i === A.length ? fr : 0), A.length > 1 ? yb + 8 : yb);
  }
  if (A.length > 1) {
    A.forEach((g, i) => hDim(X(xs[i]), X(xs[i + 1]), yb, fmt(g.l), i === 0 ? -1 : 1));
    hDim(X(0), X(L), yb + 8, fmt(L));
  } else hDim(X(0), X(L), yb, fmt(L));
  // Bohrungstiefen oben
  if (hasBore && (ins.length > 1)) {
    const ya = Yu(Rmax) - 10;
    const rOut = (x) => { let r = 0; A.forEach((g, i) => { if (x >= xs[i] - 1e-9 && x <= xs[i + 1] + 1e-9) r = Math.max(r, g.d / 2); }); return r; };
    ins.forEach((q, i) => {
      if (i === 0) ln(X(q.x0), Yu(rOut(q.x0)) - 1.5, X(q.x0), ya - 2);
      ln(X(q.x1), Yu(i === ins.length - 1 ? rOut(q.x1) : Math.max(q.r, ins[i + 1].r)) - 1.5, X(q.x1), ya - 2);
      hDim(X(q.x0), X(q.x1), ya, fmt(q.x1 - q.x0), i === 0 ? -1 : 1);
    });
  }
  // Fasen
  if (fl > 0) {
    const px = X(fl / 2), py = Yu(A[0].d / 2 - fl / 2);
    arr(px - 8, py - 7, px, py); ln(px - 8, py - 7, px - 20, py - 7);
    tx(px - 9, py - 8, `${fmt(fl)}×45°`, { anchor: "end" });
  }
  if (fr > 0) {
    const px = X(L - fr / 2), py = Yu(A[A.length - 1].d / 2 - fr / 2);
    arr(px + 8, py - 7, px, py); ln(px + 8, py - 7, px + 20, py - 7);
    tx(px + 9, py - 8, `${fmt(fr)}×45°`, { anchor: "start" });
  }

  // Seitenansicht (von links, rechts angeordnet)
  const cx = X(L) + 26 + Rmax * s, cy = ay;
  let runMax = 0;
  A.forEach((g) => { if (g.d > runMax + 1e-9) { add(`<circle class="thick" cx="${cx}" cy="${cy}" r="${(g.d / 2) * s}"/>`); runMax = g.d; } });
  if (fl > 0) add(`<circle class="thick" cx="${cx}" cy="${cy}" r="${(A[0].d / 2 - fl) * s}"/>`);
  let runMin = Infinity;
  for (const q of ins) { if (q.r <= 0) break; if (q.r < runMin - 1e-9) { add(`<circle class="thick" cx="${cx}" cy="${cy}" r="${q.r * s}"/>`); runMin = q.r; } }
  const e = Rmax * s + 3;
  ln(cx - e, cy, cx + e, cy, "ctr"); ln(cx, cy - e, cx, cy + e, "ctr");
  if (lk.anzahl > 0 && lk.lk > 0) {
    const rl = (lk.lk / 2) * s;
    add(`<circle class="ctr" cx="${cx}" cy="${cy}" r="${rl}"/>`);
    const gm = (lk.gewinde || "").match(/M\s*(\d+(?:[.,]\d+)?)/i);
    const dn = gm ? num(gm[1], 10) : (lk.d || num(st.lochd, 6.6));
    for (let k = 0; k < lk.anzahl; k++) {
      const a = -Math.PI / 2 + (k * 2 * Math.PI) / lk.anzahl;
      const hx = cx + rl * Math.cos(a), hy = cy + rl * Math.sin(a);
      if (gm) { add(`<circle class="thick" cx="${hx}" cy="${hy}" r="${dn * 0.85 / 2 * s}"/>`); add(`<circle class="thin" cx="${hx}" cy="${hy}" r="${dn / 2 * s}"/>`); }
      else add(`<circle class="thick" cx="${hx}" cy="${hy}" r="${dn / 2 * s}"/>`);
    }
    const hx = cx, hy = cy - rl, r = (dn / 2) * s;
    const tx0 = cx + Rmax * s * 0.55 + 4, ty0 = cy - Rmax * s - 6;
    arr(tx0, ty0, hx + r * 0.707, hy - r * 0.707);
    const l1 = `${lk.anzahl}× ${lk.gewinde || "Ø" + fmt(dn)}${lk.tiefe ? " ↧" + fmt(lk.tiefe) : ""}`;
    const l2 = `auf LK Ø${fmt(lk.lk)}, gleichm. verteilt`;
    ln(tx0, ty0, tx0 + Math.max(l1.length, l2.length * 0.8) * 2.1, ty0);
    tx(tx0 + 0.5, ty0 - 1, l1, { anchor: "start" });
    tx(tx0 + 0.5, ty0 + 3.6, l2, { anchor: "start", size: 2.8 });
  }
  return {
    scale: s, titel: `${m.name} Ø${fmt(Dmax)} × ${fmt(L)}`, f2Label: "Rohmaß", f2: `Ø${fmt(Dmax)} × ${fmt(L)}`,
    notes: ["Nicht bemaßte Kanten gebrochen 0,5×45°"],
  };
}

const szDefMat = (typ, st) => (typ === "dreh" ? st.werkstoff_dreh : typ === "fraes" ? st.werkstoff_fraes : st.werkstoff);
const SZ_TYPNAME = { flach: "Flachteil", dreh: "Drehteil", biege: "Biegeteil", fraes: "Frästeil" };

/* ---------------- Blatt ---------------- */
function szRender(model, st = SZ_SET_DEFAULT) {
  const m = szNormalize(model, st);
  const c = szCtx();
  const { add, ln, tx } = c;
  add(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 297 210" width="297mm" height="210mm" font-family="DejaVu Sans, Arial, Helvetica, sans-serif">`);
  add(`<defs><marker id="a" viewBox="0 0 10 10" refX="10" refY="5" markerUnits="userSpaceOnUse" markerWidth="3" markerHeight="3" orient="auto-start-reverse"><path d="M0,1.5 L10,5 L0,8.5 z" fill="#000"/></marker>`
    + `<pattern id="h" patternUnits="userSpaceOnUse" width="2.2" height="2.2" patternTransform="rotate(45)"><rect width="2.2" height="2.2" fill="#fff"/><line x1="0" y1="0" x2="0" y2="2.2" stroke="#000" stroke-width=".35"/></pattern></defs>`);
  add(`<style>.thick{stroke:#000;stroke-width:.5;fill:none}.thin{stroke:#000;stroke-width:.25;fill:none}.ctr{stroke:#000;stroke-width:.2;fill:none;stroke-dasharray:6 1.5 1 1.5}.hid{stroke:#000;stroke-width:.25;fill:none;stroke-dasharray:2 1}.bl{stroke:#000;stroke-width:.25;fill:none;stroke-dasharray:8 1.5 1 1.5 1 1.5}text{fill:#000;paint-order:stroke;stroke:#fff;stroke-width:.9px;stroke-linejoin:round}</style>`);
  add(`<rect x="0" y="0" width="297" height="210" fill="#fff"/>`);
  add(`<rect class="thick" x="20" y="10" width="267" height="190"/>`);
  const info = m.typ === "dreh" ? szDrawDreh(c, m, st) : m.typ === "biege" ? szDrawBiege(c, m, st) : m.typ === "fraes" ? szDrawFraes(c, m, st) : szDrawFlach(c, m, st);
  const tol = m.toleranz || st.toleranz || "ISO 2768-m";

  // Hinweise
  const notes = [...info.notes, `Nicht tolerierte Maße nach ${tol}`, ...m.hinweise];
  tx(30, 156, "Hinweise:", { anchor: "start", weight: "bold" });
  notes.slice(0, 8).forEach((n, i) => tx(30, 162 + i * 4.6, `${i + 1}. ${n}`, { anchor: "start", size: 3 }));

  // Schriftfeld
  const bx = 177, by = 150, bw = 110, c1 = bx + 55, c2 = bx + 82;
  add(`<rect class="thick" x="${bx}" y="${by}" width="${bw}" height="50"/>`);
  [10, 20, 30, 40].forEach((r) => ln(bx, by + r, bx + bw, by + r));
  ln(c1, by + 10, c1, by + 50); ln(c2, by + 10, c2, by + 50);
  const fs =(v, w) => { const L = String(v).length; return L * 1.9 > w ? Math.max(2, w / (L * 0.6)) : 3.2; };
  const fld = (x, y, w, label, val) => {
    tx(x + 1.5, y + 2.8, label, { anchor: "start", size: 2, fill: "#555" });
    tx(x + 1.5, y + 8, val, { anchor: "start", size: fs(val, w - 3) });
  };
  const today = new Date().toLocaleDateString("de-DE");
  tx(bx + 1.5, by + 2.8, "Benennung", { anchor: "start", size: 2, fill: "#555" });
  tx(bx + 1.5, by + 8.3, info.titel, { anchor: "start", size: Math.min(4.2, fs(info.titel, 105) * 1.25), weight: "bold" });
  const mat = m.material || szDefMat(m.typ, st) || "–";
  fld(bx, by + 10, 55, "Werkstoff", mat);
  fld(c1, by + 10, 27, info.f2Label, info.f2);
  fld(c2, by + 10, 28, "Stückzahl", String(m.stueck));
  fld(bx, by + 20, 55, "Oberfläche", m.oberflaeche || "–");
  fld(c1, by + 20, 27, "Maßstab", szScaleLabel(info.scale));
  fld(c2, by + 20, 28, "Format", "A4");
  fld(bx, by + 30, 55, "Allgemeintoleranz", tol);
  fld(c1, by + 30, 27, "Datum", today);
  fld(c2, by + 30, 28, "Blatt", "1 / 1");
  fld(bx, by + 40, 55, "Firma", st.firma || "–");
  fld(c1, by + 40, 27, "Gezeichnet", st.zeichner || "–");
  fld(c2, by + 40, 28, "Zeichn.-Nr.", m.nr || "–");
  add(`</svg>`);
  return c.out.join("");
}

/* ---------------- KI-Auftrag ---------------- */
function szPrompt(st, hint = "") {
  return `Du bekommst das Foto einer handgezeichneten Bemaßungsskizze eines Bauteils. Bestimme zuerst den Bauteiltyp:
- "flach": flaches Teil (Blech, Platte, Lasche, Zuschnitt) mit Außenkontur und Bohrungen.
- "dreh": rotationssymmetrisches Drehteil (Welle, Bolzen, Buchse, Flansch, Scheibe) – erkennbar an Ø-Maßen, Mittellinie, Schnittschraffur.
- "biege": gekantetes Blech-Biegeteil (Winkel, U-, Z-, Hutprofil, Kantteil) – erkennbar an einem Profil aus Schenkeln mit Biegungen, Biegewinkeln oder Biegeradius.
- "fraes": prismatisches Frästeil aus dem Vollen (Block, Leiste, Klotz, Nutenstück, U-Stück mit dicken Wänden) – Quader mit Nuten, Absätzen oder Bohrungen, oft als 3D-Skizze gezeichnet.
${["flach", "dreh", "biege", "fraes"].includes(st.typ) ? `Der Typ ist vorgegeben: "${st.typ}".\n` : ""}Antworte NUR mit JSON (kein Text davor oder danach).

Format für "flach":
{"typ":"flach","name":"Lasche","material":"Stahlblech verzinkt","oberflaeche":"","dicke":2,"stueck":5,"laenge":150,"breite":40,"eckradius":0,
 "bohrungen":[{"x":30,"y":20,"d":null}],"hinweise":[],"unsicher":["Bohrungsdurchmesser nicht angegeben"]}
Regeln für "flach":
- Koordinatenursprung = linke untere Ecke, x nach rechts, y nach oben. laenge = waagerechtes, breite = senkrechtes Außenmaß.
- Bohrung vom rechten Rand bemaßt → x = laenge − Abstand; vom oberen Rand → y = breite − Abstand. Durchmesser fehlt → "d": null.
- "t=" ist die Dicke.

Format für "dreh":
{"typ":"dreh","name":"Flanschbuchse","material":"","oberflaeche":"","stueck":1,"gesamtlaenge":97,
 "aussen":[{"l":18,"d":166,"tol":"","gewinde":""},{"l":79,"d":121.5,"tol":"","gewinde":""}],
 "innen":[{"l":97,"d":85,"tol":"H7"}],"fase_links":0,"fase_rechts":6,
 "lochkreis":{"anzahl":9,"lk":140,"d":null,"gewinde":"M10","tiefe":null},"hinweise":[],"unsicher":[]}
Regeln für "dreh":
- Achse waagerecht. "aussen" = Außenabsätze von links nach rechts mit Länge l und Durchmesser d; die Summe aller l ist die Gesamtlänge. Fehlt eine Absatzlänge, berechne sie aus Gesamtlänge und übrigen Längen. "gesamtlaenge" = in der Skizze geschriebene Gesamtlänge, sonst null.
- "innen" = Bohrungsabschnitte von links nach rechts (d = 0 für einen vollen Abschnitt, z. B. wenn die Bohrung nur von rechts kommt). Keine Bohrung → [].
- tol = Toleranz oder Passung am Durchmesser (z. B. "h7", "H7", "±0,1"), gewinde = Außengewinde des Absatzes (z. B. "M24x1,5"), sonst "".
- fase_links/fase_rechts = Fasenbreite an der äußeren Stirnkante in mm ("6x45°" → 6), sonst 0.
- Bohrungen oder Gewinde auf einem Lochkreis in "lochkreis" (anzahl, lk = Lochkreisdurchmesser, d oder gewinde, tiefe), sonst null.
- Radien, Freistiche, Rauheiten, Innenfasen und alles, was nicht in die Felder passt, als kurze Sätze in "hinweise".

Format für "biege":
{"typ":"biege","name":"U-Profil","material":"","oberflaeche":"","stueck":1,"dicke":2,"breite":100,"ri":2,"massart":"aussen",
 "schenkel":[50,80,50],"biegungen":[{"winkel":90,"richtung":"links"},{"winkel":90,"richtung":"links"}],
 "bohrungen":[{"schenkel":1,"a":40,"y":50,"d":9}],"hinweise":[],"unsicher":[]}
Regeln für "biege":
- "schenkel" = Schenkellängen der Reihe nach von einem Ende des Profils zum anderen. "biegungen" hat genau einen Eintrag weniger.
- winkel = Winkel zwischen den beiden Schenkeln (90 = rechtwinklig, 135 = stumpf, 180 wäre gerade).
- richtung: gehe das Profil vom ersten zum letzten Schenkel entlang; "links" = Knick gegen den Uhrzeigersinn, "rechts" = im Uhrzeigersinn. U- und Hutprofil-Mitte: gleiche Richtung; Z-Profil: entgegengesetzt.
- breite = Kantlänge (Maß senkrecht zur Profilebene). ri = Innenbiegeradius, fehlt er → null.
- massart = "aussen", wenn die Schenkel außen bemaßt sind (üblich), "innen", wenn innen bemaßt.
- bohrungen: schenkel = Nummer des Schenkels ab 0; a = Abstand entlang des Schenkels von seinem Anfang (erster Schenkel: vom freien Ende; sonst von der Außenseite des vorherigen Schenkels); y = Abstand von der unteren Kante quer zur Breite; d fehlt → null.

Format für "fraes":
{"typ":"fraes","name":"U-Stück","material":"","oberflaeche":"","stueck":1,"laenge":60,"breite":15,"hoehe":19,
 "nuten":[{"seite":"unten","abstand":5,"breite":5,"tiefe":14}],
 "bohrungen":[{"richtung":"y","a":30,"b":9.5,"d":8,"gewinde":"","tiefe":null}],"hinweise":[],"unsicher":[]}
Regeln für "fraes":
- laenge = längstes Maß (x, waagerecht in der Vorderansicht), breite = Tiefe von vorne nach hinten (y), hoehe = senkrecht (z).
- nuten laufen über die ganze Länge: seite = "unten" | "oben" | "vorne" | "hinten"; abstand = Abstand der Nutkante von der vorderen Kante (bei unten/oben) bzw. von der Unterkante (bei vorne/hinten); breite und tiefe der Nut. Fehlt die Nuttiefe, schätze sie aus der Skizze und schreibe das in "unsicher".
- bohrungen: richtung "z" = von oben (a = x von links, b = y von vorne), "y" = quer von vorne durch die Seite (a = x von links, b = Höhe z von unten), "x" = längs von der linken Stirnseite (a = y von vorne, b = Höhe z). tiefe = Bohrtiefe, null bei Durchgangsbohrung. Gewinde z. B. "M6", sonst "".
- Ist eine Position nicht bemaßt, nimm die Mitte an und vermerke es in "unsicher".

Fertigung:
- Gib zusätzlich "fertigung" an: {"empfehlung":"fraes","begruendung":"kurze Begründung","alternative":null}.
- Kommen zwei Fertigungsarten in Frage (vor allem Biegeteil oder Frästeil, z. B. bei U- oder Winkelformen), setze "alternative" auf das vollständige Objekt im anderen Format (mit eigenem "typ"). Bei Biegeteilen beachte: Wandstärke = Blechdicke; sehr dicke Wände, sehr kleine Innenradien oder ein Spalt, der kaum breiter als die Wand ist, sprechen für Fräsen.

Gemeinsame Regeln:
- Alle Maße in mm als Zahlen (Dezimalpunkt). "5x" oder "5 Stk" ist die Stückzahl.
- Werkstoff/Oberfläche nur übernehmen, wenn angegeben (Rechtschreibung korrigieren), sonst "".
- name: kurze deutsche Benennung, falls nichts dasteht.
- Alles, was du nicht sicher lesen konntest oder annehmen musstest, als kurzen Satz in "unsicher".

Zahlen lesen (sehr wichtig):
- Lies jede handschriftliche Ziffer einzeln und sorgfältig. Leicht verwechselbar sind 3↔5, 1↔7, 6↔0↔8, 4↔9, 2↔7 und Komma↔Punkt.
- Prüfe jede Zahl gegen die Proportionen der Zeichnung (ein deutlich kürzer gezeichnetes Element hat kein größeres Maß) und gegen Maßketten (Teilmaße ergeben zusammen das Gesamtmaß).
- Passt eine Lesart nicht, nimm die Lesart, die rechnerisch und optisch passt, und schreibe in "unsicher" z. B. "Maß 35 gelesen (könnte 55 sein) – passt nur mit 35 zur Gesamtlänge".
- Gib in "masse" ALLE in der Skizze geschriebenen Maßzahlen genau so an, wie sie dastehen, mit kurzer Angabe, wozu sie gehören: "masse":[{"text":"35","wo":"Länge 1. Absatz","sicher":true}]. "sicher": false, wenn die Ziffer schlecht lesbar ist.

Bohrungen zählen (sehr wichtig):
- Jeder gezeichnete Kreis bzw. jede Ellipse auf einer Fläche ist eine eigene Bohrung – auch ohne eigene Beschriftung und auch in perspektivischen 3D-Skizzen.
- Steht nur an einem Kreis ein Durchmesser (z. B. "Ø8") und es gibt weitere gleich große Kreise, gilt der Durchmesser für alle; ein "2x"/"3x" muss dafür nicht dastehen.
- Ein Maß zwischen zwei Kreisen ist ihr Lochabstand, kein Abstand zum Rand. Fehlen Randmaße, verteile die Bohrungen symmetrisch zur Mitte und vermerke das in "unsicher".
- Zähle vor der Antwort die Kreise in der Skizze nach: Die Anzahl der Bohrungen im JSON muss genau dieser Anzahl entsprechen. Nenne die gezählte Anzahl in "unsicher" (z. B. "2 Bohrungen erkannt").${st.zusatz ? `\n\nZusätzliche Vorgaben des Nutzers:\n${st.zusatz}` : ""}${hint ? `\n\nHinweis des Nutzers zu genau dieser Skizze (hat Vorrang vor deiner Lesart):\n${hint}` : ""}`;
}

// einfaches Biegeprofil (Winkel oder U, 90°) in ein gleichwertiges Frästeil umrechnen
function szBiegeZuFraes(m, st) {
  const g = szBendCalc(szNormalize(m, st), st), t = g.t, o = g.outer;
  if (!g.B.every((b) => Math.abs(b.winkel - 90) < 0.5)) return null;
  const base = { typ: "fraes", name: m.name || "Frästeil", material: m.material && m.material !== st.werkstoff ? m.material : st.werkstoff_fraes,
    stueck: m.stueck, oberflaeche: m.oberflaeche, toleranz: m.toleranz, nr: m.nr, laenge: m.breite, fbohrungen: [], hinweise: m.hinweise,
    unsicher: ["Aus dem Biegeteil umgerechnet – Lage der Nut und Bohrungen bitte prüfen"] };
  const hs = (m.bohrungen || []);
  if (g.n === 3 && g.B[0].sgn === g.B[1].sgn) {
    const h = Math.max(o[0], o[2]);
    // Schenkel 1 = vordere Wand (freies Ende unten), Schenkel 2 = Steg oben, Schenkel 3 = hintere Wand
    const fb = hs.map((b) => b.schenkel === 0 ? { richtung: "y", a: b.y, b: b.a, d: b.d, tiefe: t }
      : b.schenkel === 1 ? { richtung: "z", a: b.y, b: b.a, d: b.d, tiefe: t }
      : { richtung: "y", a: b.y, b: h - b.a, d: b.d, tiefe: 0 });
    return { ...base, breite: o[1], hoehe: h, fbohrungen: fb, nuten: [{ seite: "unten", abstand: t, breite: Math.max(0.1, o[1] - 2 * t), tiefe: Math.max(0.1, h - t) }] };
  }
  if (g.n === 2) {
    // Schenkel 1 = Steg oben (freies Ende hinten), Schenkel 2 = vordere Wand
    const fb = hs.map((b) => b.schenkel === 0 ? { richtung: "z", a: b.y, b: o[0] - b.a, d: b.d, tiefe: t } : { richtung: "y", a: b.y, b: o[1] - b.a, d: b.d, tiefe: t });
    return { ...base, breite: o[0], hoehe: o[1], fbohrungen: fb, nuten: [{ seite: "unten", abstand: t, breite: Math.max(0.1, o[0] - t), tiefe: Math.max(0.1, o[1] - t) }] };
  }
  return null;
}

// KI-Antwort aufbereiten (Typ, Werkstoff, Annahmen)
function szPrepAI(data, st) {
  data = { ...data };
  if (!["flach", "dreh", "biege", "fraes"].includes(data.typ))
    data.typ = Array.isArray(data.schenkel) && data.schenkel.length > 1 ? "biege" : Array.isArray(data.aussen) && data.aussen.length ? "dreh" : data.hoehe ? "fraes" : "flach";
  if (["flach", "dreh", "biege", "fraes"].includes(st.typ)) data.typ = st.typ;
  if (!data.material || /nicht\s*angegeben|unbekannt/i.test(data.material)) data.material = szDefMat(data.typ, st);
  data.unsicher = Array.isArray(data.unsicher) ? [...data.unsicher] : [];
  // Plausibilität: Maßketten und Lage prüfen, schlecht lesbare Ziffern melden
  if (data.typ === "dreh" && num(data.gesamtlaenge, 0) > 0 && Array.isArray(data.aussen)) {
    const s = data.aussen.reduce((a, g) => a + num(g && g.l, 0), 0), gl = num(data.gesamtlaenge, 0);
    if (Math.abs(s - gl) > 0.05) data.unsicher.push(`Absatzlängen ergeben ${fmt(s)} mm, Gesamtlänge steht als ${fmt(gl)} mm – eine Zahl ist vermutlich falsch gelesen`);
  }
  if (data.typ === "flach" && Array.isArray(data.bohrungen)) {
    const L = num(data.laenge, 0), B = num(data.breite, 0);
    data.bohrungen.forEach((b, i) => { const x = num(b && b.x, 0), y = num(b && b.y, 0); if (L && B && (x <= 0 || y <= 0 || x >= L || y >= B)) data.unsicher.push(`Bohrung ${i + 1} liegt außerhalb des Teils (x ${fmt(x)}, y ${fmt(y)}) – Maß prüfen`); });
  }
  (Array.isArray(data.masse) ? data.masse : []).filter((x) => x && x.sicher === false).forEach((x) => data.unsicher.push(`Schlecht lesbar: „${x.text}“${x.wo ? ` (${x.wo})` : ""}`));
  if (data.typ === "biege" && (data.ri == null || data.ri === "")) data.unsicher.push("Innenbiegeradius nicht angegeben – angenommen Ri = Blechdicke");
  if (data.typ === "dreh" && data.lochkreis && data.lochkreis.anzahl && !data.lochkreis.d && !data.lochkreis.gewinde)
    data.unsicher.push(`Lochdurchmesser am Lochkreis fehlt – angenommen Ø${fmt(st.lochd)}`);
  if (data.typ === "fraes") { data.fbohrungen = Array.isArray(data.fbohrungen) ? data.fbohrungen : (Array.isArray(data.bohrungen) ? data.bohrungen : []); data.bohrungen = []; }
  delete data.fertigung;
  const leer = { oberflaeche: "", stueck: 1, toleranz: "", nr: "", eckradius: 0, bohrungen: [], innen: [], fase_links: 0, fase_rechts: 0, lochkreis: null, nuten: [], fbohrungen: [], hinweise: [] };
  return { ...leer, ...data };
}

/* ---------------- Karte ---------------- */
const SZ_KEY = "skizze_zeichnung_settings";
class SkizzeZeichnungCard extends HTMLElement {
  setConfig(config) { this._config = config || {}; }
  getCardSize() { return 12; }
  getGridOptions() { return { columns: "full", rows: "auto" }; }
  set hass(hass) {
    this._hass = hass;
    if (!this._built) this._build();
    this._updateEntityInfo();
  }

  _aiEntities() { return Object.keys(this._hass?.states || {}).filter((e) => e.startsWith("ai_task.")); }
  _entity() {
    const ids = this._aiEntities();
    const want = this._st?.entity || this._config.entity;
    if (want && ids.includes(want)) return want;
    return ids.find((e) => /google|gemini/.test(e)) || ids[0];
  }

  async _loadSettings() {
    let v = null;
    try { v = JSON.parse(localStorage.getItem(SZ_KEY) || "null"); } catch (e) {}
    try { const r = await this._hass.callWS({ type: "frontend/get_user_data", key: SZ_KEY }); if (r && r.value) v = r.value; } catch (e) {}
    this._st = Object.assign({}, SZ_SET_DEFAULT, v || {});
    this._renderAll();
  }
  async _saveSettings() {
    try { localStorage.setItem(SZ_KEY, JSON.stringify(this._st)); } catch (e) {}
    try { await this._hass.callWS({ type: "frontend/set_user_data", key: SZ_KEY, value: this._st }); this._status("✓ Einstellungen gespeichert (für dein Benutzerkonto auf allen Geräten)."); }
    catch (e) { this._status("Einstellungen nur auf diesem Gerät gespeichert: " + esc(e.message || e), "err"); }
  }

  _build() {
    this._built = true;
    this._st = Object.assign({}, SZ_SET_DEFAULT);
    let saved = null;
    try { saved = JSON.parse(localStorage.getItem("skizze-zeichnung-model") || "null"); } catch (e) {}
    this._model = szNormalize(saved || SZ_DEFAULT, this._st);
    const root = this.attachShadow({ mode: "open" });
    root.innerHTML = `
<style>
:host{display:block}
ha-card{padding:16px;overflow:hidden}
h2{margin:0 0 4px;font-size:20px;font-weight:600}
.top{display:flex;justify-content:space-between;align-items:flex-start;gap:8px}
.sub{color:var(--secondary-text-color);font-size:13px;margin-bottom:14px}
.row{display:flex;gap:8px;flex-wrap:wrap;align-items:center}
button,select{font:inherit;font-size:14px;border:0;border-radius:10px;padding:10px 14px;cursor:pointer;background:var(--secondary-background-color);color:var(--primary-text-color);display:inline-flex;align-items:center;gap:6px}
select{padding:7px 8px;border:1px solid var(--divider-color);background:var(--card-background-color);width:100%}
button.pri{background:var(--primary-color);color:var(--text-primary-color,#fff);font-weight:600}
button.on{outline:2px solid var(--primary-color)}
button:disabled{opacity:.5;cursor:default}
.status{margin:12px 0;font-size:14px;min-height:20px}
.status.err{color:var(--error-color,#db4437)}
.warn{background:rgba(255,166,0,.12);border-left:3px solid var(--warning-color,#ffa600);padding:8px 10px;border-radius:6px;font-size:13px;margin:10px 0}
.warn ul{margin:4px 0 0 18px;padding:0}
.grid{display:grid;grid-template-columns:minmax(0,1fr) 360px;gap:16px;margin-top:8px}
@media (max-width:900px){.grid{grid-template-columns:1fr}}
.sheet{background:#fff;border-radius:8px;border:1px solid var(--divider-color);overflow:hidden}
.sheet svg{width:100%;height:auto;display:block}
.photo{max-width:100%;max-height:220px;border-radius:8px;display:block;margin-bottom:10px;border:1px solid var(--divider-color)}
fieldset{border:1px solid var(--divider-color);border-radius:10px;padding:10px 12px;margin:0 0 12px;min-width:0}
legend{font-size:13px;color:var(--secondary-text-color);padding:0 4px}
label{display:flex;flex-direction:column;font-size:12px;color:var(--secondary-text-color);gap:3px;flex:1;min-width:70px}
input,textarea{font:inherit;font-size:14px;padding:7px 8px;border-radius:8px;border:1px solid var(--divider-color);background:var(--card-background-color);color:var(--primary-text-color);width:100%;box-sizing:border-box}
textarea{min-height:54px;resize:vertical}
.f{display:flex;gap:8px;margin-bottom:8px}
table{width:100%;border-collapse:collapse;font-size:13px}
td,th{padding:3px 2px;text-align:left;font-weight:normal;color:var(--secondary-text-color)}
td input{padding:6px}
td select{min-width:96px;padding:6px}
td.x{width:34px}
.del{padding:6px 8px;background:transparent}
.assumed input{border-color:var(--warning-color,#ffa600)}
.spin{width:14px;height:14px;border:2px solid currentColor;border-right-color:transparent;border-radius:50%;animation:s 0.8s linear infinite;display:inline-block}
@keyframes s{to{transform:rotate(360deg)}}
.hide{display:none}
.foot{font-size:11px;color:var(--secondary-text-color);margin-top:8px}
.set{background:var(--secondary-background-color);border-radius:12px;padding:12px;margin-top:12px}
.set h3{margin:0 0 10px;font-size:16px}
.sg{display:grid;grid-template-columns:repeat(auto-fill,minmax(220px,1fr));gap:10px}
.seg{display:flex;gap:4px;margin-bottom:10px}
.seg button{flex:1;justify-content:center;padding:8px}
.seg{flex-wrap:wrap}
.lese{background:rgba(3,169,244,.10);border-left:3px solid var(--primary-color);padding:8px 10px;border-radius:6px;font-size:13px;margin:10px 0}
.chips{display:flex;flex-wrap:wrap;gap:6px;margin-top:6px}
.chip{border:1px solid var(--divider-color);border-radius:8px;padding:3px 8px;font-weight:600}
.chip small{font-weight:normal;color:var(--secondary-text-color);margin-left:4px}
.chip.bad{border-color:var(--warning-color,#ffa600);color:var(--warning-color,#ffa600)}
.fert{background:rgba(3,169,244,.12);border-left:3px solid var(--primary-color);padding:8px 10px;border-radius:6px;font-size:13px;margin:10px 0;display:flex;gap:10px;align-items:center;justify-content:space-between;flex-wrap:wrap}
</style>
<ha-card>
  <div class="top"><div><h2>Skizze → Zeichnung</h2>
  <div class="sub">Foto deiner Handskizze machen – Flach-, Dreh-, Biege- oder Frästeil wird erkannt und als Fertigungszeichnung (A4) gezeichnet. Danach kannst du alles korrigieren.</div></div>
  <button id="gear" title="Einstellungen"><ha-icon icon="mdi:cog"></ha-icon></button></div>
  <div class="row">
    <button class="pri" id="cam"><ha-icon icon="mdi:camera"></ha-icon>Foto aufnehmen</button>
    <button id="pick"><ha-icon icon="mdi:image"></ha-icon>Bild wählen</button>
    <button id="neuF"><ha-icon icon="mdi:rectangle-outline"></ha-icon>Neues Flachteil</button>
    <button id="neuD"><ha-icon icon="mdi:cylinder"></ha-icon>Neues Drehteil</button>
    <button id="neuB"><ha-icon icon="mdi:vector-polyline"></ha-icon>Neues Biegeteil</button>
    <button id="neuM"><ha-icon icon="mdi:cube-outline"></ha-icon>Neues Frästeil</button>
    <input type="file" id="fcam" accept="image/*" capture="environment" class="hide">
    <input type="file" id="fpick" accept="image/*" class="hide">
  </div>
  <div class="row" style="margin-top:8px">
    <input id="hint" style="flex:1;min-width:220px" placeholder="Hinweis für die KI (optional), z. B. „2 Bohrungen Ø8, Abstand 30“">
    <button id="again" disabled><ha-icon icon="mdi:refresh"></ha-icon>Nochmal lesen</button>
  </div>
  <div class="set hide" id="set"></div>
  <div class="status" id="status"></div>
  <div id="warn"></div>
  <div class="grid">
    <div>
      <div class="sheet" id="sheet"></div>
      <div class="row" style="margin-top:10px">
        <button class="pri" id="print"><ha-icon icon="mdi:printer"></ha-icon>Drucken / PDF</button>
        <button id="svg"><ha-icon icon="mdi:download"></ha-icon>SVG laden</button>
        <button id="png"><ha-icon icon="mdi:file-image"></ha-icon>PNG laden</button>
        <button id="save"><ha-icon icon="mdi:content-save"></ha-icon>In Medien speichern</button>
      </div>
      <div class="foot">Drucken auf A4 quer ohne Skalierung („Tatsächliche Größe“) – dann stimmt der Maßstab.</div>
    </div>
    <div>
      <img class="photo hide" id="photo" alt="Skizze">
      <div id="form"></div>
    </div>
  </div>
  <div class="foot" id="info"></div>
</ha-card>`;
    const $ = (id) => root.getElementById(id);
    this.$ = $;
    $("cam").onclick = () => $("fcam").click();
    $("pick").onclick = () => $("fpick").click();
    $("fcam").onchange = (e) => this._onFile(e.target);
    $("fpick").onchange = (e) => this._onFile(e.target);
    $("again").onclick = () => { if (this._lastBlob) this._readImage(this._lastBlob); };
    $("hint").addEventListener("keydown", (e) => { if (e.key === "Enter" && this._lastBlob) this._readImage(this._lastBlob); });
    const neu = (typ) => {
      const base = typ === "dreh"
        ? { typ, name: "Welle", material: this._st.werkstoff_dreh, aussen: [{ l: 30, d: 40 }, { l: 50, d: 25, tol: "h7" }], innen: [], fase_links: 1, fase_rechts: 1, bohrungen: [] }
        : typ === "fraes"
          ? { typ, name: "Block", material: this._st.werkstoff_fraes, laenge: 60, breite: 30, hoehe: 20, nuten: [], fbohrungen: [{ richtung: "z", a: 30, b: 15, d: 6.6, gewinde: "", tiefe: 0 }], bohrungen: [] }
          : typ === "biege"
          ? { typ, name: "Winkel", material: this._st.werkstoff, dicke: 2, breite: 50, ri: 2, massart: "aussen", schenkel: [{ l: 40 }, { l: 30 }], biegungen: [{ winkel: 90, richtung: "links" }], bohrungen: [] }
          : { typ, name: "Lasche", material: this._st.werkstoff, bohrungen: [] };
      this._model = szNormalize({ ...SZ_DEFAULT, ...base, lochkreis: null }, this._st);
      this._unsicher = []; this._fert = null; this._gelesen = []; $("photo").classList.add("hide"); this._status(""); this._renderAll();
    };
    $("neuF").onclick = () => neu("flach");
    $("neuD").onclick = () => neu("dreh");
    $("neuB").onclick = () => neu("biege");
    $("neuM").onclick = () => neu("fraes");
    $("gear").onclick = () => { const el = $("set"); el.classList.toggle("hide"); $("gear").classList.toggle("on", !el.classList.contains("hide")); this._renderSettings(); };
    $("print").onclick = () => this._print();
    $("svg").onclick = () => this._download(new Blob([this._svg()], { type: "image/svg+xml" }), this._fileName("svg"));
    $("png").onclick = () => this._png();
    $("save").onclick = () => this._saveMedia();
    this._unsicher = this._model.unsicher || [];
    this._renderAll();
    this._loadSettings();
  }

  _svg() { return szRender(this._model, this._st); }

  _updateEntityInfo() {
    if (!this.$) return;
    const e = this._entity();
    this.$("info").textContent = e ? `KI: ${this._hass.states[e]?.attributes?.friendly_name || e} · v${SZ_VERSION}` : `Keine KI-Aufgaben-Entität (ai_task) gefunden – Foto-Auswertung nicht möglich, manuelle Eingabe geht. · v${SZ_VERSION}`;
  }

  _status(msg, type = "") { const el = this.$("status"); el.className = "status " + type; el.innerHTML = msg; }

  _fileName(ext) {
    const m = szNormalize(this._model, this._st);
    const mass = m.typ === "dreh"
      ? `D${fmt(Math.max(...m.aussen.map((g) => g.d)))}x${fmt(m.aussen.reduce((a, g) => a + g.l, 0))}`
      : m.typ === "biege"
        ? `${m.schenkel.map((g) => fmt(g.l)).join("x")}_B${fmt(m.breite)}_t${fmt(m.dicke)}`
        : m.typ === "fraes"
        ? `${fmt(m.laenge)}x${fmt(m.breite)}x${fmt(m.hoehe)}`
        : `${fmt(m.laenge)}x${fmt(m.breite)}x${fmt(m.dicke)}`;
    return `${m.nr ? m.nr + "_" : ""}${m.name}_${mass}`.replace(/[^\wäöüÄÖÜß,.-]+/g, "_") + "." + ext;
  }

  _persist() { try { localStorage.setItem("skizze-zeichnung-model", JSON.stringify(this._model)); } catch (e) {} }
  _renderSheet() {
    this.$("sheet").innerHTML = this._svg(); this._persist();
    if (this._model.typ === "biege") this._renderWarn();
    const lf = this.$("form").querySelector("#lf");
    if (lf && this._model.typ === "biege") {
      const g = szBendCalc(szNormalize(this._model, this._st), this._st);
      lf.textContent = `Gestreckte Länge ≈ ${fmt(Math.round(g.Lf * 10) / 10)} mm (k = ${fmt(Math.round(g.k * 100) / 100)})`;
    }
  }
  _renderAll() { this._renderForm(); this._renderSheet(); this._renderWarn(); if (!this.$("set").classList.contains("hide")) this._renderSettings(); }
  _renderWarn() {
    const u = this._unsicher || [], m = this._model;
    let h = u.length ? `<div class="warn"><b>Bitte prüfen:</b><ul>${u.map((x) => `<li>${esc(x)}</li>`).join("")}</ul></div>` : "";
    const gl = this._gelesen || [];
    if (gl.length) h += `<div class="lese"><b>Gelesene Maße</b> – mit der Skizze vergleichen:<div class="chips">${gl.map((x) => `<span class="chip${x.sicher === false ? " bad" : ""}">${esc(x.text)}${x.wo ? `<small>${esc(x.wo)}</small>` : ""}</span>`).join("")}</div></div>`;
    const fe = this._fert;
    if (fe) {
      const rec = SZ_TYPNAME[fe.empfehlung] || "";
      h += `<div class="fert"><div><b>Fertigung:</b> Empfehlung ${fe.auto && fe.auto.includes("kantbar") ? "(Kantbarkeits-Prüfung)" : "(KI)"}: <b>${esc(rec)}</b>${fe.begruendung ? ` – ${esc(fe.begruendung)}` : ""}.
        Gezeichnet ist gerade: <b>${esc(SZ_TYPNAME[m.typ])}</b>${fe.auto && fe.empfehlung === m.typ ? ` (${esc(fe.auto)})` : ""}.</div>
        ${fe.alt ? `<button id="swap"><ha-icon icon="mdi:swap-horizontal"></ha-icon>Als ${esc(SZ_TYPNAME[fe.alt.typ])} zeichnen</button>` : ""}</div>`;
    }
    if (m.typ === "biege") {
      const ch = szBendChecks(szNormalize(m, this._st), this._st);
      if (ch.length) h += `<div class="warn"><b>Kantbarkeit (Faustregeln):</b><ul>${ch.map((x) => `<li>${esc(x)}</li>`).join("")}</ul>Eventuell als Frästeil fertigen – oben unter „Frästeil“ umschalten.</div>`;
    }
    this.$("warn").innerHTML = h;
    const sw = this.$("warn").querySelector("#swap");
    if (sw) sw.onclick = () => { const cur = this._model; this._model = fe.alt; fe.alt = cur; this._renderAll(); };
  }

  /* ----- Einstellungen ----- */
  _renderSettings() {
    const st = this._st, el = this.$("set");
    const t = (k, label, ph = "") => `<label>${label}<input data-s="${k}" value="${esc(st[k])}" placeholder="${esc(ph)}"></label>`;
    const sel = (k, label, opts) => `<label>${label}<select data-s="${k}">${opts.map(([v, l]) => `<option value="${esc(v)}"${String(st[k]) === v ? " selected" : ""}>${esc(l)}</option>`).join("")}</select></label>`;
    const ents = this._aiEntities();
    el.innerHTML = `<h3>Einstellungen</h3>
<div class="sg">
  ${t("firma", "Firma (Schriftfeld)")}
  ${t("zeichner", "Gezeichnet von")}
  ${t("toleranz", "Allgemeintoleranz", "ISO 2768-m")}
  ${t("werkstoff", "Standard-Werkstoff Flachteil", "z. B. S235JR")}
  ${t("werkstoff_dreh", "Standard-Werkstoff Drehteil", "z. B. 11SMnPb30")}
  ${t("werkstoff_fraes", "Standard-Werkstoff Frästeil", "z. B. AlMgSi1 / S355")}
  <label>Angenommener Bohrungs-Ø (wenn nicht angegeben)<input data-s="lochd" type="number" step="0.1" inputmode="decimal" value="${esc(st.lochd)}"></label>
  ${sel("massstab", "Maßstab", [["auto", "Automatisch"], ["5:1", "5:1"], ["2:1", "2:1"], ["1:1", "1:1"], ["1:2", "1:2"], ["1:5", "1:5"], ["1:10", "1:10"], ["1:20", "1:20"]])}
  ${sel("typ", "Bauteiltyp bei Fotos", [["auto", "Automatisch erkennen"], ["flach", "Immer Flachteil"], ["dreh", "Immer Drehteil"], ["biege", "Immer Biegeteil"], ["fraes", "Immer Frästeil"]])}
  ${sel("fertigung", "Fertigungsart bei Fotos", [["auto", "Automatisch wählen (KI + Prüfung)"], ["ki", "So zeichnen, wie die KI es liest"]])}
  <label>k-Faktor Biegeteile (leer = nach DIN 6935)<input data-s="kfaktor" inputmode="decimal" value="${esc(st.kfaktor)}" placeholder="automatisch"></label>
  ${sel("entity", "KI", [["", "Automatisch"], ...ents.map((e) => [e, this._hass.states[e]?.attributes?.friendly_name || e])])}
</div>
<label style="margin-top:10px">Zusätzliche Anweisung an die KI<textarea data-s="zusatz" placeholder="z. B. Maße ohne Einheit sind mm; Gewinde immer als Regelgewinde">${esc(st.zusatz)}</textarea></label>
<div class="row" style="margin-top:10px"><button class="pri" id="sSave"><ha-icon icon="mdi:content-save"></ha-icon>Speichern</button><button id="sReset">Standard</button></div>`;
    el.querySelectorAll("[data-s]").forEach((i) => i.addEventListener(i.tagName === "SELECT" ? "change" : "input", () => {
      this._st[i.dataset.s] = i.dataset.s === "lochd" ? num(i.value, 6.6) : i.value;
      this._renderSheet(); this._updateEntityInfo();
    }));
    el.querySelector("#sSave").onclick = () => this._saveSettings();
    el.querySelector("#sReset").onclick = () => { this._st = Object.assign({}, SZ_SET_DEFAULT); this._renderAll(); };
  }

  /* ----- Formular ----- */
  _renderForm() {
    const m = this._model = szNormalize(this._model, this._st);
    const inp = (k, label, type = "text", step, ph = "") => `<label>${label}<input data-k="${k}" type="${type}" ${step ? `step="${step}" inputmode="decimal"` : ""} value="${esc(m[k])}" placeholder="${esc(ph)}"></label>`;
    const typSel = `<div class="seg"><button data-typ="flach" class="${m.typ === "flach" ? "pri" : ""}">Flachteil</button><button data-typ="dreh" class="${m.typ === "dreh" ? "pri" : ""}">Drehteil</button><button data-typ="biege" class="${m.typ === "biege" ? "pri" : ""}">Biegeteil</button><button data-typ="fraes" class="${m.typ === "fraes" ? "pri" : ""}">Frästeil</button></div>`;
    const kopf = `<fieldset><legend>Bauteil</legend>${typSel}
  <div class="f">${inp("name", "Benennung")}${inp("nr", "Zeichn.-Nr.")}</div>
  <div class="f">${inp("material", "Werkstoff", "text", null, szDefMat(m.typ, this._st))}${inp("stueck", "Stückzahl", "number", "1")}</div>
  <div class="f">${inp("oberflaeche", "Oberfläche")}${inp("toleranz", "Toleranz", "text", null, this._st.toleranz)}</div>`;
    const hin = `<fieldset><legend>Zusätzliche Hinweise (eine Zeile je Hinweis)</legend><textarea data-k="hinweise">${esc(m.hinweise.join("\n"))}</textarea></fieldset>`;
    const n = (path, v) => `<input data-p="${path}" type="number" step="0.1" inputmode="decimal" value="${esc(v)}">`;
    const t = (path, v, ph = "") => `<input data-p="${path}" value="${esc(v)}" placeholder="${esc(ph)}">`;
    const del = (list, i) => `<td class="x"><button class="del" data-del="${list}:${i}" title="Entfernen"><ha-icon icon="mdi:close"></ha-icon></button></td>`;
    let body;
    if (m.typ === "flach") {
      body = `${kopf}
  <div class="f">${inp("laenge", "Länge", "number", "0.1")}${inp("breite", "Breite", "number", "0.1")}${inp("dicke", "Dicke t", "number", "0.1")}</div>
  <div class="f">${inp("eckradius", "Eckradius", "number", "0.1")}</div></fieldset>
<fieldset><legend>Bohrungen (x von links, y von unten, mm)</legend>
  <table><tr><th>x</th><th>y</th><th>Ø</th><th></th></tr>
  ${m.bohrungen.map((b, i) => `<tr class="${b.angenommen ? "assumed" : ""}"><td>${n(`bohrungen.${i}.x`, b.x)}</td><td>${n(`bohrungen.${i}.y`, b.y)}</td><td>${n(`bohrungen.${i}.d`, b.d)}</td>${del("bohrungen", i)}</tr>`).join("")}
  </table>
  <div class="row" style="margin-top:6px"><button data-add="bohrungen"><ha-icon icon="mdi:plus"></ha-icon>Bohrung</button></div>
  ${m.bohrungen.some((b) => b.angenommen) ? `<div class="foot">Orange = Durchmesser war nicht angegeben (angenommen Ø${fmt(this._st.lochd)}).</div>` : ""}
</fieldset>${hin}`;
    } else if (m.typ === "fraes") {
      const opt = (v, cur, l) => `<option value="${v}"${cur === v ? " selected" : ""}>${l}</option>`;
      const tt = (path, v, ph = "") => `<input data-p="${path}" value="${esc(v)}" placeholder="${esc(ph)}">`;
      body = `${kopf}
  <div class="f">${inp("laenge", "Länge (x)", "number", "0.1")}${inp("breite", "Breite (y)", "number", "0.1")}${inp("hoehe", "Höhe (z)", "number", "0.1")}</div></fieldset>
<fieldset><legend>Nuten (durchgehend über die Länge)</legend>
  <table><tr><th>Seite</th><th>Abstand</th><th>Breite</th><th>Tiefe</th><th></th></tr>
  ${m.nuten.map((u, i) => `<tr><td><select data-p="nuten.${i}.seite">${opt("unten", u.seite, "unten")}${opt("oben", u.seite, "oben")}${opt("vorne", u.seite, "vorne")}${opt("hinten", u.seite, "hinten")}</select></td>
    <td>${n(`nuten.${i}.abstand`, u.abstand)}</td><td>${n(`nuten.${i}.breite`, u.breite)}</td><td>${n(`nuten.${i}.tiefe`, u.tiefe)}</td>${del("nuten", i)}</tr>`).join("")}
  </table>
  <div class="row" style="margin-top:6px"><button data-add="nuten"><ha-icon icon="mdi:plus"></ha-icon>Nut</button></div>
  <div class="foot">Abstand: unten/oben von der Vorderkante, vorne/hinten von der Unterkante.</div>
</fieldset>
<fieldset><legend>Bohrungen</legend>
  <table><tr><th>Richtung</th><th>a</th><th>b</th><th>Ø</th><th>Gew.</th><th>Tiefe</th><th></th></tr>
  ${m.fbohrungen.map((b, i) => `<tr class="${b.angenommen ? "assumed" : ""}"><td><select data-p="fbohrungen.${i}.richtung">${opt("z", b.richtung, "von oben")}${opt("y", b.richtung, "von vorne")}${opt("x", b.richtung, "Stirnseite")}</select></td>
    <td>${n(`fbohrungen.${i}.a`, b.a)}</td><td>${n(`fbohrungen.${i}.b`, b.b)}</td><td>${n(`fbohrungen.${i}.d`, b.d)}</td><td>${tt(`fbohrungen.${i}.gewinde`, b.gewinde, "M6")}</td><td>${n(`fbohrungen.${i}.tiefe`, b.tiefe)}</td>${del("fbohrungen", i)}</tr>`).join("")}
  </table>
  <div class="row" style="margin-top:6px"><button data-add="fbohrungen"><ha-icon icon="mdi:plus"></ha-icon>Bohrung</button></div>
  <div class="foot">von oben: a = x, b = y · von vorne: a = x, b = Höhe · Stirnseite: a = y, b = Höhe · Tiefe 0 = durch</div>
</fieldset>${hin}`;
    } else if (m.typ === "biege") {
      const g = szBendCalc(m, this._st);
      const opt = (v, cur, l) => `<option value="${v}"${cur === v ? " selected" : ""}>${l}</option>`;
      body = `${kopf}
  <div class="f">${inp("dicke", "Dicke t", "number", "0.1")}${inp("breite", "Breite (Kantlänge)", "number", "0.1")}${inp("ri", "Innenradius", "number", "0.1")}</div>
  <div class="f"><label>Schenkelmaße<select data-k="massart">${opt("aussen", m.massart, "Außenmaße")}${opt("innen", m.massart, "Innenmaße")}</select></label></div></fieldset>
<fieldset><legend>Schenkel &amp; Biegungen (der Reihe nach)</legend>
  <table><tr><th>#</th><th>Länge</th><th>Winkel danach</th><th>Richtung</th><th></th></tr>
  ${m.schenkel.map((sg, i) => `<tr><td>${i + 1}</td><td>${n(`schenkel.${i}.l`, sg.l)}</td>
    <td>${i < m.biegungen.length ? n(`biegungen.${i}.winkel`, m.biegungen[i].winkel) : ""}</td>
    <td>${i < m.biegungen.length ? `<select data-p="biegungen.${i}.richtung">${opt("links", m.biegungen[i].richtung, "links ↺")}${opt("rechts", m.biegungen[i].richtung, "rechts ↻")}</select>` : ""}</td>
    ${m.schenkel.length > 2 ? del("schenkel", i) : "<td></td>"}</tr>`).join("")}
  </table>
  <div class="row" style="margin-top:6px"><button data-add="schenkel"><ha-icon icon="mdi:plus"></ha-icon>Schenkel</button></div>
  <div class="foot" id="lf">Gestreckte Länge ≈ ${fmt(Math.round(g.Lf * 10) / 10)} mm (k = ${fmt(Math.round(g.k * 100) / 100)})</div>
</fieldset>
<fieldset><legend>Bohrungen (Schenkel-Nr., a = Abstand vom Schenkelanfang, y = von unten)</legend>
  <table><tr><th>Schenkel</th><th>a</th><th>y</th><th>Ø</th><th></th></tr>
  ${m.bohrungen.map((b, i) => `<tr class="${b.angenommen ? "assumed" : ""}"><td><input data-p="bohrungen.${i}.schenkel" type="number" step="1" min="1" inputmode="numeric" value="${b.schenkel + 1}"></td><td>${n(`bohrungen.${i}.a`, b.a)}</td><td>${n(`bohrungen.${i}.y`, b.y)}</td><td>${n(`bohrungen.${i}.d`, b.d)}</td>${del("bohrungen", i)}</tr>`).join("")}
  </table>
  <div class="row" style="margin-top:6px"><button data-add="bohrungen"><ha-icon icon="mdi:plus"></ha-icon>Bohrung</button></div>
  <div class="foot">Schenkel 1: a vom freien Ende; übrige Schenkel: a von der Außenseite des vorherigen Schenkels.</div>
</fieldset>${hin}`;
    } else {
      const lk = m.lochkreis;
      body = `${kopf}</fieldset>
<fieldset><legend>Außenkontur – Absätze von links nach rechts</legend>
  <table><tr><th>Länge</th><th>Ø</th><th>Passung / Gewinde</th><th></th></tr>
  ${m.aussen.map((g, i) => `<tr><td>${n(`aussen.${i}.l`, g.l)}</td><td>${n(`aussen.${i}.d`, g.d)}</td><td>${t(`aussen.${i}.zusatz`, g.gewinde || g.tol, "h7 / M24")}</td>${del("aussen", i)}</tr>`).join("")}
  </table>
  <div class="row" style="margin-top:6px"><button data-add="aussen"><ha-icon icon="mdi:plus"></ha-icon>Absatz</button><span class="foot">Gesamtlänge ${fmt(m.aussen.reduce((a, g) => a + g.l, 0))} mm</span></div>
</fieldset>
<fieldset><legend>Bohrung innen – von links (Ø 0 = voll)</legend>
  <table><tr><th>Länge</th><th>Ø</th><th>Passung</th><th></th></tr>
  ${m.innen.map((g, i) => `<tr><td>${n(`innen.${i}.l`, g.l)}</td><td>${n(`innen.${i}.d`, g.d)}</td><td>${t(`innen.${i}.tol`, g.tol, "H7")}</td>${del("innen", i)}</tr>`).join("")}
  </table>
  <div class="row" style="margin-top:6px"><button data-add="innen"><ha-icon icon="mdi:plus"></ha-icon>Bohrungsabschnitt</button></div>
</fieldset>
<fieldset><legend>Fasen &amp; Lochkreis</legend>
  <div class="f">${inp("fase_links", "Fase links (×45°)", "number", "0.1")}${inp("fase_rechts", "Fase rechts (×45°)", "number", "0.1")}</div>
  <div class="f"><label>Anzahl Löcher${n("lochkreis.anzahl", lk.anzahl)}</label><label>Lochkreis Ø${n("lochkreis.lk", lk.lk)}</label></div>
  <div class="f"><label>Loch Ø${n("lochkreis.d", lk.d)}</label><label>oder Gewinde${t("lochkreis.gewinde", lk.gewinde, "M10")}</label><label>Tiefe${n("lochkreis.tiefe", lk.tiefe)}</label></div>
</fieldset>${hin}`;
    }
    this.$("form").innerHTML = body;
    const f = this.$("form");
    f.querySelectorAll("[data-typ]").forEach((b) => b.onclick = () => {
      const neuTyp = b.dataset.typ;
      if (this._model.typ === "biege" && neuTyp === "fraes" && !(this._model.nuten || []).length) {
        const conv = szBiegeZuFraes(this._model, this._st);
        if (conv) { this._model = szNormalize(conv, this._st); this._unsicher = this._model.unsicher; this._renderAll(); return; }
      }
      this._model.typ = neuTyp; this._renderAll();
    });
    f.querySelectorAll("[data-k]").forEach((el) => el.addEventListener("input", () => {
      const k = el.dataset.k;
      this._model[k] = k === "hinweise" ? el.value.split("\n").filter((x) => x.trim()) : el.value;
      this._renderSheet();
    }));
    f.querySelectorAll("[data-p]").forEach((el) => el.addEventListener("input", () => {
      const [list, a, b] = el.dataset.p.split(".");
      if (list === "lochkreis") { this._model.lochkreis = { ...this._model.lochkreis, [a]: a === "gewinde" ? el.value : num(el.value, 0) }; }
      else {
        const o = this._model[list][+a];
        if (b === "zusatz") { const v = el.value.trim(); if (/^M\s*\d/i.test(v)) { o.gewinde = v; o.tol = ""; } else { o.tol = v; o.gewinde = ""; } }
        else if (b === "tol" || b === "gewinde" || el.tagName === "SELECT") o[b] = el.value;
        else if (b === "schenkel") o.schenkel = Math.max(0, Math.round(num(el.value, 1)) - 1);
        else { o[b] = num(el.value, 0); if ((list === "bohrungen" || list === "fbohrungen") && b === "d") { o.angenommen = false; el.closest("tr").classList.remove("assumed"); } }
      }
      this._renderSheet();
    }));
    f.querySelectorAll("[data-del]").forEach((el) => el.onclick = () => {
      const [l, i] = el.dataset.del.split(":");
      this._model[l].splice(+i, 1);
      if (l === "schenkel") this._model.biegungen.splice(Math.min(+i, this._model.biegungen.length - 1), 1);
      this._renderAll();
    });
    f.querySelectorAll("[data-add]").forEach((el) => el.onclick = () => {
      const l = el.dataset.add, m2 = this._model;
      if (l === "bohrungen") m2.bohrungen.push({ x: m2.laenge / 2, y: m2.breite / 2, d: num(this._st.lochd, 6.6), schenkel: 0, a: 10 });
      if (l === "schenkel") { m2.schenkel.push({ l: 30 }); m2.biegungen.push({ winkel: 90, richtung: "links" }); }
      if (l === "nuten") m2.nuten.push({ seite: "oben", abstand: m2.breite / 3, breite: m2.breite / 3, tiefe: m2.hoehe / 3 });
      if (l === "fbohrungen") m2.fbohrungen.push({ richtung: "z", a: m2.laenge / 2, b: m2.breite / 2, d: num(this._st.lochd, 6.6), gewinde: "", tiefe: 0 });
      if (l === "aussen") { const last = m2.aussen[m2.aussen.length - 1]; m2.aussen.push({ l: 20, d: last ? last.d * 0.8 : 20, tol: "", gewinde: "" }); }
      if (l === "innen") { const L = m2.aussen.reduce((a, g) => a + g.l, 0), used = m2.innen.reduce((a, g) => a + g.l, 0); m2.innen.push({ l: Math.max(1, L - used), d: 10, tol: "" }); }
      this._renderAll();
    });
  }

  /* ----- Foto → KI ----- */
  async _shrink(file) {
    const url = URL.createObjectURL(file);
    try {
      const img = await new Promise((res, rej) => { const i = new Image(); i.onload = () => res(i); i.onerror = () => rej(new Error("Bild konnte nicht geladen werden")); i.src = url; });
      const max = 2000, k = Math.min(1, max / Math.max(img.naturalWidth, img.naturalHeight));
      const c = document.createElement("canvas");
      c.width = Math.round(img.naturalWidth * k); c.height = Math.round(img.naturalHeight * k);
      c.getContext("2d").drawImage(img, 0, 0, c.width, c.height);
      const blob = await new Promise((res) => c.toBlob(res, "image/jpeg", 0.85));
      return { blob, preview: c.toDataURL("image/jpeg", 0.6) };
    } finally { URL.revokeObjectURL(url); }
  }

  async _upload(blob, name) {
    const fd = new FormData();
    fd.append("media_content_id", "media-source://media_source/local/.");
    fd.append("file", blob, name);
    const r = await this._hass.fetchWithAuth("/api/media_source/local_source/upload", { method: "POST", body: fd });
    if (!r.ok) throw new Error(`Upload fehlgeschlagen (${r.status}) ${await r.text().catch(() => "")}`);
    return (await r.json()).media_content_id;
  }

  async _onFile(input) {
    const file = input.files && input.files[0];
    input.value = "";
    if (!file) return;
    try {
      this._status(`<span class="spin"></span> Bild wird vorbereitet …`);
      const { blob, preview } = await this._shrink(file);
      const ph = this.$("photo"); ph.src = preview; ph.classList.remove("hide");
      this._lastBlob = blob;
      this.$("again").disabled = false;
    } catch (e) { this._status(esc(e.message || String(e)), "err"); return; }
    await this._readImage(this._lastBlob);
  }

  async _readImage(blob) {
    const ent = this._entity(), st = this._st;
    const hint = (this.$("hint").value || "").trim();
    const btns = ["cam", "pick", "again"].map((id) => this.$(id));
    btns.forEach((b) => (b.disabled = true));
    let mediaId = null;
    try {
      if (!ent) throw new Error("Keine KI eingerichtet (ai_task-Entität fehlt). Bis dahin kannst du die Maße rechts eintippen.");
      mediaId = await this._upload(blob, `skizze_${Date.now()}.jpg`);
      this._status(`<span class="spin"></span> KI liest die Skizze … (dauert ca. 5–30 s)`);
      const res = await this._hass.callWS({
        type: "call_service", domain: "ai_task", service: "generate_data", return_response: true,
        service_data: {
          task_name: "Skizze lesen", entity_id: ent, instructions: szPrompt(st, hint),
          attachments: [{ media_content_id: mediaId, media_content_type: "image/jpeg" }],
        },
      });
      let data = res?.response?.data;
      if (typeof data === "string") {
        const mt = data.match(/\{[\s\S]*\}/);
        if (!mt) throw new Error("Die KI hat kein lesbares Ergebnis geliefert: " + data.slice(0, 200));
        data = JSON.parse(mt[0]);
      }
      const fe = data.fertigung && typeof data.fertigung === "object" ? data.fertigung : null;
      this._gelesen = Array.isArray(data.masse) ? data.masse.filter((x) => x && x.text != null).slice(0, 40) : [];
      const main = szPrepAI(data, st);
      this._model = szNormalize(main, st);
      this._fert = null;
      if (fe) {
        const alt = fe.alternative && typeof fe.alternative === "object" && !["flach", "dreh", "biege", "fraes"].includes(st.typ) ? szPrepAI({ ...fe.alternative }, st) : null;
        this._fert = { empfehlung: SZ_TYPNAME[fe.empfehlung] ? fe.empfehlung : this._model.typ, begruendung: String(fe.begruendung || ""),
          alt: alt && alt.typ !== this._model.typ ? szNormalize(alt, st) : null };
      }
      if (st.fertigung !== "ki" && !["flach", "dreh", "biege", "fraes"].includes(st.typ)) this._autoFertigung();
      this._unsicher = this._model.unsicher;
      this._renderAll();
      this._status(`✓ ${SZ_TYPNAME[this._model.typ]} gezeichnet – bitte die Maße prüfen.`);
    } catch (e) {
      console.error("skizze-zeichnung", e);
      this._status(esc(e.message || String(e)), "err");
    } finally {
      btns.forEach((b) => (b.disabled = false));
      if (!this._lastBlob) this.$("again").disabled = true;
      if (mediaId) this._hass.callWS({ type: "media_source/local_source/remove", media_content_id: mediaId }).catch(() => {});
    }
  }

  // Fertigungsart automatisch wählen: KI-Empfehlung übernehmen, Biegeteile auf Kantbarkeit prüfen
  _autoFertigung() {
    const st = this._st;
    let fe = this._fert;
    if (fe && fe.alt && fe.alt.typ === fe.empfehlung && this._model.typ !== fe.empfehlung) {
      const cur = this._model; this._model = fe.alt; fe.alt = cur; fe.auto = `automatisch als ${SZ_TYPNAME[this._model.typ]} gewählt (KI-Empfehlung)`;
    }
    if (this._model.typ === "biege") {
      const ch = szBendChecks(this._model, st);
      if (!ch.length) return;
      let alt = fe && fe.alt && fe.alt.typ === "fraes" ? fe.alt : null;
      if (!alt) { const conv = szBiegeZuFraes(this._model, st); if (conv) alt = szNormalize(conv, st); }
      if (!alt) return;
      const cur = this._model;
      this._model = alt;
      this._fert = fe = { empfehlung: "fraes", begruendung: ch[0].replace(/\.$/, ""), alt: cur, auto: "automatisch als Frästeil gewählt, weil das Biegeteil kaum kantbar ist" };
    }
  }

  /* ----- Ausgabe ----- */
  _download(blob, name) {
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob); a.download = name;
    document.body.appendChild(a); a.click(); a.remove();
    setTimeout(() => URL.revokeObjectURL(a.href), 4000);
  }

  async _pngBlob() {
    const img = new Image();
    const url = URL.createObjectURL(new Blob([this._svg()], { type: "image/svg+xml" }));
    await new Promise((res, rej) => { img.onload = res; img.onerror = rej; img.src = url; });
    const dpmm = 300 / 25.4;
    const c = document.createElement("canvas");
    c.width = Math.round(297 * dpmm); c.height = Math.round(210 * dpmm);
    const g = c.getContext("2d"); g.fillStyle = "#fff"; g.fillRect(0, 0, c.width, c.height);
    g.drawImage(img, 0, 0, c.width, c.height);
    URL.revokeObjectURL(url);
    return await new Promise((res) => c.toBlob(res, "image/png"));
  }

  async _png() {
    try { this._download(await this._pngBlob(), this._fileName("png")); }
    catch (e) { this._status("PNG-Export fehlgeschlagen: " + esc(e.message || e), "err"); }
  }

  async _saveMedia() {
    try {
      this._status(`<span class="spin"></span> Speichere …`);
      const id = await this._upload(await this._pngBlob(), this._fileName("png"));
      this._status(`✓ Gespeichert in <b>Medien → Meine Medien</b> als ${esc(id.split("/").pop())}`);
    } catch (e) { this._status("Speichern fehlgeschlagen: " + esc(e.message || e), "err"); }
  }

  _print() {
    const fr = document.createElement("iframe");
    fr.style.cssText = "position:fixed;right:0;bottom:0;width:0;height:0;border:0";
    document.body.appendChild(fr);
    const d = fr.contentDocument;
    d.open();
    d.write(`<!doctype html><html><head><title>${esc(this._fileName("pdf").replace(/\.pdf$/, ""))}</title><style>@page{size:A4 landscape;margin:0}html,body{margin:0;padding:0}svg{width:297mm;height:210mm;display:block}</style></head><body>${this._svg()}</body></html>`);
    d.close();
    setTimeout(() => { fr.contentWindow.focus(); fr.contentWindow.print(); setTimeout(() => fr.remove(), 60000); }, 300);
  }
}

if (!customElements.get("skizze-zeichnung-card")) customElements.define("skizze-zeichnung-card", SkizzeZeichnungCard);
window.customCards = window.customCards || [];
if (!window.customCards.find((c) => c.type === "skizze-zeichnung-card"))
  window.customCards.push({ type: "skizze-zeichnung-card", name: "Skizze → Zeichnung", description: "Handskizze fotografieren und als technische Zeichnung ausgeben (Flach-, Dreh-, Biege- und Frästeile)" });
if (typeof module !== "undefined") module.exports = { szRender, szNormalize, szPrompt, szBendCalc, szBendChecks, szPrepAI, szBiegeZuFraes };
