# Dough Reference — Blueprint for SoW Live Music Engine

Source: https://dough.strudel.cc/ (Codeberg: uzu/dough)
License: AGPL-3.0 (compatible with our project)
Language: Single C file (dough.c), compiles native + WASM

---

## 1. Architecture Overview

Dough is a **monolithic, fixed-path, semi-modular synth** remote-controlled via key/value messages.
Every sound is a "voice" that flows through an identical signal chain:

```
Oscillator → Gain Envelope → Filter → Per-voice FX → Pan → [Delay Send] → [Reverb Send] → Mix
                                                       ↑
                                           FM / AM / RM / Vibrato
```

Key design principles:
- **Single signal path per voice** — no routing graph, no patch cables
- **Parameter-driven** — every aspect controllable via flat key/value pairs
- **Envelopes attach to many targets** — filters, pitch, FM, not just gain
- **Voice allocation is automatic** — unless you pin a voice with `/voice/N`
- **Stateless messages** — each message carries its full intent, no prior state required

---

## 2. Signal Chain (per voice)

### 2.1 Oscillators

| Sound   | Description                                    | Param |
|---------|------------------------------------------------|-------|
| `sine`  | Pure sine wave                                 | `/sound/sine` |
| `tri`   | Triangle wave (default)                       | `/sound/tri` |
| `saw`   | Sawtooth (ramp up)                             | `/sound/saw` |
| `zaw`   | Reverse sawtooth (ramp down)                   | `/sound/zaw` |
| `pulse` | Pulse/square with variable width               | `/sound/pulse` + `/pw/0.3` |
| `pulze` | Antialiased pulse (band-limited)               | `/sound/pulze` |
| `white` | White noise                                    | `/sound/white` |
| `pink`  | Pink noise (1/f)                                | `/sound/pink` |
| `brown` | Brown noise (1/f²)                              | `/sound/brown` |
| `const` | Always 1.0 (useful for testing envelopes)       | `/sound/const` |
| samples | Sample playback via `s/` prefix                 | `/s/crate_bd` |

Default: `tri`

**Frequency control:**
- `/freq/440` — direct Hz (wins over note if both set)
- `/note/60` — MIDI note number (60 = middle C)
- `/speed/1.0` — multiplier on source freq / playback speed

**Pitch modulation:**
- `/vib/8` — vibrato LFO rate in Hz
- `/vibmod/2` — vibrato depth in semitones
- `/penv/24` — pitch envelope amount (semitones)
- `/patt/0.2` — pitch envelope attack time
- `/pdec/0.2` — pitch envelope decay time

**Pulse width:**
- `/pw/0.5` — 0 to 1, default 0.5 (square). Only affects `pulse` and `pulze`.

### 2.2 Gain Envelope (ADSR)

```
attack → decay → sustain → release
```

| Param      | Default | Description                   |
|------------|---------|-------------------------------|
| `/attack`  | 0.001   | Attack time in seconds        |
| `/decay`   | 0       | Decay time in seconds         |
| `/sustain`  | 1.0    | Sustain level (0–1)           |
| `/release`  | 0.001  | Release time in seconds       |
| `/gain`     | 0.5    | Pre-FX gain                   |
| `/postgain` | 1.0    | Post-FX gain                  |
| `/velocity` | 1.0    | Multiplies with gain          |

**Duration / gating:**
- `/duration/0.5` — gate duration in seconds. If not set, voice plays until released.
- `/time/0` — start time offset (seconds from now)
- `/repeat/8` — repeat the command within N seconds

### 2.3 FM Synthesis

Dough implements **2-operator FM**: carrier + modulator.

```
carrier_freq = freq (or note→freq)
modulator_freq = freq * fmh
modulation_index = fm * fm_env(t)
output = carrier(modulator(t) * fm_index)
```

| Param   | Default | Description                                |
|---------|---------|--------------------------------------------|
| `/fm`   | 0       | FM index (modulation depth)                |
| `/fmh`  | 1.0     | Harmonic ratio (mod_freq = freq × fmh)     |
| `/fme`  | 1.0     | FM envelope amount                         |
| `/fma`  | 0       | FM envelope attack (seconds)               |
| `/fmd`  | 0       | FM envelope decay (seconds)                |
| `/fms`  | 1.0     | FM envelope sustain (0–1)                  |
| `/fmr`  | 0       | FM envelope release (seconds)              |

**Key insight for game music:** `fmh` close to simple ratios (1, 2, 1.5, etc.) sounds harmonic.
Slightly detuned ratios (1.001, 2.01) create beating/drift — perfect for tension.

### 2.4 AM / RM (Amplitude & Ring Modulation)

