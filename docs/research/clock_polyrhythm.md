# Clock-Aware & Polyrhythmic Drum Design

drummr already detects tempo from MIDI onsets (`src/dsp/bpm_engine.rs`) and emits MIDI
Clock at 24 PPQN through a master clock thread (`src/sync.rs`), but the synthesis
engines themselves are tempo-agnostic. LFOs run at fixed Hz
(`src/dsp/modulation_engine.rs:78-83`), envelopes decay in fixed milliseconds
(`src/dsp/envelope.rs:31-47`), and triggers fire one-shot voices that never consult
the clock. This document scopes how to make the synthesis itself tempo-aware: drums
that decay over bars, polyrhythmic sub-patterns, beat-divided LFOs, probabilistic
ghost notes, and the kits that become possible.

The whole document hinges on one ~10 LOC change: an atomic BPM snapshot on
`SharedState` so the audio thread can read tempo every block without locking the BPM
estimator. Half the features below are gated on it.

## Section A — What's possible today

Effectively nothing clock-aware. The mod matrix can route `Lfo1`/`Lfo2` to any
modulatable param, but both LFOs are configured in Hz only — no path from "BPM 128,
1/16" to a frequency. The envelope is two floats (`attack_sec`, `decay_sec`) locked
at trigger time. Voices have no notion of elapsed bars. The audio thread shares only
the `MidiEvent` ring buffer with the clock — note-on triggers, no tempo or
beat-phase metadata. Every preset in `presets/kits/*.toml` is rhythm-blind: two
kicks 500 ms apart sound identical at 60 BPM and 200 BPM.

## Section B — Tempo-locked LFOs and envelopes (Path 1)

The chokepoint. Implementation:

1. Add `pub current_bpm_q24: AtomicU32` to `SharedState` (`src/state.rs`). The
   existing 10 Hz BPM broadcast in `main.rs` already holds the `BpmEngine` lock; add
   one `store(..., Relaxed)` call. Audio thread reads with `Relaxed`. ~10 LOC total.
2. Define `BeatDivision` enum: `T32, T16, D16, T8, S8, D8, T4, S4, D4, S2, Bar,
   Bar2, Bar4, Bar8` (triplet, dotted, straight × each level — 14 variants).
   `fn beats(&self) -> f32` (e.g. `S4 = 1.0`, `D8 = 0.75`, `T8 = 1.0/3.0`,
   `Bar4 = 16.0`).
3. Extend `DrumSound` (`src/kit.rs`) with `lfo1_division`, `lfo2_division`,
   `decay_division: Option<BeatDivision>`.
4. At trigger time, compute `decay_sec = (60.0/bpm) * div.beats()` and pass to
   `AdEnvelope::set_params`. Same pattern for LFO Hz.

Trigger-time computation is the right tradeoff: it adapts to slow tempo drift across
takes but doesn't alias if BPM stutters mid-decay. No lock contention on the audio
thread.

**Phase-reset semantics.** Today `Lfo::phase` is never reset. Add
`lfo_reset_on_trigger: bool` per voice — free-running for shared modulation,
trigger-reset for one-shot per-hit motion. A stronger variant resets phase on bar
boundaries, derived from a sync-engine tick counter exposed as an atomic.

## Section C — Polyrhythmic sub-hits (Path 2)

A trigger spawns N additional triggers with delays:

```toml
sub_hits = [
    { offset_ms = 30.0, velocity_factor = 0.5 },   # flam
    { offset_ms = 60.0, velocity_factor = 0.3 },   # drag
]
```

Lives entirely in the audio thread: a small `Vec<(samples_remaining, slot,
velocity)>` queue per `KitEngine`. Each block tick decrements counters; on zero,
re-enter `trigger_slot`. ~50 LOC, no atomic-BPM dependency for the ms variant.

A second variant uses `BeatDivision` offsets — that one depends on Path 1. Ship
both: ms for flam/drag, division for triplets and swing. Sub-hits chain (cap
recursion at 2). Fixes the standing `TODO.md:247` multi-tap clap gap (LinnDrum/909
clap = 4 noise bursts ~12 ms apart).

## Section D — Per-slot rhythm patterns (Path 3)

The slot's "single hit" becomes a sequencer:

```toml
[[slots]]
rhythm = { pattern = [
    { div = "S16", velocity = 1.0 },
    { div = "S16", velocity = 0.0 },   # rest
    { div = "S16", velocity = 0.4 },
    { div = "S16", velocity = 0.7 },
] }
```

On trigger, replay the pattern at current BPM. This is where drummr becomes a
generative drum machine: an external sequencer fires once per bar, but each voice
plays its own 16-step sub-pattern, possibly at different divisions. Depends on
Path 1. ~100 LOC plus a per-slot state machine.

