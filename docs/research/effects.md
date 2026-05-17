# Rhythm-Enhancing Effects — Research & Recommendation

Persisted from research originally orphaned in conversation context.

## Audit of what ships today

drummr's FX chain is two stages. Per-voice: `PostFx` in `src/dsp/postfx.rs` (zero-order-hold sample-rate decimator + N-bit quantizer; defaults `bits=16, rate=1` pass through). Master: one line in `src/audio.rs::soft_clip` — `x.tanh()`. The summed signal is `kit.tick() * 0.7`, then tanh, then duplicated across stereo channels (mono in a stereo container). No reverb, no delay, no EQ, no compression, no pan, no width, no modulation FX. As `TODO.md` puts it: "Master path is mono, dry, identical." Adjacent infrastructure ready to leverage: `BpmEngine` (`src/dsp/bpm_engine.rs`) broadcasts tempo at 10Hz, and a per-voice mod matrix with two LFOs ships in `src/dsp/modulation_engine.rs` — both addressable from an audio-thread effect via an `AtomicU32` BPM snapshot (~10 LOC).

## Section A — Top 5 effects to add next

### 1. Stereo bus + per-voice pan + master plate reverb with per-voice send (per-kit master + per-voice send)
DSP. Convert mono `tick() -> f32` to `(f32, f32)` with constant-power pan per voice (`cos θ, sin θ`). Master plate via Dattorro's 1997 JAES topology [1]: four serial input diffuser allpasses (5–70ms), then a cross-coupled tank of two allpass loops with damping LPFs and modulated taps. ~30 fixed delay lines totalling ~22k samples at 48kHz. Knobs: pre-delay, bandwidth, decay, damping. Each voice routes a `send` amount into it.

drummr impact. Same 22 presets become 22 different rooms before changing a voice param — the single biggest "kits feel different" unlock. The Dattorro plate is *the* snare reverb (Lexicon 224, EMT 140). Wacky kits (Glass Forest, Foundry) finally inhabit space.

Reference. Phil Collins "In the Air Tonight" (Townhouse, 1981) [2]; every modern snare on a Steve Albini, Greg Wells, or Jack Antonoff record.

Implementation. ~500 LOC (300 reverb, 100 stereo plumbing, 100 commands+UI). CPU: ~80 µs/128-frame buffer at 48kHz for the reverb (one master instance); ~0.5 µs/voice for the pan.

Why higher priority. Stereo is a chokepoint: every other effect either needs it (ping-pong), benefits dramatically from it (chorus, stereo tremolo), or feels half-rendered without it (linked compressor). The TODO names this explicitly under `Track B step 1`.

### 2. Per-voice transient shaper (per-voice insert)
DSP. SPL Differential-Envelope-style [3]: two parallel envelope followers, one fast (~1ms attack, 5ms release) tracking the transient, one slow (~30ms attack, 200ms release) tracking the body. `attack_gain = pow(fast/slow, attack_amount)` shapes the spike; `sustain_gain` similarly shapes the tail. Level-independent — no threshold. Two knobs: `attack ∈ [-1,+1]`, `sustain ∈ [-1,+1]`.

drummr impact. The TODO notes "72% of voices share `attack = 1.0` ms — identical transient = identical ear-lock across every kit." A transient shaper sharpens or softens existing synthesis without retuning a preset. Negative attack softens FM kick clicks; positive sharpens claps and rims. Negative sustain chops decays (gated feel without an actual gate). Pairs perfectly with Modal voices, where exciter vs. resonator is already explicit.

Reference. SPL Transient Designer (1999) defines the category. Aphex Twin IDM snares = positive attack, negative sustain on top of bitcrushed samples [4].

Implementation. ~120 LOC. CPU: ~1.5 µs/voice (two one-pole followers, two multiplies). Zero infrastructure dependencies.

Why higher priority. Solves a named TODO problem (kit-differentiation attack monoculture) at near-zero CPU and LOC without re-tuning any preset.

### 3. Per-voice 2-band tilt EQ via SVF (per-voice insert)
DSP. One TPT state-variable filter per voice [5], wired as a tilt: low shelf at 250Hz with gain `+g`, high shelf at 4kHz with gain `-g`. One knob `tilt ∈ [-1,+1]` brightens or darkens; or expose `low_gain` and `high_gain` separately. SVF (TPT/Chamberlin) is the right block — simultaneous LP/BP/HP outputs from two integrators, stable at low Q, well-behaved at low frequencies where biquads stagger.