| Param       | Default | Description                     |
|-------------|---------|---------------------------------|
| `/am`       | 0       | AM frequency in Hz              |
| `/amdepth`  | 0.5     | AM depth (0–1)                  |
| `/rm`       | 0       | Ring mod frequency in Hz        |
| `/rmdepth`  | 0.5     | Ring mod depth (0–1, 1=full RM) |

**Game music use:** AM creates tremolo (slow) or metallic tones (fast).
RM creates inharmonic bell/metal textures.

### 2.5 Filters (LP / HP / BP)

Each filter type has a full ADSR envelope on its cutoff frequency.

**Low Pass Filter:**

| Param  | Default    | Description                        |
|--------|------------|------------------------------------|
| `/lpf` | 20000      | Cutoff frequency (Hz)              |
| `/lpq` | 1.0        | Resonance (0–16)                   |
| `/lpe` | 0          | Envelope amount                    |
| `/lpa` | 1.0        | Envelope attack (seconds)          |
| `/lpd` | 1.0        | Envelope decay (seconds)           |
| `/lps` | 1.0        | Envelope sustain (0–1)             |
| `/lpr` | 1.0        | Envelope release (seconds)         |

**High Pass Filter:** Same pattern with `hp*` prefix.

| Param  | Default |
|--------|---------|
| `/hpf` | 100     |
| `/hpq` | 0       |

**Band Pass Filter:** Same pattern with `bp*` prefix.

| Param  | Default |
|--------|---------|
| `/bpf` | 1       |
| `/bpq` | 1       |

**Filter slope:**

| `/ftype` | Value    | Description          |
|----------|----------|----------------------|
| `0`      | `12db`   | 12 dB/oct (2-pole)   |
| `1`      | `24db`   | 24 dB/oct (4-pole)   |
| `2`      | `48db`   | 48 dB/oct (8-pole)   |

**Envelope behavior (same for lp/hp/bp):**
- Set only `lpa` (no `lpd`) → sustain = 1 (opens up)
- Set only `lpd` (no `lpa`) → sustain = 0 (closes down)
- Set both `lpa` + `lpd` → sustain = 0 (closes down)
- Set `lps` explicitly → overrides default

**Example: Filter sweep on combat intensity**
```
/lpf/200/lpe/5/lpd/0.6     → filter opens then decays
/lpf/200/lpe/5/lpa/0.3     → filter swells in from below
/lpf/200/lpe/5/lpa/0.2/lpd/0.4  → opens then closes
```

### 2.6 Per-voice FX

| Param          | Default | Description                         |
|----------------|---------|--------------------------------------|
| `/coarse`      | 1       | Sample rate divider (aliasing FX)    |
| `/crush`       | 32      | Bit crusher (bits, lower = more)     |
| `/distort`     | 0       | Waveshaping distortion (0–10+)       |
| `/distortvol`  | 1.0     | Post-distortion gain                 |

**Game music use:**
- `coarse/4` → lo-fi/retro textures (great for 8-bit aesthetic)
- `crush/4` → crunchy, degraded quality
- `distort/3` → warm overdrive (good for bass)
- `distort/8` → aggressive (combat music)

### 2.7 Stereo / Panning

| Param  | Default | Description                     |
|--------|---------|---------------------------------|
| `/pan` | 1.0     | 0=left, 0.5=center, 1=right    |

Note: dough's pan is mono→stereo. SoW SFX already does spatial panning better.
For music, we want stereo width/spread, not spatial positioning.

---

## 3. Global FX Sends

These are **shared across all voices** (not per-voice):

### 3.1 Delay

| Param             | Default | Description                    |
|-------------------|---------|--------------------------------|
| `/delay`          | 0.5     | Send level (0–1)               |
| `/delayfeedback`  | 0.5     | Feedback (0–0.999)             |
| `/delaytime`      | 0.12    | Delay time in seconds          |

**Game music use:** Dotted-eighth delays, slapback, feedback washes.

### 3.2 Reverb

| Param            | Default | Description                        |
|------------------|---------|------------------------------------|
| `/verb`          | 0.5     | Send level (0–1)                   |
| `/verbdecay`     | 1.0     | Tail length (0–1)                  |
| `/verbdamp`      | 1.0     | HF damping (0–1, lower = darker)  |
| `/verbpredelay`  | 1.0     | Pre-delay (0–1)                    |
| `/verbdiff`      | 1.0     | Diffusion / early reflection density|

---

## 4. Voice Management

