# drummr TODO

Last updated: 2026-05-16

Tracked list of known issues. Ranked P0 → P3. `[x]` = landed (commit hash where known).
Items marked `(in progress)` are being addressed in a parallel implementation pass.

---

## P0 — Blockers

- [x] **Kit paths resolve relative to cwd** — anchored via `env!("CARGO_MANIFEST_DIR")` in `src/main.rs:21-26`. (`b3de2f1`)

---

## P1 — Top architectural priorities

1. [x] **Eliminate disk-as-truth for kit mutations** — mutations now go through in-memory `SharedState::kit` + persistence-worker snapshot. (`2ef61dd`)
2. [x] **Stop rebuilding `KitEngine` for mapping changes** — `KitEngine::set_mapping()` in place, voice state preserved. (`9eed2fa`)
3. [x] **Surface filesystem errors in persistence worker** — `src/persistence.rs` now logs errno + path. (`1d2d128`)
4. [x] **Consolidate triple-defined `kit_to_json`** — single helper at `src/commands.rs:17`. (`2ef61dd`)

---

## P1 — Confirmed Rust bugs

- [ ] **Hybrid engine ignores velocity** — `src/dsp/hybrid.rs:124` returns `mixed * env` with no velocity term. Quiet hits play full volume.
- [ ] **Granular engine ignores velocity** — `src/dsp/granular.rs:166` returns `mixed * env * 0.5` with no velocity term.
- [ ] **`BpmEngine::new(_sample_rate: f32)` param unused** — `src/dsp/bpm_engine.rs:31`; drop from signature after autocorrelation refactor.
- [ ] **cpal stream leaked via `std::mem::forget` on every `SELECT_AUDIO`** — `src/commands.rs:423`, `src/main.rs:174`. Dead streams accumulate on runtime device changes.
- [ ] **Audio stream error callback is `|_err| {}`** — `src/audio.rs:69`. USB unplug / device disconnect / sleep is silent.
- [ ] **3 frontend tests failing** — `ui/src/App.test.tsx` (2 WebSocket lifecycle tests), `ui/src/views/KitEditorView.test.tsx:66` ("Base Pitch" text not found).
- [ ] **`kit.toml` read-modify-write race on `SET_PARAM`** — `src/commands.rs:271-311` (largely subsumed by P1 #1; verify residuals).
- [ ] **Atomic rename uses hardcoded tmp + swallows errors** — `src/persistence.rs:20,28`. Need per-target `*.tmp.<pid>.<nonce>`.
- [ ] **`Voice::Noise` silently no-ops `set_mod`/`set_lfo`** — `src/kit.rs:62,72`. Return Result, reject unhonoured routes.
- [ ] **`SET_PARAM` doesn't refresh `shared_state.kit` for non-engine-type params** — `src/commands.rs:277`.
- [ ] **`cmd_rx.pop()` drops commands when audio thread can't lock** — `src/audio.rs:21`. No retry, no log.
- [ ] **`set_param` silent no-op on unknown names** — `src/fm.rs:133`, `src/phys.rs:190`, `src/granular.rs:177`, `src/hybrid.rs:52,134`. Return `Result<(), UnknownParam>`.
- [ ] **`NoiseVoice` schema empty but `set_param` matches `"attack"`/`"decay"`** — `src/noise.rs:44-54`.
- [ ] **`TEST_TRIGGER` bypasses BPM onset registration** — `src/commands.rs:408` vs `src/app_utils.rs:31`.
- [ ] **`from_config` silently truncates kits >16 voices** — `src/kit.rs:180`.
- [ ] **`set_mod` dedupe keyed on `(param, source)` without validation** — `src/commands.rs:336`.
- [ ] **`set_mod` persistence accretes zero-depth entries forever** — `src/commands.rs:336-341`. Treat `depth == 0.0` as remove.

---

## P1 — Confirmed UI bugs (Phase 2 review)

- [ ] **Engine-type selector missing Modal and Noise** — `ui/src/views/KitEditorView.tsx:250` hardcodes `['fm','phys','granular','hybrid']`. Modal_Demo.toml not editable; clicking pills silently overwrites modal.
- [ ] **PostFx (`bits`/`rate`) has zero UI** — round-trips via KIT JSON but no sliders. Suggested placement: 5th column in `KitEditorView.tsx:241` grid after Modulation.
- [ ] **Auto-Sync placebo from cold start** — `App.tsx:140-144` sends `SET_AUTO_SYNC:true` but `commands.rs:406-408` only flips a flag; master clock thread (`sync.rs:42`) only spawned by SYNC_START. Lazy-spawn in `set_auto_sync(true)` or have UI also send `SYNC_START`.
- [ ] **Only one mod slot per param rendered** — `KitEditorView.tsx:331-358` truncates with `while (displayMods.length < 1)`. Backend `voice.set_mod` is additive.
- [ ] **No per-slot test-trigger button** — `KitEditorView.tsx:175-181` Preview is single-slot; not exposed in slot-tab row or `MappingView.tsx`.
- [ ] **WebSocket reconnect doesn't re-fetch full state** — `App.tsx:61-66` only re-sends `LIST_MIDI/LIST_AUDIO/LIST_KITS/GET_SYNC_STATUS`. Missing: `GET_KIT`, `GET_MAPPING`, `LIST_SOUND_PRESETS`, all `GET_SCHEMA:<slot>`. Editor shows stale data after backend restart.
- [ ] **`selectedSound.attack.toFixed(0)` crashes when schema arrives late** — `KitEditorView.tsx:315`. Same for `decay`. Needs `?? 0`.
- [ ] **`MasterPeakMeter` driven by `isMidiFlashing`, not real audio** — `App.tsx:254`. "Signal Status" tied to syncStatus only. Misleading.
- [ ] **`MappingView.tsx:163` re-requests `GET_MAPPING` on every `KIT:` broadcast** — noisy refresh loop.
- [ ] **LibrarySidebar truncates kit names with no tooltip** — `LibrarySidebar.tsx:182`. 22 kits, single-input filter, no tags/categories.
- [ ] **No "kit dirty" indicator** — save-kit-as is the only save (no overwrite); no signal that in-memory differs from disk.
- [ ] **No error/failure feedback on WS commands** — save kit, load preset, etc. fire-and-forget.

---

## P2 — Suspicious patterns

- [ ] **Audio-thread lock contention during kit/preset swap** — `src/audio.rs:21`. Snapshot/swap with double-buffered kit pointer.
- [ ] **`shared_state.kit` and `kit.toml` diverge when rtrb command ring is full** — surface backpressure with a "dropped command" counter.
- [ ] **`LOAD_KIT` doesn't bundle mappings** — slot order A on mapping B silently mis-triggers. Persist mappings with the kit or version-pair.
- [ ] **`from_config` re-adds default mappings after user deletion** — track "user removed" set or stop merging defaults at load.
- [ ] **`DrumSound` is a flat `Option<…>` bag** — stale engine-specific fields persist across `engine_type` changes. (Motivates P3 enum-tag.)
- [ ] **`noise_color` clamp inconsistent between `set_param` (0.0) and `tick` (0.01)** — `src/hybrid.rs:104,130`. Single shared constant.

---

## P2 — Hygiene & build

- [ ] **`cargo fmt --check` fails repo-wide** — code unformatted.
- [ ] **41 clippy warnings** — ~20 `collapsible_if` (sync.rs:89,104, audio.rs, commands.rs, main.rs:170), missing `Default` impls (MidiEngine, CommEngine, FastSine), `declare_interior_mutable_const` at `state.rs:18`, same-type cast, manual range contains.
- [ ] **3 unused-import warnings** — `src/main.rs:1,5,13`: `ModSource`, `KitEngine/DrumKit/DrumMapping/DrumSound`, `PersistenceCommand`.
- [ ] **38 ESLint errors in `ui/`** — mostly `@typescript-eslint/no-explicit-any`, empty-block, unused `e`.
- [ ] **`npm run lint` returns non-zero**.
- [ ] **README.md `:7-9` lists only FM + Phys** — Granular, Hybrid, Modal, Noise, PostFx, BPM detection, sync engine, 22 kit presets all unmentioned.
- [ ] **CLAUDE.md `:47-53` lists 5 engines, omits Modal** — also says modulation broadcast is "16 × 5" but `get_mod_values` returns `[f32; 4]` (kit.rs:83). Still describes pre-refactor "kit.toml as source of truth" model.
- [ ] **`.gitignore` only `/target`** — missing `kit.toml.tmp`, `mapping.toml.tmp`, `settings.toml`, `ui/settings.toml`.
- [ ] **`settings.toml` is committed AND machine-specific** — currently holds user's SONY TV / MacBook Pro Speakers / DDTi MIDI 1 / MPK mini 3. Convert to `settings.example.toml` + gitignored real `settings.toml`.

---

## P2 — Missing test coverage

- [ ] **ModalEngine** — only 2 inline unit tests in `src/dsp/modal.rs:347`; no `tests/modal_engine_tests.rs`.
- [ ] **PostFx** — only inline tests in `src/dsp/postfx.rs:78`; no `tests/postfx_tests.rs`. Per-slot routing in `kit.rs:351` untested end-to-end.
- [ ] **BPM engine** — no `tests/bpm_engine_tests.rs`. Autocorrelation + tactus + sub-harmonic logic uncovered.
- [ ] **`commands.rs`** — entire 440-line WS dispatcher has zero coverage. SET_PARAM, LOAD_KIT, SET_MOD, SET_BITS/SET_RATE round-trips untested.
- [ ] **`persistence.rs`** — untested.
- [ ] **Groove MIDI Dataset corpus** — zero references in `tests/`. BPM accuracy untested against real beats.

---

## P3 — Design / architectural

- [ ] **Source-of-truth ambiguity** — TOML on disk, `SharedState::kit`, rtrb ring all claim authority. Pick one canonical (in-memory) and define others as views/sinks.
- [ ] **`DrumSound` should be enum-tagged per engine** — flat `Option<…>` bag won't scale. Tagged-union TOML (`engine = "fm"` + `[fm]` table) eliminates stale-field bleed.
- [ ] **Split `shared_state.kit: Mutex<KitEngine>`** — mixes per-voice runtime state with mostly-immutable config. Finer locks or RCU swap.
- [ ] **No `version` field on kit TOML** — silent schema drift between releases.
- [ ] **Fully schema-driven UI** — UI is half schema-driven (`KitEditorView.tsx:331`). Migrate via `GET_SCHEMA`; add `LIST_ENGINES` WS command; add `category` + `display_name` optional fields to `ParamSchema`. Removes ~30% of `KitEditorView.tsx` and kills "backend ships new engine → UI silently broken" drift.
- [ ] **Velocity contract across engines** — FM scales at tick (`fm.rs:106`), Phys at trigger via excitation_amp (`phys.rs:121`), Hybrid + Granular ignore it. Codify in a Voice trait or shared envelope-velocity convention.
- [ ] **SyncEngine thread detached, never joined** — `src/sync.rs:57`. Stop sets a flag with no clean shutdown.
- [ ] **`comm_engine.start().await?` ordering at `main.rs:130`** — verify it doesn't block (audio/MIDI init below would be dead code if so; functional evidence suggests it doesn't).
- [ ] **No structured logging** — `println!`/`eprintln!` everywhere. Add `tracing` + `RUST_LOG` filter.
- [ ] **No CI pipeline** — `.github/workflows/` absent. Nothing enforces fmt / clippy / lint / test.
- [ ] **WS protocol unversioned** — plain-text prefix dispatcher with no handshake. Add `HELLO:v1` ↔ `OK:v1`.
- [ ] **No LICENSE file** — repo defaults to all-rights-reserved.
- [ ] **Packaging paths anchored to `CARGO_MANIFEST_DIR`** — `presets/`, `kit.toml`, `mapping.toml`, `settings.toml` won't survive a packaged release binary. Make `DRUMMR_HOME` env var optional.
- [ ] **Modal engine `_ = env_active` at `src/dsp/modal.rs:297`** — leftover from refactor; either early-out when fully decayed or remove.