drummr impact. The TODO calls out "non-overlapping spectral identities" as a kit-differentiation task; per-voice EQ is the granular tool. Office After Hours hats tilted up to 3–4kHz; Glass Forest brightened wholesale; Kitchen Sink Symphony rolled off above 6kHz.

Reference. Neve 1073 high-shelf; SSL E-channel 12kHz drum snap.

Implementation. ~80 LOC for the TPT SVF + ~20 LOC dual-shelf wrapper. CPU: ~3 µs/voice.

Why higher priority. Lowest LOC of any meaningfully audible effect, and the SVF is reused infrastructure for item 5 candidates (filter envelope) and Section B's stutter pitch logic.

### 4. Per-voice drive / saturation (per-voice insert)
DSP. Asymmetric soft clipper. Minimum: `tanh(x * (1 + drive*6))`. Better: add pre-tanh bias `x + b`, remove DC after, one-pole LP at ~12kHz post-clip to tame aliasing. With 2× oversampling for the nonlinearity you get clean saturation; without it you get the lo-fi character SP-1200 records made famous.

drummr impact. Most percussion timbre difference comes from harmonic distortion at the transient, not from the synthesis. Drive on an FM kick adds over-driven-monitor cone-flap. Drive on a Phys snare turns ringing into buzzing. Already named in `Track B` (`Per-voice drive/saturation on FM and Phys`) — this just formalises priority.

Reference. SP-1200 lo-fi character — 12-bit/26.04kHz aliasing into the converter, Curtis filter on output [6]. Boards of Canada multi-pass tape bouncing [7].

Implementation. ~60 LOC with oversampling, ~15 without. CPU: ~2 µs/voice without, ~8 µs/voice with 2×.

Why higher priority. Tiny, audibly transformative, already wanted. Pairs synergistically with #3 — drive into tilt EQ is the entire 70s drum mix.

### 5. Master tape-style bus compressor with optional sidechain (per-kit master)
DSP. Feedforward, smoothed-gain-reduction one-pole envelope (~10ms attack, 100ms release), 2:1 to 4:1, soft knee. Optional external trigger from any designated voice slot. Self-keyed = glue; kick-keyed = the EDM pump [8].

drummr impact. Two distinct sounds from one box. Glue makes 22 kits feel like one studio. Pump makes every kit work in a 4/4 house context. The TODO's `Track B step 1` lists stereo + pan + reverb but conspicuously omits master dynamics; that's a gap.

Reference. SSL G-bus on every drum mix from Stardust to *Random Access Memories*. Daft Punk's Alesis 3630 sidechain pump [8].

Implementation. ~180 LOC. CPU: ~5 µs/buffer (master, not per voice).

Why higher priority than runners-up. The *only* effect that changes how the **kit-as-a-whole** behaves rhythmically. Tape delay (lovely but single-trick), spring reverb (Foundry-only character), and slapback (era-specific) all colour one voice; bus compression makes every voice breathe in time.

## Section B — Top 3 rhythm-specific effects

### B1. Tempo-synced ping-pong delay
Two delay lines bouncing L↔R, each at a BPM-locked subdivision (1/4, 1/8, 1/8T, 1/16) read via the `BpmEngine` AtomicU32 snapshot. Feedback 30–60%, one-pole LP in the loop for tape-like decay. Per-voice send so snare and hat can use different divisions. The "rhythm-enhancing" effect par excellence — turns a single hit into a polyrhythmic event. Autechre's "Bike" (LP5, 1998) — every hit gets a pluck-like short delay [9]. ~150 LOC, ~10 µs/buffer.

### B2. Probabilistic stutter / beat-repeat (per-voice insert)
On voice trigger, with probability `p`, capture the first ~80ms into a ring buffer and re-trigger 1–4 times at a BPM-locked 16th or 32nd, optionally pitched up an octave or decreasing in amplitude. With `p = 0.05–0.15` you get a tasteful glitch — every ~10th snare suddenly stutters. RP Boo's triplet stutter [10] is the reference. ~100 LOC; ~3 µs/voice idle, ~15 µs during an event.