| Param    | Description                                          |
|----------|------------------------------------------------------|
| `/voice/N` | Pin to voice index N (skip auto-allocation)        |
| `/reset/1` | Reset voice even if active (retrigger envelopes)   |
| `/glide/0.5` | Pitch slide time when freq changes on active voice |

**Game music use:** Pin bass to voice 0, lead to voice 1, pads to voice 2, etc.
This lets the game update active voices in real-time without cutting them off.
`glide` creates portamento — essential for smooth melodic movement.

**Voice groups** (from keyboard example): `group` field lets you release by group.
Equivalent to "note off" for a specific held key.

---

## 5. Full Parameter Reference Table

| Param          | Default  | Min    | Max      | Unit      |
|----------------|----------|--------|----------|-----------|
| sound          | tri      | -      | -        | enum      |
| freq           | -        | 20     | 20000    | Hz        |
| note           | -        | 0      | 127      | MIDI      |
| speed          | 1.0      | 0.1    | 10       | ×         |
| time           | 0        | 0      | ∞        | seconds   |
| duration       | ∞        | 0      | ∞        | seconds   |
| repeat         | -        | -      | -        | seconds   |
| attack         | 0.001    | 0      | 2+       | seconds   |
| decay          | 0        | 0      | 2+       | seconds   |
| sustain        | 1.0      | 0      | 1        | level     |
| release        | 0.001    | 0      | 3+       | seconds   |
| gain           | 0.5      | 0      | 1        | level     |
| postgain       | 1.0      | 0      | 1        | level     |
| velocity       | 1.0      | 0      | 1        | level     |
| pw             | 0.5      | 0      | 1        | ratio     |
| voice          | auto     | 0      | N        | index     |
| reset          | 0        | 0      | 1        | bool      |
| glide          | 0        | 0      | ∞        | seconds   |
| penv           | 0        | 0      | 12+      | semitones |
| patt           | 0        | 0      | 1+       | seconds   |
| pdec           | 0        | 0      | 1+       | seconds   |
| vib            | 0        | 0      | 16       | Hz        |
| vibmod         | 0        | 0      | 4+       | semitones |
| fm             | 0        | 0      | 12+      | index     |
| fmh            | 1.0      | 0.1    | 8        | ratio     |
| fme / fmenv    | 1.0      | 0      | 8        | amount    |
| fma            | 0        | 0      | 2        | seconds   |
| fmd            | 0        | 0      | 2        | seconds   |
| fms            | 1.0      | 0      | 1        | level     |
| fmr            | 0        | 0      | 2        | seconds   |
| am             | 0        | 0      | 16       | Hz        |
| amdepth        | 0.5      | 0      | 1        | depth     |
| rm             | 0        | 0      | 220      | Hz        |
| rmdepth        | 0.5      | 0      | 1        | depth     |
| phaser         | 0        | 0      | 16       | Hz        |
| phaserdepth    | 0.5      | 0      | 1        | depth     |
| phasersweep    | 2000     | 100    | 8000     | Hz        |
| phasercenter   | 2000     | 100    | 8000     | Hz        |
| flanger        | 2000     | 100    | 8000     | Hz        |
| flangerdepth   | 0.5      | 0      | 1        | depth     |
| flangerfeedback| 0        | 0      | 0.95     | ratio     |
| chorus         | 1.0      | 0      | 1        | Hz        |
| chorusdepth    | 1.0      | 0      | 1        | depth     |
| chorusdelay    | 20       | 1      | 100      | ms        |
| lpf            | 20000    | 1      | 20000    | Hz        |
| lpq            | 1.0      | 0      | 16       | Q         |
| lpe            | 0        | 0      | 8        | amount    |
| lpa            | 1.0      | 0      | 2        | seconds   |
| lpd            | 1.0      | 0      | 2        | seconds   |
| lps            | 1.0      | 0      | 1        | level     |
| lpr            | 1.0      | 0      | 2        | seconds   |
| hpf            | 100      | 100    | 20000    | Hz        |
| hpq            | 0        | 0      | 16       | Q         |
| hpe            | 1.0      | 0      | 8        | amount    |
| hpa–hpr        | 1.0      | 0      | 2        | seconds   |
| bpf            | 1        | 0      | 20000    | Hz        |
| bpq            | 1        | 0      | 16       | Q         |
| bpe–bpr        | 1.0      | 0      | 2        | seconds   |
| ftype          | 12db     | -      | -        | enum      |
| coarse         | 1        | 1      | 32       | divisor   |
| crush          | 32       | 0      | 32       | bits      |
| distort        | 0        | 0      | 5+       | amount    |
| distortvol     | 1.0      | 0      | 1        | level     |
| pan            | 1.0      | 0      | 1        | position  |
| delay          | 0.5      | 0      | 1        | send      |
| delayfeedback  | 0.5      | 0      | 0.999    | ratio     |
| delaytime      | 0.12     | 0.001  | 1        | seconds   |
| verb           | 0.5      | 0      | 1        | send      |
| verbdecay      | 1.0      | 0      | 1        | amount    |
| verbdamp       | 1.0      | 0      | 1        | amount    |
| verbpredelay   | 1.0      | 0      | 1        | amount    |
| verbdiff       | 1.0      | 0      | 1        | amount    |
| begin          | 0        | 0      | 1        | ratio (samples only) |
| end            | 1        | 0      | 1        | ratio (samples only) |