---

## Classic Kit Library

All twelve TOML presets landed in `presets/kits/`.

**Classic-faithful (4):**

- [x] **808 Reborn** — TR-808 emulation: FM kick at 50Hz, cowbell as FM 800/540Hz Schmitt pair, descending FM toms.
- [x] **909 Warehouse** — TR-909: punchy FM kick with high noise; granular cymbals; FM toms.
- [x] **Linn Lite** — LinnDrum LM-1: FM kicks; Phys wooden toms; long produced clap.
- [x] **Hexagon** — Simmons SDS-V: FM toms with maximum `pitch_bend` sweep.

**Modern/hybrid (4):**

- [x] **Rytm Lab** — Analog Rytm character.
- [x] **Polar Kick** — Behringer RD-9: dirtier-than-909, extreme noise.
- [x] **Tokyo Toms** — Yamaha RX5 FM-drum character.
- [x] **Volca Hybrid** — analog FM + granular grit split.

**Wild/experimental (4):**

- [x] **Karplus Forge** — all-Phys percussion.
- [x] **Grain Dust** — all-Granular textural kit.
- [x] **Foundry** — industrial/metallic FM `mod_index` 15–30+.
- [x] **Drift** — ambient/cinematic.

---

## Wacky Kit Library

All five themed kits landed.

