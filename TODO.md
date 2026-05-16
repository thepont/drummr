# drummr TODO

Last updated: 2026-05-16

Tracked list of known issues in the kit system surfaced by the recent bug-hunt
pass. Ranked P0 → P3. Items marked `(in progress)` are being addressed in a
parallel implementation pass.

---

## P0 — Blockers

- [ ] **Kit paths resolve relative to cwd** — all kit/preset file I/O uses relative paths; launching from outside the repo root makes every kit silently disappear from the UI. `src/commands.rs:216,223,228,230,239,278`. `(in progress)`
  - Why it matters: end-user-visible "No kits found" with no error in logs; reproduces any time the binary is launched from a different working dir (IDE, packaged build, systemd unit, etc.).
  - Suggested fix: anchor all kit/preset/sound paths via `env!("CARGO_MANIFEST_DIR")` or a `DRUMMR_HOME` env var; log every fs error instead of `let _ =` no-op.

---

## P1 — Top architectural priorities (in flight)

1. [ ] **Eliminate disk-as-truth for kit mutations** — 8+ command handlers in `src/commands.rs` re-read `kit.toml` from disk just to mutate it. `(in progress)`
   - Move mutation onto in-memory `SharedState::kit`; snapshot to the persistence worker.
   - Removes the disk round-trip from the audio/control hot path and the read-modify-write race below.

2. [ ] **Stop rebuilding `KitEngine` for mapping changes** — `src/commands.rs:122,139` rebuild the full engine, which drops voice state mid-playback. `(in progress)`
   - Add `KitEngine::set_mapping()` that mutates the existing engine in place.
   - Preserves envelopes, LFO phase, and any pending voice state across mapping edits.

3. [ ] **Surface filesystem errors in persistence worker** — `src/persistence.rs:21-30` swallows every save failure with `let _ =`. `(in progress)`
   - Log errors with file path + errno; consider a UI-visible "save failed" toast.
   - Today, a full disk or permission error is completely invisible.

---

## P1 — Confirmed bugs

- [ ] **`kit.toml` read-modify-write race on every `SET_PARAM`** — `src/commands.rs:271-311`.
  - Why it matters: concurrent slider drags can wipe each other's edits.
  - Suggested fix: subsumed by P1#1 (in-memory truth + snapshot writer).

- [ ] **Atomic rename uses a hardcoded tmp filename + swallows errors** — `src/persistence.rs:20,28`.
  - Why it matters: single-writer-only assumption is fragile; failed saves are invisible.
  - Suggested fix: per-target `*.tmp.<pid>.<nonce>` filenames; bubble up `io::Error` and log.

- [ ] **`Voice::Noise` silently no-ops `set_mod` / `set_lfo`** — `src/kit.rs:62,72`.
  - Why it matters: TOML still accumulates mod/LFO entries that do nothing; users think routing is broken.
  - Suggested fix: return a result from `set_mod`/`set_lfo`; reject (or skip persisting) entries the voice can't honour.

- [ ] **`SET_PARAM` doesn't refresh `shared_state.kit` for non-engine-type params** — `src/commands.rs:277`.
  - Why it matters: UI shows the new value, audio engine keeps the old one until next kit reload.
  - Suggested fix: write through to `SharedState::kit` for every param path, not just engine swaps.

- [ ] **`cmd_rx.pop()` drops commands when audio thread can't lock** — `src/audio.rs:21`.
  - Why it matters: lost UI edits with no retry, no log, no user feedback.
  - Suggested fix: peek + retry, or move param updates off the audio thread; at minimum log the drop.

- [ ] **`set_param` is a silent no-op on unknown names across every engine** — `src/fm.rs:133`, `src/phys.rs:190`, `src/granular.rs:177`, `src/hybrid.rs:52,134`.
  - Why it matters: typo'd param names from the UI vanish silently; refactors break controls without anyone noticing.
  - Suggested fix: return `Result<(), UnknownParam>`; log+surface from the command handler.