---

## 6. Envelope System Design

Dough has **5 independent envelope targets**, each with full ADSR:

| Target     | Env Amount | Attack | Decay | Sustain | Release |
|------------|------------|--------|-------|---------|---------|
| Gain       | N/A        | attack | decay | sustain | release |
| Low Pass   | lpe        | lpa    | lpd   | lps     | lpr     |
| High Pass  | hpe        | hpa    | hpd   | hps     | hpr     |
| Band Pass  | bpe        | bpa    | bpd   | bps     | bpr     |
| FM Index   | fme        | fma    | fmd   | fms     | fmr     |
| Pitch      | penv       | patt   | pdec  | (N/A)   | (N/A)   |

**How envelope amount works:**

For filter envelopes, `lpe` scales the cutoff:
```
effective_cutoff = lpf + lpe * lpf * envelope_value
```
When `lpe=5` and `lpf=200`:
- At envelope peak: cutoff = 200 + 5×200 = 1200 Hz (opens way up)
- At envelope sustain=0: cutoff returns to 200 Hz (base value)

For FM envelope, `fme` scales the FM index:
```
effective_fm = fm * fme * envelope_value
```

**Smart sustain defaults:**
- If only attack set → sustain = 1 (envelope opens up, stays open)
- If only decay set → sustain = 0 (envelope decays to zero)
- If both set → sustain = 0 (closes down)
- Explicit sustain overrides

**Game music implication:** Any parameter can have its own temporal shape.
Combat → filter opens fast then decays.
Ambient → filter slowly swells in.
Nuke → pitch envelope swoops up 24 semitones.

---

## 7. FX Chain Detail

### 7.1 Phaser
LFO sweeps notch filters across the spectrum.
- Slow rate (0.5 Hz) → sweeping, spacious
- Fast rate (4 Hz) → warbly, underwater
- `phasersweep` controls how wide the sweep is in Hz
- `phasercenter` is the midpoint frequency

### 7.2 Flanger
Short delay modulated by LFO creates comb filtering.
- Creates whooshy, metallic sweeps
- `flangerfeedback` controls resonance (0.7–0.9 = metallic, <0.3 = subtle)

### 7.3 Chorus
Slightly detuned copies create thickness/width.
- Slow rate (0.05–0.3 Hz) → lush pads
- `chorusdelay` in ms controls base delay (20ms = standard, 30ms = wider)

### 7.4 Delay
Simple feedback delay line.
- `delaytime/0.12` → ~120ms slapback
- `delaytime/0.375` → dotted eighth at ~120 BPM
- `delayfeedback/0.8` → long trailing washes

### 7.5 Reverb
Freeverb-style algorithmic reverb (Schroeder).
- `verbdecay` 0–1 → short room to infinite wash
- `verbdamp` → low = dark/muffled, high = bright
- `verbdiff` → high = dense/smooth, low = sparse/grainy

---

## 8. Math Primitives (from dough.c bakery articles)

Dough implements its own approximations for `sin`, `exp2`, `log2` — no stdlib math needed.
This enables barebones/nostdlib builds and WASM with minimal deps.

### 8.1 sin(x) — Parabolic → Rational Padé

**Fast version** (parabolic sine, max error ~0.056):
```c
// Wrap x into [-pi, pi]
x += PI; x *= (0.5/PI); x -= floor(x); x *= (2*PI); x -= PI;
return 0.40528473456935094 * x * (PI - fabs(x));
```

**Accurate version** (rational Padé, max error ~1e-7):
```c
// Triangle wave input for symmetry
x = (4*fabs(x*(0.5/PI) - floor(x*(0.5/PI) + 0.75) + 0.25) - 1) * (PI/2);
const float c2 = 445.0/12122.0, c3 = -2363.0/18183.0;
const float c4 = 601.0/872784.0, c5 = 12671.0/4363920.0;
const float c6 = 121.0/16662240.0;
float xx = x*x;
return x*(1 + xx*(c3 + xx*c5)) / (1 + xx*(c2 + xx*(c4 + xx*c6)));
```