- [x] **Kitchen Sink Symphony** — Wet Drip Kick, Knife-on-Bottle Snare, Sizzle Hat, Wooden Spoon Tom, Plate-Stack Crash.
- [x] **Office After Hours** — Stapler Kick, Paper-Tear Snare, Pen-Click Rim, Typewriter Hat, Drawer-Slam Accent.
- [x] **Glass Forest** — Iceberg Kick, Wine-Glass Snare, Hailstone Hat, Ice-Crack Clap, Glass-Bowl Bell.
- [x] **Ratchet & Wheeze** — Beatbox Kick, Tongue-Click Rim, Wheeze Hat, Servo Snare, Modem Squeal Fill.
- [x] **Garden at 3 AM** — Hollow-Log Kick, Twig-Snap Snare, Cricket Hat, Frog-Throat Tom, Owl-Hoot Accent.

Plus: [x] **Modal_Demo** preset showcasing the modal engine.

---

## Synth Methods

1. [x] **Modal synthesis (parallel resonator bank)** — landed; Bessel-zero ratios; ~200 LOC. Now applied across multiple kits via the 3 upgrade commits.
2. [ ] **4-/6-op FM with feedback** — DX7-style cymbals, metallic clangs, Yamaha RX-style snares. ~250 LOC.
3. [ ] **Wavetable with morph-over-envelope** — evolving hats, sideband crashes, vocal-formant toms. ~150 LOC.
4. [x] **Bitcrusher / SRR per-voice post-FX** — landed; SP-1200 / LinnDrum lo-fi character. Applied across existing kits via the 3 upgrade commits.
5. [ ] **Self-oscillating SVF** — 909 hi-hat character, filter-pinged toms. ~80 LOC.

