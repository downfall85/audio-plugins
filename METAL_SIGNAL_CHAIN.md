# Metal Signal Chain — Settings Per Style

## Signal Chain (All Styles)

```
Guitar → [1] Tuner → [2] Noise Gate → [3] Overdrive (boost)
       → [4] AIDA-X (amp + cab) → [5] Presence EQ
       → [6] Noise Gate → [7] Delay → [8] Reverb → Output
```

---

## Iron Maiden (NWOBHM / Classic Metal)

Target: Marshall JCM800-style crunch, defined midrange, slightly loose feel.
Recommended AIDA-X model: **Marshall JCM800 or JTM45**

| Plugin | Parameter | Value |
|---|---|---|
| **Noise Gate** (pre) | Threshold | -55 dB |
| | Attack / Release | 5 ms / 150 ms |
| **Overdrive** (boost) | Drive | 5–8 dB |
| | Tone | 2.5 kHz |
| | Output | -3 dB |
| | Mix | 100% |
| **AIDA-X** | Amp gain | ~50–60% |
| | Bass / Mid / Treble | 6 / 7 / 7 |
| | Presence | 60% |
| **Presence EQ** | HP Freq | 80 Hz |
| | Mid Freq / Gain | 1.2 kHz / +3 dB |
| | LP Freq | 6.5 kHz |
| **Noise Gate** (post) | Threshold | -65 dB |
| | Release | 200 ms |
| **Delay** | Time | 380 ms (dotted 8th at tempo) |
| | Feedback | 20% |
| | Mix | 18% |
| **Reverb** | Mix | 20% |

---

## Modern American Metal (Metallica, Pantera, Lamb of God)

Target: Tight, punchy, slightly scooped, Mesa/Boogie or 5150-style.
Recommended AIDA-X model: **Mesa Boogie Rectifier, 5150, or Diezel**

| Plugin | Parameter | Value |
|---|---|---|
| **Noise Gate** (pre) | Threshold | -50 dB |
| | Attack / Release | 3 ms / 100 ms |
| **Overdrive** (boost) | Drive | 8–12 dB |
| | Tone | 1.8 kHz |
| | Output | -2 dB |
| | Mix | 100% |
| **AIDA-X** | Amp gain | 70–80% |
| | Bass | 5 |
| | Mid | 4–5 (slight scoop) |
| | Treble | 7 |
| | Presence | 65% |
| **Presence EQ** | HP Freq | 100 Hz |
| | Mid Freq / Gain | 900 Hz / -2 dB |
| | LP Freq | 7 kHz |
| **Noise Gate** (post) | Threshold | -60 dB |
| **Delay** | Time | 250 ms |
| | Feedback | 15% |
| | Mix | 10% |
| **Reverb** | Mix | 10% |

---

## Extreme / Technical Metal (Arch Spire, Jeff Loomis, Meshuggah)

Target: Maximum tightness, surgical low end, zero mud. Every note must articulate even at 200+ BPM.
Recommended AIDA-X model: **5150 / Diezel Herbert / Engl Savage**

| Plugin | Parameter | Value |
|---|---|---|
| **Noise Gate** (pre) | Threshold | -45 dB |
| | Attack | 1 ms |
| | Release | 60 ms |
| **Overdrive** (boost) | Drive | 15–20 dB |
| | Tone | 1.5 kHz |
| | Output | 0 dB |
| | Mix | 100% |
| **AIDA-X** | Amp gain | 60–70% |
| | Bass | 3–4 |
| | Mid | 5 |
| | Treble | 7–8 |
| | Presence | 70% |
| **Presence EQ** | HP Freq | 110 Hz |
| | Mid Freq / Gain | 3 kHz / +2 dB |
| | LP Freq | 7 kHz |
| | Output | -1 dB |
| **Noise Gate** (post) | Threshold | -55 dB |
| | Attack | 1 ms |
| | Release | 40 ms |
| **Delay** | Mix | 5% |
| **Reverb** | Mix | 5% |

---

## Key Tip: Overdrive vs Amp Gain Balance

The **Overdrive drive level relative to AIDA-X amp gain** is the main tone knob:

- Overdrive high + Amp gain moderate = **tight, modern, controlled** (extreme metal)
- Overdrive low + Amp gain high = **loose, saturated, rock** (classic metal)
- Overdrive medium + Amp gain medium = **balanced** (Metallica territory)

## AIDA-X Model Sources

Community models at [aida-x.cc](https://aida-x.cc) and the MOD Forum:

| Model | Best For |
|---|---|
| 5150 / 5153 (EVH) | Tightest modern metal, extreme genres |
| Mesa Boogie Dual/Triple Rectifier | Punchy with low-mid weight |
| Diezel Herbert / VH4 | Articulate high gain, complex harmonics |
| Marshall JCM800 | Classic rock through NWOBHM |
| Engl Savage 120 | Jeff Loomis signature tones |