### 8.2 exp2(x) — Float bit hack + polynomial

```c
// Scale x into large integer, reinterpret as float exponent
float result = u32_to_f32((int)(x + 127) << 23);
// Correction: multiply by polynomial for fractional part
```

### 8.3 log2(x) — Float bit hack + Padé

```c
// Extract exponent bits → stairstep log2
int stairstep = (f32_to_u32(x) >> 23) - 127;
// Compress mantissa into [1, 2] range
float xc = u32_to_f32((f32_to_u32(x) & 0x7FFFFF) | (127 << 23));
// Padé approximant for log2 on [1, 2]:
float f0 = xc;
return stairstep - 124.22544637 - 1.498030302*f0 - 1.72587999/(0.3520887068 + f0);
```

### 8.4 pow(a, b) = exp2(b * log2(a))

For constant base (like `pow(10, y)`):
```c
// pow(10, y) = exp2(y * 3.321928094887362)
```
This gives **6× speedup** vs. calling `pow()`.

**Key takeaway for SoW:** We can do all audio math without libm.
`libm = "0.2"` is already in our workspace deps, but the bit-hack approach
means we could compile a nostdlib WASM audio engine with zero deps.

---

## 9. Pattern / Sequencing Model

Dough itself does NOT have a built-in sequencer. It's triggered by external messages.
Strudel (the live-coding language) generates timed events that dough plays.

**Pattern mechanism (from strudel example):**
1. Strudel queries a time range `[a, b]` against a pattern
2. Pattern returns "hap" objects with onset times + durations
3. Each hap becomes a dough `play` message with timing

**For SoW game music, we need our own pattern engine:**
- Euclidean rhythms for percussion
- Arpeggiator patterns for melodic content
- Chord sequencing (I–V–vi–IV type progressions)
- Periodic triggers at BPM-synced intervals

**What dough gives us for free:**
- `/time/T` — schedule a voice T seconds from now
- `/repeat/N` — repeat the event every N seconds
- `/duration/D` — how long the note holds

So the game just needs to emit timed messages like:
```rust
music.send(Play { time: 0.0, note: 36, duration: 0.5, sound: Saw, repeat: 2.0, .. });
music.send(Play { time: 0.5, note: 48, duration: 0.25, sound: Pulse, repeat: 2.0, .. });
```
And dough handles the voice allocation, synthesis, and mixing.

---

## 10. SoW Current Audio vs Dough — Feature Gap

### What sow-audio already has:
- ✅ Warm harmonic oscillator (sine + 2nd + 3rd harmonics)
- ✅ ADSR-like envelopes (attack, decay, tail fade)
- ✅ Pitch bends / frequency sweeps
- ✅ Vibrato (in WarHorn source)
- ✅ Multi-voice mixing (3 voices, priority system)
- ✅ Spatial stereo panning + distance attenuation
- ✅ Volume ducking per voice count
- ✅ Pentatonic scale system + per-tile melodic context
- ✅ Simple arpeggiator (ArpeggioSource)
- ✅ Pulse / sweep / noise sources for SFX
- ✅ Sample-rate: 22050 Hz (low, mobile-friendly)
- ✅ Thread-based audio worker with mpsc channel

### What sow-audio is MISSING for live music:

| Feature                    | Priority | Notes                                    |
|----------------------------|----------|------------------------------------------|
| **Filters (LP/HP/BP)**    | Critical | No filters at all. dough has 3 with ADSR |
| **FM synthesis**           | Critical | Only additive harmonics. FM = richness   |
| **Filter envelopes**       | Critical | Central to dough's sound-shaping         |
| **Waveform selection**     | High     | Only warm_at(). Need saw, tri, pulse, sq |
| **Per-voice FX**          | High     | No crush, distort, coarse               |
| **Global reverb**          | High     | No reverb at all. Essential for music    |
| **Global delay**           | High     | No delay. Core musical effect           |
| **Chorus/Flanger/Phaser**  | Medium   | Adds depth, motion, space               |
| **AM/RM**                  | Medium   | Metallic/bell textures                   |
| **Voice pinning + glide**  | High     | No way to hold a voice and morph it      |
| **Parameter smoothing**   | High     | No interpolation (clicks on changes)     |
| **Pattern/sequencer**      | High     | Only ArpeggioSource (4 notes, no rhythm) |
| **Music Director**         | High     | No game-state→music mapping at all       |
| **WASM audio output**     | High     | rodio is native-only. Need Web Audio     |

