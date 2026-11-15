"""Cut individual quacks out of longer duck recordings.

usage: python tools/cut_quacks.py OUT_DIR in1.wav in2.wav ...
Finds bursts of sound, keeps the quack-shaped ones (right length, energy in
the 0.6-3 kHz "nasal" band), trims with soft fades, normalises, and writes
them plus gently pitch-shifted copies. Inputs: mono 16-bit 22050 Hz wav.
"""
import sys, os, wave
import numpy as np

SR = 22050


def read(p):
    with wave.open(p) as w:
        assert w.getframerate() == SR and w.getnchannels() == 1
        return np.frombuffer(w.readframes(w.getnframes()), dtype="<i2").astype(np.float32) / 32768


def write(p, x, peak=0.6):
    x = x / (np.max(np.abs(x)) + 1e-9) * peak
    with wave.open(p, "wb") as w:
        w.setnchannels(1); w.setsampwidth(2); w.setframerate(SR)
        w.writeframes((x * 32767).astype("<i2").tobytes())


def bursts(x):
    hop = int(SR * 0.01)
    rms = np.array([np.sqrt(np.mean(x[i:i + hop] ** 2)) for i in range(0, len(x) - hop, hop)])
    floor = np.percentile(rms, 30)
    thr = max(floor * 4, np.percentile(rms, 75) * 0.6)
    on = rms > thr
    out, i = [], 0
    while i < len(on):
        if on[i]:
            j = i
            gap = 0
            while j < len(on) and (on[j] or gap < 3):
                gap = 0 if on[j] else gap + 1
                j += 1
            out.append((i * hop, (j - gap) * hop, rms[i:j].max()))
            i = j
        else:
            i += 1
    return out


def quackiness(seg):
    sp = np.abs(np.fft.rfft(seg * np.hanning(len(seg))))
    fr = np.fft.rfftfreq(len(seg), 1 / SR)
    band = sp[(fr > 600) & (fr < 3000)].sum() / (sp.sum() + 1e-9)
    return band


def resample(x, factor):
    # pitch/tempo shift by resampling (factor > 1 = higher, shorter)
    n = int(len(x) / factor)
    return np.interp(np.linspace(0, len(x) - 1, n), np.arange(len(x)), x)


def main():
    out_dir, inputs = sys.argv[1], sys.argv[2:]
    os.makedirs(out_dir, exist_ok=True)
    picks = []
    for p in inputs:
        x = read(p)
        # gentle high-pass to drop wind / handling rumble
        x = x - np.convolve(x, np.ones(64) / 64, mode="same")
        for a, b, peak in bursts(x):
            dur = (b - a) / SR
            if not (0.07 <= dur <= 0.45):
                continue
            pre = int(SR * 0.012)
            seg = x[max(0, a - pre): b + int(SR * 0.03)].copy()
            q = quackiness(seg)
            if q < 0.45:
                continue
            fade_in = np.minimum(1, np.arange(len(seg)) / (SR * 0.006))
            fade_out = np.minimum(1, (len(seg) - np.arange(len(seg))) / (SR * 0.035))
            seg *= fade_in * fade_out
            picks.append((q * peak, os.path.basename(p), a / SR, seg))
    picks.sort(key=lambda t: -t[0])
    # keep the best, but no more than 8 per source so voices vary
    kept, per = [], {}
    for score, src, t, seg in picks:
        if per.get(src, 0) >= 8:
            continue
        per[src] = per.get(src, 0) + 1
        kept.append((src, t, seg))
        if len(kept) >= 14:
            break
    n = 0
    for src, t, seg in kept:
        for shift in (1.0, 1.12, 0.92):
            n += 1
            write(os.path.join(out_dir, f"q{n:02d}.wav"), resample(seg, shift))
        print(f"{src} @ {t:.2f}s  ({len(seg) / SR:.2f}s)")
    print(n, "quacks")


main()