- [ ] **`NoiseVoice` schema is empty but `set_param` matches `"attack"`/`"decay"`** — `src/noise.rs:44-54`.
  - Why it matters: UI has no way to discover/edit these params even though the engine accepts them.
  - Suggested fix: declare the schema fields, or remove the match arms.

- [ ] **`TEST_TRIGGER` bypasses BPM onset registration** — `src/commands.rs:408` vs `src/app_utils.rs:31`.
  - Why it matters: test triggers aren't counted by the tempo estimator, so manual auditioning skews/clears BPM tracking.
  - Suggested fix: route test triggers through the same onset path as live hits.

- [ ] **`from_config` silently truncates kits >16 voices** — `src/kit.rs:180`.
  - Why it matters: oversized kits load partially with no error; voices past 16 just disappear.
  - Suggested fix: hard error (or log warning) on overflow; surface to the UI.

- [ ] **`set_mod` dedupe keyed on `(param, source)`** — `src/commands.rs:336`.
  - Why it matters: typos create ghost entries that aren't replaced on the next edit.
  - Suggested fix: validate `param`/`source` against the engine schema before insert.

- [ ] **`set_mod` persistence accretes zero-depth entries forever** — `src/commands.rs:336-341`.
  - Why it matters: TOML grows unbounded with `depth = 0.0` rows; harder to read and slows reloads.
  - Suggested fix: treat `depth == 0.0` as a remove.

---

## P2 — Suspicious patterns

- [ ] **Audio-thread lock contention during kit/preset swap** — `src/audio.rs:21`.
  - Commands hold the `kit` lock for the whole swap; audio thread can stall or drop commands.
  - Fix direction: snapshot/swap pattern with a double-buffered kit pointer.

- [ ] **`shared_state.kit` and `kit.toml` diverge when rtrb command ring is full**.
  - Backpressure isn't surfaced; the UI and engine drift silently.
  - Fix direction: bounded ring with a visible "dropped command" counter.

- [ ] **`LOAD_KIT` doesn't bundle mappings** — slot order A loaded onto mapping B causes silent mis-triggers.
  - Fix direction: persist mappings alongside the kit, or version-pair them.

- [ ] **`from_config` re-adds default mappings after user deletion** — deleted mappings don't stick across reload.
  - Fix direction: track an explicit "user removed" set, or stop merging defaults at load time.

- [ ] **`DrumSound` is a flat `Option<…>` bag** — stale engine-specific fields persist across `engine_type` changes.
  - Symptom: switching engines leaves dead fields in TOML; later switches accidentally re-pick them up.
  - Fix direction: enum-tag per engine (see P3 design item).

- [ ] **`GET_KIT` JSON projection duplicated in 3 places** — `src/commands.rs:54-76,185-208,245-267`.
  - Fix direction: extract a single `kit_to_json(&KitEngine)` helper and call it everywhere.

- [ ] **`noise_color` clamp inconsistent between `set_param` (0.0) and `tick` (0.01)** — `src/hybrid.rs:104,130`.
  - Subtle DC/aliasing differences depending on entry point.
  - Fix direction: define one clamp constant and use it in both paths.

- [ ] **`KitEngine::midi_map` only updated by full rebuild** — drops voice state on every mapping edit. `src/kit.rs:163`, triggered at `src/commands.rs:122,139`.
  - Duplicate of P1#2; tracked separately here as the symptom view.

---

## P3 — Design improvements

- [ ] **Source-of-truth ambiguity**: TOML on disk, `SharedState::kit`, and the rtrb command ring all claim authority.
  - Pick one canonical source (in-memory state) and define the others as views/sinks.

- [ ] **`DrumSound` should be enum-tagged per engine** — flat `Option<…>` bag won't scale as more engines are added.
  - Tagged-union TOML (`engine = "fm"` plus an `[fm]` table) eliminates stale-field bleed.

- [ ] **Split `shared_state.kit: Mutex<KitEngine>`** — mixes mutable per-voice runtime state with mostly-immutable config.
  - Use finer locks (or RCU-style swap) for config; keep voice state on the audio thread.