### What sow-audio has that dough DOESN'T:
- ✅ Spatial 3D panning + distance attenuation
- ✅ Priority-based voice stealing
- ✅ Game-event-driven triggering (combat, building, nuke, death)
- ✅ Per-tile melodic context (pentatonic scale from map position)
- ✅ Troops→amplitude mapping
- ✅ Rate limiting (combat + bunker sound spam prevention)

---

## 11. Game Music Architecture Proposal

### 11.1 Music Director Pattern

```
Game State → Music Director → Parameter Messages → Synth Voices → Audio Output
```

The Music Director watches game state and translates it into synth parameters:

| Game State              | Music Parameter                |
|-------------------------|---------------------------------|
| Turn number             | Chord progression step          |
| Turn phase (build/fight)| Timbre: warm→harsh (filter, FM)|
| Active combats          | Filter cutoff, distortion gain  |
| Territory count         | Bass note root                  |
| Player ranking          | Tempo (BPM)                     |
| Nuke in flight          | Pitch envelope, reverb wash     |
| Victory/defeat imminent | Harmony mode (major→minor)      |
| Zoom level              | Mix: ambient→intimate           |

### 11.2 Stem-based Voice Layout

| Voice Index | Role        | Sound    | Typical Params                  |
|-------------|-------------|----------|---------------------------------|
| 0           | Sub bass    | sine     | note 24–36, fm 0.5, lpf 200    |
| 1           | Bass        | saw      | note 36–48, lpf 400, lpd 0.3   |
| 2           | Pad/Chords  | tri      | note 48–72, verb 0.8, chorus   |
| 3           | Lead        | pulse    | note 60–84, vib 4, delay 0.5  |
| 4           | Perc BD     | sample   | crate_bd, distort 1            |
| 5           | Perc SD     | sample   | crate_sd, hpf 200              |
| 6           | Perc HH     | sample   | crate_hh, gain 0.3             |
| 7           | FX/Noise    | pink     | hpf 2000, verb 0.9             |

### 11.3 Pattern Engine

```rust
struct Pattern {
    bpm: f32,
    step: u32,           // current 16th note
    bars: Vec<Bar>,      // sequence of bars
    tick_interval: Duration,
}
```

Each tick, the pattern engine evaluates which voices to trigger/update.
Not a full language like Strudel — just a programmatic sequencer.

### 11.4 WASM Strategy

rodio doesn't work in WASM. Options:

| Option                              | Pros                           | Cons                          |
|-------------------------------------|--------------------------------|-------------------------------|
| **web-audio-api crate**             | Rust API, WASM, Web Audio      | Limited doc, early crate       |
| **wasm-bindgen → AudioContext**    | Full control, browser-native    | Boilerplate, JS bridge        |
| **Compile dough.c → WASM**         | Already works, single C file   | C interop, AGPL copyleft      |
| **Pure Rust synth + Web Audio**    | Own code, no C deps            | Most work, most control        |
| **cpal + WASM shim**               | Native + WASM abstraction       | cpal WASM support unclear      |

**Recommended:** Pure Rust synth engine (learning from dough.c architecture)
with a thin WASM output layer using `wasm-bindgen` → `AudioContext` + `AudioWorklet`.
This keeps the engine in Rust, no C FFI, and works on both native + WASM.

---

## 12. Implementation Priority

### Phase 1: Core Synthesis (can prototype in 1–2 days)
1. `Oscillator` enum: sine, saw, zaw, tri, pulse, white, pink, brown
2. `Filter` struct: 2-pole SVF with LP/HP/BP outputs, resonance
3. `Envelope` struct: generic ADSR with amount scaling
4. `Voice` struct: osc → gain_env → filter(fil_env) → output
5. `Mixer` struct: mix N voices, global delay + reverb sends

### Phase 2: Modulation (1–2 days)
1. FM: modulator osc → carrier frequency
2. AM/RM: LFO on amplitude
3. Vibrato: LFO on pitch
4. Pitch envelope
5. Per-voice FX: crush, distort, coarse

### Phase 3: Voice Management (1 day)
1. Voice pinning (assign by index)
2. Glide / portamento
3. Parameter smoothing (1-sample lag for filter cutoff, gain)
4. Voice steal on overflow

### Phase 4: FX (1–2 days)
1. Delay line with feedback
2. Freeverb (Schroeder reverb)
3. Chorus (delay + LFO)
4. Phaser / Flanger (if time)

### Phase 5: Pattern Engine (2–3 days)
1. Step sequencer at configurable BPM
2. Euclidean rhythm generator
3. Chord/arpeggio patterns
4. Schedule with time offset