### B3. Gated reverb (master send + master gate, event-keyed)
Combines #1 with a fast-release gate, but cleaner is *event-driven*: open the gate for ~120ms after any kit trigger, snap closed in 5ms. Shorter gate = more 80s. With high reverb send → Townhouse / Padgham / Collins sound [2]. ~60 LOC of gate plus a sidechain trigger wire once #1 is in. Worth its own slot because it has the strongest *recognised genre identity* of any item on this list.

## Section C — Producer cross-reference

**Phil Collins / Hugh Padgham gated snare** — Originally an SSL 4000 talkback-into-Stone-Room accident. Chain: drums → reverberant Stone Room → SSL channel with talkback "listen mic" compressor → noise gate to chop tail [2]. drummr: master plate (#1) → master gate keyed to snare slot (B3) → drive (#4) into plate input.

**Boards of Canada warm haze** — Multi-pass bouncing across Grundig, Revox, Tascam — each pass adds wow/flutter, hiss, HF rolloff [7]. drummr: low-amount drive (#4) + master tilt EQ (#3 applied at master) dark + slow ~3Hz LFO routed to filter cutoff (existing mod matrix once cutoff is a target).

**Burial ambience bed** — Sound Forge built-in reverb everywhere + layered vinyl crackle filling every dead space [11]. drummr: long-decay high-diffusion plate (#1) + a low-level Noise-engine voice ducked into the reverb as crackle bed.

**Aphex Twin IDM glitchsnares** — Single-cycle chops, bitcrushed, transient-sharpened beyond natural [4]. drummr: existing PostFx bitcrush + transient shaper (#2) with attack=+1, sustain=−0.5 + stutter (B2).

**LinnDrum / SP-1200 grit** — 12-bit @ 26.04kHz aliasing on input + Curtis analog filter on output [6]. drummr partial today: PostFx `bits=12, rate≈2` emulates the rate; drive (#4) + tilt-down EQ (#3) replaces the Curtis output filter.

## Section D — drummr-specific synergies

- **Karplus Forge (all-Phys) + spring reverb** (a 3-spring FDN) — boings reinforce the plucked excitation; nylon-string / cigar-box-guitar / dulcimer territory.
- **909 Warehouse + master plate (#1) + master drive (#4)** — granular cymbals through a long plate = the warehouse-techno cymbal. Master drive replaces drummr's sterile tanh with saturated-monitor character.
- **Industrial Glitch / Foundry + transient shaper (#2, +attack) + sidechain bus compression (#5)** — sharpens anvil/metal past natural, then pumps the kit around an implied kick. Eight bars of this is industrial techno; rhythm comes from compression as much as from the notes.
- **Drift / Grain Dust + ping-pong (B1) tempo-locked at 1/8 dotted** — ambient kits *need* tempo-locked time motion; long sustains become Eno-style frippertronics pads from a single trigger.
- **Glass Forest + sub-audio AM via existing LFO matrix** — no new effect needed; 5–7 Hz tremolo on wine-glass sustains is shimmer. Route already exists in `modulation_engine.rs` and is unused.
- **Modal_Demo + per-voice tilt EQ (#3)** — Modal is mathematically purest, hence most "synthetic." A 6kHz high-shelf cut immediately makes it sound recorded, not generated.

## Section E — One radical idea

**Per-voice probabilistic FX layering — "Mulligan FX."** On every voice trigger, draw a random number; with probability `p` (1–10%), route that single hit through an *entirely different* FX chain. Snare normally goes through plate + drive; once every 30 hits, it goes through a tape-stop pitch-down + ring-modulator at 90Hz. Listeners can't predict it but their brain *knows* it just heard something. Implementation: per-voice `mulligan_chain` (parallel FX params), per-voice `mulligan_prob`, a coin flip on trigger that atomically swaps a chain pointer before the next `tick()`. ~150 LOC if a chain framework exists. This converts drummr from "drum machine" to "drummer having a bad day occasionally" — the *human imperfection* axis no commercial drum machine offers built-in. The TODO's "layer unexpected with expected" principle, executed at sample time.

## Section F — Implementation dependency tree

```
Stereo signal path (audio.rs: f32 -> (f32, f32))
├── Per-voice pan
├── Master plate reverb (#1)
│   ├── Per-voice reverb send
│   └── Gated reverb (B3)        -- depends on master gate trigger wire
├── Ping-pong delay (B1)
│   └── BPM AtomicU32 snapshot   -- ~10 LOC on top of existing BpmEngine
└── Stereo width / Haas / chorus (future)

Per-voice insert chain  (mono-safe; can ship before stereo)
├── Transient shaper (#2)        -- zero deps
├── 2-band SVF tilt EQ (#3)      -- zero deps; provides SVF for filter env
├── Drive / saturation (#4)      -- zero deps
└── Stutter (B2)                 -- depends on BPM snapshot

Master dynamics (master bus, post-stereo, pre-master-limiter)
└── Bus compressor (#5)
    └── Sidechain key wire from any voice slot  -- ~30 LOC

Existing, ready: PostFx (bitcrush + SRR), tanh limiter.
```

**The chokepoint is stereo.** Items #1, B1, and any future modulation FX (chorus, stereo tremolo) sit behind it. Items #2, #3, #4, #5 do not — all useful as mono inserts first, auto-expanding when the bus widens.

## Sources

[1] Dattorro, J. (1997). *Effect Design, Part 1: Reverberator and Other Filters*. JAES. https://ccrma.stanford.edu/~dattorro/EffectDesignPart1.pdf
[2] Sweetwater InSync, *Dissecting the Phil Collins Drum Sound*. https://www.sweetwater.com/insync/dissecting-the-phil-collins-drum-sound/ ; Wikipedia, *Gated reverb*. https://en.wikipedia.org/wiki/Gated_reverb
[3] Plugin Alliance, *SPL Transient Designer Plus — Differential Envelope Technology*. https://www.plugin-alliance.com/products/transient-designer-plus
[4] Splice Blog, *3 Approaches for Aphex Twin Style Drum Programming*. https://splice.com/blog/programming-drums-aphex-twin-style/
[5] Smith, J.O. III, *Digital State-Variable Filters*. https://ccrma.stanford.edu/~jos/svf/svf.pdf ; EarLevel, *The digital state variable filter*. https://www.earlevel.com/main/2003/03/02/the-digital-state-variable-filter/
[6] LANDR, *SP-1200: The Sampler That Changed Hip-Hop Forever*. https://blog.landr.com/sp-1200/ ; Wikipedia, *E-mu SP-1200*. https://en.wikipedia.org/wiki/E-mu_SP-1200
[7] Gearspace, *Boards of Canada sounds (multi-tape-deck bouncing)*. https://gearspace.com/board/electronic-music-instruments-and-electronic-music-production/457922-boards-canada-sounds.html
[8] Point Blank / DJ Mag, *Daft Punk Style Vintage Sidechain Compression in Logic Pro*. https://djmag.com/watch/point-blank-tutorial-daft-punk-style-vintage-sidechain-compression-logic-pro ; gearnews, *Alesis 3630: The Daft Punk Compressor*. https://www.gearnews.com/alesis-3630-studio/
[9] Studio Brootle, *IDM Techniques*. https://www.studiobrootle.com/idm-techniques/
[10] Beat Lab Academy, *The Secret Sauce Behind Aphex Twin, IDM and Glitch Music*. https://beatlabacademy.com/soundtrackers-secret-sauce-behind-aphex-twin-idm-glitch-music/
[11] MusicRadar, *How Burial produced the 21st century's most influential electronic album*. https://www.musicradar.com/artists/im-not-a-musician-i-was-always-scared-of-people-who-had-studios-how-burial-produced-the-21st-centurys-most-influential-electronic-album-on-a-rubbish-dying-computer-with-outdated-software ; Attack Magazine, *Burying the Mix in Burial Style Reverb*. https://www.attackmagazine.com/technique/tutorials/burying-the-mix-in-burial-style-reverb/

Rust crate references (pure-Rust, MIT/Apache, suitable for vendoring or inspiration):
FunDSP — https://crates.io/crates/fundsp (MIT OR Apache-2.0, no_std, ships reverbs/filters/delay); dasp — https://github.com/RustAudio/dasp (Apache-2.0, allocation-free primitives); surgefx-reverb — https://lib.rs/crates/surgefx-reverb; synfx-dsp — https://docs.rs/synfx-dsp; fdn-reverb — https://github.com/padenot/fdn-reverb.