**Honorable mentions:** ring modulation hybrid voice (Simmons clang), phase distortion (Casio CZ), dust/sparse impulse source, waveshaper post-FX, comb filter bank.

**Radical idea (long-term):** Lorenz/Chua chaos oscillators — bifurcation-driven timbre changes with velocity. ~30 LOC.

---

## Identified synthesis gaps for classic kit faithfulness

- [ ] **Multi-tap clap** — real LinnDrum/909 clap is 4 offset noise bursts ~12ms apart.
- [ ] **True cymbal machine** — 6 detuned squares/saws 200-800Hz + HPF + optional bell partial. Granular jitter misses the bell ping.

---

## Producer reference tracks (for testing wacky kits)

- [ ] Burial — *Archangel* (Untrue, 2007): rim-shot + clap as backbeat, vinyl crackle bed.
- [ ] Amon Tobin — *Esther's* (Foley Room, 2007): wasps-in-jar + revving motorbike; submerged drum hits.
- [ ] Squarepusher — *The Swifty* (Hard Normal Daddy, 1997): time-stretched Funky Drummer.
- [ ] RP Boo — *Baby Come On* (Legacy, 2015): triplet stutter as groove; subtraction as percussion.
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

1. P1 velocity bugs (Hybrid + Granular) — one-line fixes with audible impact.
2. P1 UI bugs in `KitEditorView.tsx` — engine selector, PostFx UI, mod slots, schema-late crashes.
3. P1 WS reconnect re-fetch — eliminates stale-data class.
4. P1 audio cleanup — cpal leak + error callback + Auto-Sync semantics.
5. P2 hygiene sweep — fmt, clippy, eslint, gitignore, settings.example.toml — unblocks CI.
6. P2 test coverage for Modal / PostFx / BPM / commands / persistence.
7. P3 design milestones — schema-driven UI + enum-tagged DrumSound + structured logging together motivate kit-schema v2.