## Section E — Probabilistic / generative (Path 4)

Three features sharing one RNG:

- `trigger_probability: Option<f32>` — fraction of hits that fire (0.7 = 30%
  dropped). Accidental groove holes.
- `ghost_probability` + `ghost_offset_ms` + `ghost_velocity_factor` — stochastic
  soft echo per hit.
- `velocity_jitter: Option<f32>` — ±N% random velocity per hit. Cures the
  machine-gun roll.

Audio thread keeps a `SmallRng` seeded at construction. ~30 LOC. No atomic-BPM
dependency. Round-robin variants (cycle through N parameter sets per hit) are
conceptually adjacent and trivial. Markov chains are rejected — state-table
explosion in TOML outweighs perceptual return over a biased coin flip.

## Section F — Bar-aware decay (Path 5)

**Multi-bar envelope.** `bar_decay: Option<f32>` — voice decays over N bars at
current BPM. At 120 BPM, `bar_decay = 4.0` = 8 s. Depends on Path 1. ~50 LOC.

**Step decay.** Voice drops `step_decay_db` on every quarter-note boundary instead
of decaying continuously. Needs the sync engine's tick counter exposed.

**Bar-locked pitch sweep.** `bar_pitch_semitones` shifts pitch every bar — combined
with long modal voices yields a 4-bar bell descending one semitone per bar.

**Self-gating.** Voice plays its envelope, then snaps off at the next bar/beat
boundary. Stuttery cymbal washes for free.

## Section G — Polymeter and poly-tempo

**Polymeter (different cycle lengths, same tempo).** Mostly a Path 3 concern: slot
A's pattern is 16 steps, slot B's is 12, drifting in/out of alignment over a
48-step super-cycle.

**Poly-tempo (different tempos in one kit).** Slot A's LFO runs at BPM, slot B's at
BPM × 5/4. `lfo_tempo_ratio: f32` multiplier on top of Path 1. ~10 extra LOC.

**Phase drift.** Steve Reich's *Piano Phase* as a drum kit: two identical voices at
BPM and BPM × 1.005 drift one beat over ~3 minutes. `tempo_drift_percent: f32` per
voice.

Poly-tempo is where drummr becomes architecturally unlike any commercial drum
machine — Ableton, Maschine, Rytm all treat tempo as global. drummr can treat it
per-voice. The differentiator.

## Section H — Producer references

- **Aphex Twin — *Vordhosbn*** (drukqs, 2001) — each hit has its own tempo-locked
  sub-pattern. https://en.wikipedia.org/wiki/Drukqs
- **Squarepusher — *The Swifty*** (Hard Normal Daddy, 1997) — time-stretched,
  polyrhythmically re-sequenced breakbeat.
- **Autechre — *Bike*** (LP5, 1998) — every hit gets a pluck-like short delay;
  later records embed generative patterns per voice.
- **RP Boo — *Baby Come On*** (Legacy, 2015) — Chicago footwork: triplet hits at
  160 BPM, asymmetric polymeter as groove.
  https://en.wikipedia.org/wiki/Footwork_(genre)
- **Steve Reich — *Piano Phase*** (1967) — phase-process precedent.
  https://en.wikipedia.org/wiki/Phasing_(music)
- **Conlon Nancarrow — *Studies for Player Piano*** (1948-92) — canonical
  poly-tempo (irrational ratios like √2:1).
  https://en.wikipedia.org/wiki/Conlon_Nancarrow
- **Euclidean rhythms** — Toussaint (2005), `bjorklund(k, n)`.
  http://cgm.cs.mcgill.ca/~godfried/publications/banff.pdf

## Section I — Top 5 features by value-per-LOC

1. **Atomic BPM snapshot** (~10 LOC). Chokepoint. Delivers nothing alone — but
   Paths 1, 3, 5, and all of Sections F/G are gated on it. Ship first.