### Phase 6: Game Integration (2–3 days)
1. Music Director: game state → param updates
2. Stem definitions per game phase
3. Crossfade between musical states
4. Hook into `SowApp::update()`

### Phase 7: WASM Output (2–3 days)
1. `web-sys` AudioContext construction
2. ScriptProcessorNode or AudioWorklet for sample output
3. Shared ring buffer between Rust (WASM) and Web Audio
4. COOP/COEP headers (or use AudioWorklet which avoids SharedArrayBuffer requirement)

---

## 13. Key dough.c Code Patterns to Study

From the source (single C file), the key patterns are:

1. **Voice struct as flat state** — all params stored as floats, no graph
2. **Message = struct of optional params** — parse once, apply to voice
3. **Sample-accurate timing** — messages carry `time` field, queued ahead
4. **Stateless rendering** — each sample computed from voice state, no lookahead
5. **Filter = SVF (state variable filter)** — single struct gives LP/HP/BP simultaneously
6. **Reverb = Freeverb** — 4 parallel comb + 4 series allpass
7. **Delay = simple circular buffer** — read head at (write - delay_samples)

---

*This document is the reference spec for building sow-audio's live music engine.
When implementing, consult this before searching the internet.*
*Last updated: 2026-07-01*

## 14. dough.c Source Code Breakdown

This section details the actual implementation patterns found in `dough.c`, highlighting areas where the C code diverges from initial assumptions and providing a blueprint for the Rust port.

### 14.1 Memory Layout
Dough avoids dynamic allocation during playback. In `dough_engine_init`, it allocates a single massive block of memory using a custom `malloc` (or system `malloc` for native):
```c
engine->memory = malloc(vs + os + ds + vbs + es + fs + fb);
```
The memory is strictly partitioned:
1. `vs`: Array of `voice` structs (Voice state)
2. `os`: Array of `Orbit` structs (Global delay/reverb states)
3. `ds`: Delay line buffers
4. `vbs`: Reverb buffers (Dattorro delay lines)
5. `es`: Scheduler events array
6. `fs`: PCM sample data
7. `fb`: Framebuffer for visualization

**Rust Translation:** We can achieve this without unsafe pointers by storing these as `Vec<T>` or pre-allocated `Box<[f32]>` in an `Engine` or `Mixer` struct that lives for the lifetime of the application.

### 14.2 The Voice Struct (Flat State)
A `voice` contains no graph or pointers. It is a completely flat state machine:
```c
typedef struct {
  // Envelopes
  ADSRNode gain_adsr, lp_adsr, hp_adsr, bp_adsr, fm_adsr, p_adsr;
  // Oscillators/LFOs
  Phasor phasor, vib_lfo, fm_modulator, am_lfo, rm_lfo;
  // Filters (Biquad arrays for multi-pole)
  BiquadFilter lp_bq[4], hp_bq[4], bp_bq[4];
  // FX
  Phaser phaser; Flanger flanger; Chorus chorus;
  // Params
  Event p; // The parsed message that triggered this voice
  float ch[CHANNELS]; // Output buffer
  // ...
} voice;
```
**Rust Translation:** A massive `Voice` struct containing all sub-components. We process them sequentially in a fixed order. If an effect isn't active (checked via `v.p.lp_active`), it's simply skipped.

### 14.3 Rendering Loop Flow
The engine renders sample by sample, not block by block.
1. `dsp()` loops over `BLOCK_SIZE` (128).
2. For each sample index `i`, it calls `schedule_update()` to process any events due at the current `doughtime`.
3. It calls `gen_sample(engine, output, i)`.
4. `gen_sample` loops over all active voices.
5. `run_voice()` computes the source, applies LFOs, computes envelopes, runs channel-wise FX (filters, crush, distort, chorus), and accumulates into `voice->ch`.
6. Voice channels are added to the mix and multiplied by send amounts for delay/reverb.
7. Delay and Reverb are processed globally per "Orbit" and added to the mix.

### 14.4 Filter Implementation (Biquad vs SVF)
Contrary to initial assumptions, Dough uses two distinct filter implementations:
1. **Audio Path (BiquadFilter):** The actual lowpass/highpass/bandpass filters applied to the audio use the **Audio EQ Cookbook Biquad** formulas. To achieve steeper slopes (12dB, 24dB, 48dB), it cascades 1, 2, or 4 biquad stages in series.
2. **Modulation Path (Filter):** A simple 2-pole State Variable Filter (SVF) is implemented but appears to be used primarily for smoothing or internal modulation, not the main audio path.

