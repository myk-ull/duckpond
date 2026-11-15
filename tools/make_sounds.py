"""Synthesise the duckling peeps (assets/quacks/peep1..4.wav).

The adult quacks are real recordings: see tools/cut_quacks.py and
assets/quacks/CREDITS.md.
"""
import wave
import numpy as np

SR = 22050


def peep(f0, rise, n_notes, seed):
    rng = np.random.default_rng(seed)
    parts = []
    for k in range(n_notes):
        dur = 0.06 + rng.uniform(0, 0.025)
        n = int(SR * dur)
        t = np.arange(n) / SR
        f = f0 * (1 + rise * t / dur) * (1 - 0.04 * k)
        ph = 2 * np.pi * np.cumsum(f) / SR
        s = (np.sin(ph) + 0.15 * np.sin(2 * ph)) * np.sin(np.pi * t / dur) ** 2
        parts += [s * (1 - 0.12 * k), np.zeros(int(SR * rng.uniform(0.035, 0.06)))]
    x = np.concatenate(parts)
    return x / np.max(np.abs(x)) * 0.3


for i, (f0, rise, nn) in enumerate([(2900, 0.3, 2), (3300, 0.25, 1), (2600, 0.35, 3), (3100, 0.2, 2)], 1):
    x = peep(f0, rise, nn, i)
    with wave.open(f"assets/quacks/peep{i}.wav", "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(SR)
        w.writeframes((x * 32767).astype("<i2").tobytes())
print("ok")