2. **`BeatDivision` enum + tempo-locked decay/LFO** (~110 LOC on top of #1).
   Multi-bar decay, dotted-eighth hat sweeps, triplet snares. Biggest "this is a
   tempo-aware drum machine" payoff.
3. **Sub-hits (ms)** (~50 LOC, no #1 dependency). Flams, drags, multi-tap clap.
   Could ship before #1.
4. **Probability + velocity jitter** (~30 LOC, no #1 dependency). Cures
   machine-gun rolls; parallel with #1/#2.
5. **Per-slot rhythm patterns** (~100 LOC, after #2). Where drummr stops being a
   sample-trigger and starts being a generative instrument.

Features 2 and 4 share an enum and atomic — ship as one PR. Feature 5 is
independent and can ship in parallel with #2.

## Section J — Kit concepts unlocked

1. **Phase Mirror** — two kicks, identical FM patches, slot 0 at BPM, slot 1 at
   BPM × 1.005. Map both to the same MIDI note. Drift one full beat over ~2 min.
   Steve Reich as a drum kit.
2. **Triplet Trap** — every snare fires a `rhythm = [T8, T8, T8]` ghost sub-pattern
   at 0.3 velocity. Snare-fill character without programming fills.
3. **Cathedral Pulse** — every voice has `bar_decay = 4.0` and an LFO at `Bar`
   division routed to brightness. One hit per bar, evolves over four bars. Drum
   machine as ambient drone.
4. **Polymeter Madness** — kick on a 4-step pattern, snare on 3, hat on 5, ride on
   7 — same step rate, 420-step super-cycle.
5. **Ghost Maker** — every voice has `ghost_probability = 0.4`. Same MIDI input
   produces a different groove every pass. Live-improvisation kit.
6. **Drift** — half the kit at `tempo_drift_percent = +1.0`, half at −1.0. Kick on
   grid, hat ahead, snare behind. Permanent "humanise" without LFO wobble.
7. **Footwork Engine** — tempo-locked sub-hits at `T16` everywhere, 160 BPM target,
   `velocity_jitter = 0.3`. Triplet shuffles emerge from straight 16th input.

## Section K — One bold move

**Phase Mirror.** It's a 4-line kit definition that demonstrates the entire
poly-tempo architecture in 30 seconds of listening. Two kicks. Drift them. Done.

Right flagship because (a) instantly audible — the kit *is* the demo; (b) forces
every dependency to land (atomic BPM, `BeatDivision`, `tempo_drift_percent`);
(c) no commercial drum machine offers it, so it stakes a genuine "what is drummr
for?" claim; (d) it scales — the obvious next step is the Nancarrow "four kicks
at irrational ratios" kit, then live tempo-drift modulation in the mod matrix.
Ship `Phase_Mirror.toml` alongside a 2-minute MIDI loop in `examples/` that hits
both kicks on every quarter note. The drift sells itself.

## Implementation roadmap

```
Phase Mirror ──┐
Cathedral ─────┤
Triplet Trap ──┤
Polymeter ─────┤
               ├── BeatDivision + tempo-locked LFO/decay ──┐
Footwork ──────┤                                            ├── atomic BPM
               ├── per-slot rhythm patterns ────────────────┤   snapshot
               ├── multi-bar decay ─────────────────────────┘   (~10 LOC)
Ghost Maker ───┼── probability + ghost sub-hits ── (no dep, ~30 LOC)
Multi-tap clap ┴── sub-hits (ms) ────────────────── (no dep, ~50 LOC)
Drift ────────── tempo_drift_percent ───────── needs atomic BPM (~20 LOC)
```

LOC budget:

- Phase 1 — atomic BPM snapshot: **~10 LOC**.
- Phase 2 — `BeatDivision` + tempo-locked decay + LFO: **~110 LOC** (schema, serde,
  trigger-time conversion).
- Phase 3 — sub-hits (ms): **~50 LOC**, parallel-friendly.
- Phase 4 — probability + jitter: **~30 LOC**, parallel-friendly.
- Phase 5 — per-slot rhythm patterns: **~100 LOC**, after Phase 2.
- Phase 6 — multi-bar decay + bar-locked features: **~50 LOC**, after Phase 2.
- Phase 7 — poly-tempo (`tempo_drift_percent`, `lfo_tempo_ratio`): **~30 LOC**.

Total: **~380 LOC** for the entire roadmap. Phase 1 (10 LOC) unblocks ~250 LOC of
downstream work.

## Sources

- `src/dsp/bpm_engine.rs` — autocorrelation BPM detector (6-second window).
- `src/sync.rs` — MIDI Clock master (24 PPQN).
- `src/dsp/modulation_engine.rs` — Hz-only LFOs.
- `src/dsp/envelope.rs` — ms-only AD envelope.
- `src/state.rs` — `SharedState`, gateway for atomic BPM.
- Toussaint, "The Euclidean Algorithm Generates Traditional Musical Rhythms" (2005):
  http://cgm.cs.mcgill.ca/~godfried/publications/banff.pdf
- Bjorklund (2003): https://www.osti.gov/biblio/1003302
- Nancarrow poly-tempo: https://en.wikipedia.org/wiki/Conlon_Nancarrow
- Reich phasing: https://en.wikipedia.org/wiki/Phasing_(music)
- MIDI Clock (24 PPQN): https://en.wikipedia.org/wiki/MIDI_beat_clock
- Footwork conventions: https://en.wikipedia.org/wiki/Footwork_(genre)