- [ ] **No `version` field on kit TOML** — silent schema drift between releases.
  - Add `version = N`; refuse to load (or auto-migrate) on mismatch.

---

## Classic Kit Library (queued for build)

TOML presets to land in `presets/kits/`. Twelve kits across three buckets.

**Classic-faithful (4):**

- [ ] **808 Reborn** — Roland TR-808 emulation: FM kick at 50Hz with `mod_ratio 0.5` and `noise 0.05`; cowbell at FM 800Hz + 540Hz detuned-fifth Schmitt-trigger pair (`mod_ratio 0.675`); descending FM toms with strong `pitch_bend`.
- [ ] **909 Warehouse** — TR-909: punchy FM kick with high noise (`0.3`); granular cymbals (small grains, high jitter) to approximate the 6-bit sample texture; FM toms.
- [ ] **Linn Lite** — LinnDrum LM-1 controlled 80s sound: FM kicks with woody decay; Phys-modeled wooden toms (`brightness 0.4`, `dampening 0.7`); long produced clap.
- [ ] **Hexagon** — Simmons SDS-V: FM toms with maximum `pitch_bend` (1500/1200/900/600Hz sweep) for the iconic "pewww" descending toms.

**Modern/hybrid (4):**

- [ ] **Rytm Lab** — Elektron Analog Rytm character: heavier `mod_index`, brighter `noise_color` throughout.
- [ ] **Polar Kick** — Behringer RD-9: dirtier-than-909, extreme noise levels.
- [ ] **Tokyo Toms** — Yamaha RX5 FM-drum character: bell-like inharmonic toms with high `mod_index`.
- [ ] **Volca Hybrid** — analog FM + granular grit split character.

**Wild/experimental (4):**

- [ ] **Karplus Forge** — all-Phys percussion (every drum has resonant tonal pitch).
- [ ] **Grain Dust** — all-Granular textural kit.
- [ ] **Foundry** — industrial/metallic: FM `mod_index` 15–30+ for clangour.
- [ ] **Drift** — ambient/cinematic with slow attack and long decay throughout.

---

## Wacky Kit Library (queued for build)

Five themed wacky kits to ship as TOML presets.

- [ ] **Kitchen Sink Symphony**
  - Wet Drip Kick (FM)
  - Knife-on-Bottle Snare (Hybrid metallic)
  - Sizzle Hat (Granular small grains)
  - Wooden Spoon Tom (Phys low brightness)
  - Plate-Stack Crash (Hybrid max metallic)
- [ ] **Office After Hours**
  - Stapler Kick (FM `ratio 1.0` short decay)
  - Paper-Tear Snare (Noise filtered)
  - Pen-Click Rim (Phys tiny exciter)
  - Typewriter Hat (Granular 4-grain burst)
  - Drawer-Slam Accent (Hybrid wooden + metallic)
- [ ] **Glass Forest**
  - Iceberg Kick (FM low ratio sweep)
  - Wine-Glass Snare (Phys high brightness 750Hz)
  - Hailstone Hat (Granular dense small grains)
  - Ice-Crack Clap (Noise+Hybrid burst)
  - Glass-Bowl Bell (Hybrid very metallic 4s decay)
- [ ] **Ratchet & Wheeze**
  - Beatbox Kick (FM+noise burst)
  - Tongue-Click Rim (Phys short impulse)
  - Wheeze Hat (Noise band-passed 4-8kHz)
  - Servo Snare (Hybrid pitch sweep)
  - Modem Squeal Fill (FM detuned ops + S&H)
- [ ] **Garden at 3 AM**
  - Hollow-Log Kick (Phys 55Hz high damp)
  - Twig-Snap Snare (Noise+Phys exciter)
  - Cricket Hat (Granular modulated rate)
  - Frog-Throat Tom (FM `ratio 1.4` fast index drop)
  - Owl-Hoot Accent (FM with vibrato)

---

## Synth Methods (queued)

Five new engines recommended by the synth-research agent, ranked.

