"""Prüft, ob die Rust-Rechnung (core/src/ki.rs) genau das Netz aus trainieren.py ergibt.
    python abgleich.py <gewichte.bin>   (im Ordner tools/zmn-ki)
"""
import os, struct, subprocess, sys
import numpy as np, torch
sys.path.insert(0, os.path.dirname(__file__))
from trainieren import Netz

gew = os.path.abspath(sys.argv[1])
d = open(gew, "rb").read()
netz = Netz()
o = 12
for s in netz.schichten:
    aus, ein, k = struct.unpack_from("<III", d, o); o += 12
    n = aus * ein * k * k
    s.weight.data = torch.from_numpy(np.frombuffer(d, "<f4", n, o).copy()).reshape(aus, ein, k, k); o += n * 4
    s.bias.data = torch.from_numpy(np.frombuffer(d, "<f4", aus, o).copy()); o += aus * 4
w, h = 68, 44
x = torch.rand(1, 3, h, w)
ref = torch.sigmoid(netz(x))[0, 0].detach().numpy()
pfad = "/tmp/zmn_ki_abgleich.f32"
with open(pfad, "wb") as f:
    f.write(struct.pack("<II", w, h)); f.write(x[0].numpy().astype("<f4").tobytes())
core = os.path.join(os.path.dirname(os.path.abspath(__file__)), "../../core")
subprocess.run(["cargo", "test", "--release", "--lib", "ki::tests::abgleich_mit_pytorch", "--", "--ignored", "--quiet"], cwd=core, check=True,
               env={**os.environ, "ZMN_KI_GEWICHTE": gew, "ZMN_KI_TEST": pfad}, stdout=subprocess.DEVNULL)
out = np.fromfile(pfad + ".aus", "<f4").reshape(h, w)
fehler = np.abs(out - ref).max()
print(f"Größte Abweichung Rust ↔ PyTorch: {fehler:.2e}")
sys.exit(0 if fehler < 1e-4 else 1)
