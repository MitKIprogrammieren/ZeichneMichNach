"""Kanten-KI für ZeichneMichNach trainieren.

    python trainieren.py <datenordner> <ausgabe.bin> [--epochen 25] [--schnelltest]

Kleines U-Netz (3 Stufen, 12/24/48 Kanäle, nur 3×3-Faltungen, ReLU, Max-Pooling,
Nächster-Nachbar-Vergrößerung). Genau dieser Aufbau wird in core/src/ki.rs ohne
Fremdbibliothek nachgerechnet – Änderungen hier müssen dort gespiegelt werden.

Ausgabe: Gewichte im Format ZMNK (siehe speichern()).
"""
import argparse, glob, math, os, struct, sys, time

import numpy as np
import torch
import torch.nn as nn
import torch.nn.functional as F
from PIL import Image

KANAELE = (12, 24, 48)


class Netz(nn.Module):
    def __init__(self):
        super().__init__()
        a, b, c = KANAELE
        k = lambda i, o: nn.Conv2d(i, o, 3, padding=1)
        # Reihenfolge = Reihenfolge in der Datei
        self.schichten = nn.ModuleList([
            k(3, a), k(a, a),          # e1
            k(a, b), k(b, b),          # e2
            k(b, c), k(c, c),          # Mitte
            k(c + b, b), k(b, b),      # d2
            k(b + a, a), k(a, a),      # d1
            nn.Conv2d(a, 1, 1),        # Ausgabe
        ])

    def forward(self, x):
        s = self.schichten
        e1 = F.relu(s[1](F.relu(s[0](x))))
        e2 = F.relu(s[3](F.relu(s[2](F.max_pool2d(e1, 2)))))
        m = F.relu(s[5](F.relu(s[4](F.max_pool2d(e2, 2)))))
        d2 = torch.cat([F.interpolate(m, scale_factor=2, mode="nearest"), e2], 1)
        d2 = F.relu(s[7](F.relu(s[6](d2))))
        d1 = torch.cat([F.interpolate(d2, scale_factor=2, mode="nearest"), e1], 1)
        d1 = F.relu(s[9](F.relu(s[8](d1))))
        return s[10](d1)


def laden(ordner, gr=None):
    ein = sorted(glob.glob(os.path.join(ordner, "*_ein.png")))
    xs, ys = [], []
    for f in ein:
        k = f.replace("_ein.png", "_kante.png")
        if not os.path.exists(k):
            continue
        xs.append(np.asarray(Image.open(f).convert("RGB")))
        ys.append(np.asarray(Image.open(k).convert("L")))
    if not xs:
        sys.exit(f"Keine Trainingsdaten in {ordner}")
    x = torch.from_numpy(np.stack(xs)).permute(0, 3, 1, 2).contiguous()  # uint8 N,3,H,W
    y = torch.from_numpy((np.stack(ys) > 127).astype(np.uint8))[:, None]
    return x, y