1. [ ] **Modal synthesis (parallel resonator bank)** — N second-order bandpass biquads tuned to Bessel-zero ratios for membranes (1.0, 1.594, 2.136, 2.296, 2.653…) excited by impulse+noise. Each mode = damped sine. Unlocks: realistic toms with tuning, cowbells, blocks, marimba, bells, tabla. ~200 LOC. **HIGHEST PRIORITY — being implemented in this pass.**
2. [ ] **4-/6-op FM with feedback** — Generalize FM to N operators with small "algorithm" routing enum (parallel stack, modulator-chain, feedback on op 1). Unlocks DX7-style cymbals, metallic clangs, Yamaha RX-style snares. ~250 LOC.
3. [ ] **Wavetable with morph-over-envelope** — 2D table (positions × samples-per-cycle), wave position modulatable by envelope/LFO. Unlocks evolving hats, crashes that develop sidebands, vocal-formant toms. ~150 LOC.
4. [ ] **Bitcrusher / SRR as per-voice post-FX** — `floor(x · 2^bits)/2^bits` + hold-and-skip sample rate reducer + tilt EQ. Unlocks SP-1200 / LinnDrum lo-fi character. ~50 LOC. **HIGH PRIORITY — being implemented in this pass.**
5. [ ] **Self-oscillating SVF** — State-variable filter at high Q kicked by impulse. Unlocks 909 hi-hat character, filter-pinged toms, ARP-style bongo. ~80 LOC.

**Honorable mentions:** ring modulation hybrid voice (Simmons clang), phase distortion (Casio CZ), dust/sparse impulse source, waveshaper post-FX, comb filter bank.

**Radical idea (long-term):** Lorenz/Chua chaos oscillators — strange-attractor percussion where velocity crosses bifurcation thresholds and timbre changes character, not just loudness. ~30 LOC.

---

## Identified synthesis gaps for classic kit faithfulness

- [ ] **Multi-tap clap** — real LinnDrum/909 clap is 4 offset noise bursts ~12ms apart. Needed for LinnDrum, Linn Lite, 808 Reborn, 909 Warehouse.
- [ ] **True cymbal machine** — 6 detuned squares/saws 200-800Hz + HPF + optional bell partial. Current best is granular jitter, which misses the bell ping. Needed for 909 ride.

---

## Producer reference tracks (for testing wacky kits)

- [ ] Burial — *Archangel* (Untrue, 2007): rim-shot + clap as backbeat, vinyl crackle as continuous bed.
- [ ] Amon Tobin — *Esther's* (Foley Room, 2007): wasps-in-jar + revving motorbike; drum kit hits submerged in water.
- [ ] Squarepusher — *The Swifty* (Hard Normal Daddy, 1997): Funky Drummer break time-stretched into unrecognizable fragments.
- [ ] RP Boo — *Baby Come On* (Legacy, 2015): triplet stutter is the groove; subtraction as percussion design.
- [ ] Autechre — *Bike* (LP5, 1998): every hit gets a pluck-like short delay.

---

## 7 grooving principles (cheat-sheet from wacky research)

- [ ] Sharp transient first (5 ms locks the ear), decay second.
- [ ] Decay length defines role: <60ms hat, 80-250ms snare, 200ms-1s kick/tom, >1s texture.
- [ ] Inharmonic content lives in the mids (300Hz-4kHz).
- [ ] Pitch instability sells "found" (2-semitone chirp in first 30ms).
- [ ] Ambience IS rhythm (Burial).
- [ ] Subtract a slot — negative space is musical (RP Boo).
- [ ] Layer unexpected with expected (10% real-snare transient under a wasp).

---

## Suggested order of attack

1. P0 kit-paths bug (in progress).
2. P1 in-flight trio (in-memory truth, in-place mapping updates, error-logging persistence).
3. P1 confirmed bugs in source-file order — most are short fixes once truth lives in memory.
4. P2 patterns once the P1 plumbing is in place (several collapse into the P1 fixes).
5. P3 design items as a follow-up milestone; they motivate a kit-schema v2.