### 14.5 Reverb Algorithm (Jon Dattorro)
Dough does NOT use Freeverb. It implements the **Jon Dattorro Reverb**, a much higher quality algorithm based on a complex feedback delay network.
* **Pre-delay & Pre-filter:** Lowpass filter on the input.
* **Input Diffusion:** 4 cascading all-pass filters.
* **Reverb Tank (Figure-8):** Two symmetrical tanks cross-coupled. Each tank has an allpass, delay, lowpass (damping), and another allpass.
* **Taps:** The stereo output is created by tapping the delay lines at 7 specific prime-number offsets (4 for Left, 3 for Right).

**Rust Translation:** We should port this exact `DattorroVerb` struct and its hardcoded prime-number buffer sizes.

### 14.6 Message Parsing & "Event" State
Messages arrive as key/value pairs (`/freq/440/lpf/2000`).
* They are parsed into an `Event` struct where every field defaults to `NAN`.
* `nan_fallback()` replaces un-set fields with sensible defaults (e.g., if `/sound` is set but not `/pw`, `pw` becomes 0.5).
* Missing sustain values are intelligently inferred: if only `attack` is provided, `sustain` defaults to 1.0; if `decay` is provided, `sustain` defaults to 0.0.

### 14.7 Math Primitives without libm
To compile for WASM without a standard library (`#ifdef CLANGWASM`), Dough implements its own math:
* `our_sinf`: Rational Padé approximant (highly accurate without Taylor series branching).
* `our_exp2f` & `our_log2f`: Brilliant float-bit-hacks manipulating the IEEE-754 exponent bits directly, combined with polynomials for the mantissa.
* `our_powf`: Implemented as `exp2(y * log2(x))`.

**Rust Translation:** While Rust's `core` doesn't have math, `libm` does. However, if we want extreme WASM binary size optimization, porting these bit-hacks would eliminate the `libm` dependency entirely.


## 15. Audit: sow-audio vs. dough architecture

Before starting the WASM migration and the new music engine, here is where `sow-audio` currently stands compared to the `dough` blueprint:

### 1. Audio Output Threading
*   **dough:** Native uses PortAudio with a single callback. WASM uses a shared memory block; JS calls `js_init` and reads the float buffer directly via `requestAnimationFrame` or `AudioWorklet`.
*   **sow-audio:** Uses `rodio` heavily gated behind `#[cfg(not(target_arch = "wasm32"))]`. `engine.rs` spawns a dedicated background thread that receives commands via an `mpsc` channel. For WASM, everything is stubbed out (no audio). 

**Migration Path:** We need to rip out `rodio`. We will keep the `mpsc` channel and background worker thread concept for Native (using `cpal` to drive the audio stream, which `rodio` uses underneath anyway). For WASM, we'll expose a function like `fill_buffer(&mut [f32])` to the JS/WebAudio side, which will pull from our Rust mixer.

### 2. Audio Processing (Sources vs Mixer)
*   **dough:** One monolithic `Mixer` that loops over `active_voices`, rendering sample-by-sample (`gen_sample`).
*   **sow-audio:** Defines sources as structs implementing `Iterator<Item = f32>` and `rodio::source::Source` (e.g., `DoublePulseSource`, `ArpeggioSource`). These are pushed into a `rodio::MixerDeviceSink`.

**Migration Path:** We need to write our own `Mixer` in Rust. Our SFX sources (`combat.rs`, `death.rs`) need to be rewritten slightly to simply provide a `.next_sample() -> f32` method instead of implementing the complex `rodio::Source` trait. 

### 3. State Management & Commands
*   **dough:** `Event` struct holds 50+ parameters. Messages mutate the parameters of a `voice` struct.
*   **sow-audio:** Commands like `AudioCommand::PlayCombatSound` carry simple enums, which the engine turns into new `Source` objects and pushes to `rodio`. Priority system tracks the number of active voices.

**Migration Path:** The `mpsc` command system in `sow-audio` is good and works well for game integration. We will extend `AudioCommand` to include `PlaySynthEvent(Box<Event>)` for the music engine, allowing the game's `MusicSession` to send parameter updates to the mixer thread.

### 4. Math Dependencies
*   **dough:** Implements `our_sinf`, `our_exp2f` to avoid C `math.h` and shrink WASM size.
*   **sow-audio:** Uses `std::f32::consts` and standard `.sin()` methods.

**Migration Path:** Since Rust's `core` doesn't include trig functions, compiling `sow-audio` for `wasm32-unknown-unknown` without `std` (or without `libm`) would fail. We can either just use `std` (since WASM size isn't a critical bottleneck for us yet, and web-sys brings in a lot anyway) or port Dough's bit-hack math functions if we want an ultra-minimal core. For now, we will stick to `std::f32::sin()` for simplicity.