def speichern(netz, pfad):
    """ZMNK-Format, little endian:
    'ZMNK' u32 version=1 u32 anzahl_schichten
    je Schicht: u32 aus u32 ein u32 k, dann aus*ein*k*k f32 Gewichte (aus, ein, ky, kx), dann aus f32 Bias
    """
    schichten = list(netz.schichten)
    with open(pfad, "wb") as f:
        f.write(b"ZMNK")
        f.write(struct.pack("<II", 1, len(schichten)))
        for s in schichten:
            w = s.weight.detach().cpu().float().numpy()
            b = s.bias.detach().cpu().float().numpy()
            o, i, k, _ = w.shape
            f.write(struct.pack("<III", o, i, k))
            f.write(w.astype("<f4").tobytes())
            f.write(b.astype("<f4").tobytes())


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("daten")
    ap.add_argument("ausgabe")
    ap.add_argument("--epochen", type=int, default=25)
    ap.add_argument("--stapel", type=int, default=32)
    ap.add_argument("--schnelltest", action="store_true", help="nur ein paar Schritte – prüft, ob alles läuft")
    a = ap.parse_args()

    if torch.backends.mps.is_available():
        dev = torch.device("mps")
        print("Rechne auf dem Grafikchip (Apple MPS)")
    elif torch.cuda.is_available():
        dev = torch.device("cuda")
        print("Rechne auf der Grafikkarte (CUDA)")
    else:
        dev = torch.device("cpu")
        torch.set_num_threads(os.cpu_count() or 4)
        print("Rechne auf dem Prozessor")

    x, y = laden(a.daten)
    n = x.shape[0]
    nval = max(1, n // 20)
    perm = torch.randperm(n, generator=torch.Generator().manual_seed(1))
    vi, ti = perm[:nval], perm[nval:]
    print(f"{n} Beispiele ({len(ti)} Training, {nval} Prüfung), Kantenanteil {y.float().mean().item() * 100:.1f} %")

    torch.manual_seed(1)
    netz = Netz().to(dev)
    print(f"{sum(p.numel() for p in netz.parameters())} Gewichte")
    opt = torch.optim.AdamW(netz.parameters(), lr=2e-3, weight_decay=1e-4)
    epochen = 1 if a.schnelltest else a.epochen
    schritte_je = math.ceil(len(ti) / a.stapel)
    if a.schnelltest:
        schritte_je = min(schritte_je, 3)
    sched = torch.optim.lr_scheduler.OneCycleLR(opt, max_lr=3e-3, total_steps=epochen * schritte_je)
    pos = torch.tensor([4.0], device=dev)

    def stapel(idx, aug):
        xb = x[idx].to(dev).float() / 255.0
        yb = y[idx].to(dev).float()
        if aug:
            if torch.rand(1).item() < 0.5:
                xb, yb = xb.flip(3), yb.flip(3)
            if torch.rand(1).item() < 0.5:
                xb, yb = xb.flip(2), yb.flip(2)
            k = int(torch.randint(0, 4, (1,)).item())
            xb, yb = torch.rot90(xb, k, (2, 3)), torch.rot90(yb, k, (2, 3))
            # leichte Farbverschiebung, damit das Netz Kanten und nicht Farben lernt
            xb = (xb * (0.8 + 0.4 * torch.rand(xb.shape[0], 3, 1, 1, device=dev)) + 0.1 * (torch.rand(xb.shape[0], 1, 1, 1, device=dev) - 0.5)).clamp(0, 1)
        return xb, yb

    def verlust(logit, yb):
        bce = F.binary_cross_entropy_with_logits(logit, yb, pos_weight=pos)
        p = torch.sigmoid(logit)
        dice = 1 - (2 * (p * yb).sum() + 1) / (p.sum() + yb.sum() + 1)
        return bce + dice

    bestes = None
    for ep in range(epochen):
        netz.train()
        t0 = time.time()
        reihen = ti[torch.randperm(len(ti))]
        summe = 0.0
        for s in range(schritte_je):
            idx = reihen[s * a.stapel:(s + 1) * a.stapel]
            if len(idx) == 0:
                break
            xb, yb = stapel(idx, True)
            l = verlust(netz(xb), yb)
            opt.zero_grad()
            l.backward()
            opt.step()
            sched.step()
            summe += l.item()
        # Prüfung: F1 der Kanten (mit 1 px Toleranz)
        netz.eval()
        tp = fp = fn = 0.0
        with torch.no_grad():
            for s in range(0, nval, a.stapel):
                xb, yb = stapel(vi[s:s + a.stapel], False)
                p = (torch.sigmoid(netz(xb)) > 0.5).float()
                yd = F.max_pool2d(yb, 3, 1, 1)
                pd = F.max_pool2d(p, 3, 1, 1)
                tp += (p * yd).sum().item()
                fp += (p * (1 - yd)).sum().item()
                fn += (yb * (1 - pd)).sum().item()
        prec = tp / max(tp + fp, 1)
        rec = (tp) / max(tp + fn, 1)
        f1 = 2 * prec * rec / max(prec + rec, 1e-9)
        print(f"Epoche {ep + 1}/{epochen}: Verlust {summe / max(schritte_je, 1):.3f}  Genauigkeit {prec * 100:.0f} %  Trefferquote {rec * 100:.0f} %  F1 {f1 * 100:.1f} %  ({time.time() - t0:.0f} s)", flush=True)
        if bestes is None or f1 > bestes:
            bestes = f1
            speichern(netz, a.ausgabe)
    print(f"Gespeichert: {a.ausgabe} (bestes F1 {bestes * 100:.1f} %)")


if __name__ == "__main__":
    main()
