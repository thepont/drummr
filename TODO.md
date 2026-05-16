# drummr TODO

Last updated: 2026-05-16

Tracked list of known issues. Ranked P0 → P3. `[x]` = landed (commit hash where known).
Items marked `(in progress)` are being addressed in a parallel implementation pass.

---

## P0 — Blockers

- [x] **Kit paths resolve relative to cwd** — anchored via `env!("CARGO_MANIFEST_DIR")` in `src/main.rs:21-26`. (`b3de2f1`)

---

## Recently landed (Phase 3 DSP / audio polish)

- [x] **`hybrid.rs:126` metallic=1.0 nukes oscillator path** — metallic blend no longer zeros the oscillator. (`54385ee`)
- [x] **`modal.rs` voices saturate to 1.0** — output headroom trimmed and retuned for low-Q kicks. (`f63ce39`, `4b96fbc`)
- [x] **`modal.rs` `is_active` doesn't honour mode-bank tail** — fixed in modal headroom pass. (`f63ce39`)
- [x] **PostFx decimator state held across voice-inactive boundaries** — decimator now resets on voice trigger. (`ebe2cf2`)
- [x] **Audio recovery task has no backoff** — 500ms backoff added. (`3a78fde`)
- [x] **Modal bandpass impulse response inaudible** — switched to constant-skirt-gain bandpass. (`12324c9`)
- [x] **MIDI NoteOff cut drum tails short** — NoteOff now ignored so voices ring out. (`4be1763`)
- [x] **`SELECT_AUDIO` ring buffers not rebuilt** — device switching restored. (`02eb7fe`)
- [x] **Foundry Clap / pipe sound-alikes** — kit voices differentiated via TOML pass. (`76a4245`)

---

## P1 — Top architectural priorities

1. [x] **Eliminate disk-as-truth for kit mutations** — mutations now go through in-memory `SharedState::kit` + persistence-worker snapshot. (`2ef61dd`)
2. [x] **Stop rebuilding `KitEngine` for mapping changes** — `KitEngine::set_mapping()` in place, voice state preserved. (`9eed2fa`)
3. [x] **Surface filesystem errors in persistence worker** — `src/persistence.rs` now logs errno + path. (`1d2d128`)
4. [x] **Consolidate triple-defined `kit_to_json`** — single helper at `src/commands.rs:17`. (`2ef61dd`)

---

## P1 — Confirmed Rust bugs

- [x] **Hybrid engine ignores velocity** — `src/dsp/hybrid.rs:124` returns `mixed * env` with no velocity term. Quiet hits play full volume. (`e81dea7`)
- [x] **Granular engine ignores velocity** — `src/dsp/granular.rs:166` returns `mixed * env * 0.5` with no velocity term. (`e81dea7`)
- [x] **`BpmEngine::new(_sample_rate: f32)` param unused** — `src/dsp/bpm_engine.rs:31`; drop from signature after autocorrelation refactor. (`817b6b4`)
- [~] **cpal stream leaked via `std::mem::forget` on every `SELECT_AUDIO`** — `src/commands.rs:423`, `src/main.rs:174`. Logging + leak counter added (`448c1fd`); architectural fix deferred (cpal::Stream is !Send).
- [x] **Audio stream error callback is `|_err| {}`** — `src/audio.rs:69`. USB unplug / device disconnect / sleep is silent. (`448c1fd`, `d6ee7d5`)
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
- [x] **`set_mod` persistence accretes zero-depth entries forever** — `src/commands.rs:336-341`. Treat `depth == 0.0` as remove. (`e07d3f5`)

---

## P1 — Confirmed UI bugs (Phase 2 review)

- [x] **Engine-type selector missing Modal and Noise** — `ui/src/views/KitEditorView.tsx:250` hardcodes `['fm','phys','granular','hybrid']`. Modal_Demo.toml not editable; clicking pills silently overwrites modal. (`0c480cf`)
- [x] **PostFx (`bits`/`rate`) has zero UI** — round-trips via KIT JSON but no sliders. Suggested placement: 5th column in `KitEditorView.tsx:241` grid after Modulation. (`0c480cf`)
- [x] **Auto-Sync placebo from cold start** — `App.tsx:140-144` sends `SET_AUTO_SYNC:true` but `commands.rs:406-408` only flips a flag; master clock thread (`sync.rs:42`) only spawned by SYNC_START. Lazy-spawn in `set_auto_sync(true)` or have UI also send `SYNC_START`. (`1b187b4`)
- [x] **Only one mod slot per param rendered** — `KitEditorView.tsx:331-358` truncates with `while (displayMods.length < 1)`. Backend `voice.set_mod` is additive. (`0c480cf`)
- [ ] **No per-slot test-trigger button** — `KitEditorView.tsx:175-181` Preview is single-slot; not exposed in slot-tab row or `MappingView.tsx`.
- [x] **WebSocket reconnect doesn't re-fetch full state** — `App.tsx:61-66` only re-sends `LIST_MIDI/LIST_AUDIO/LIST_KITS/GET_SYNC_STATUS`. Missing: `GET_KIT`, `GET_MAPPING`, `LIST_SOUND_PRESETS`, all `GET_SCHEMA:<slot>`. Editor shows stale data after backend restart. (`f51f74a`)
- [x] **`selectedSound.attack.toFixed(0)` crashes when schema arrives late** — `KitEditorView.tsx:315`. Same for `decay`. Needs `?? 0`. (`0c480cf`)
- [ ] **`MasterPeakMeter` driven by `isMidiFlashing`, not real audio** — `App.tsx:254`. "Signal Status" tied to syncStatus only. Misleading.
- [x] **`MappingView.tsx:163` re-requests `GET_MAPPING` on every `KIT:` broadcast** — noisy refresh loop. (`f51f74a`)
- [x] **LibrarySidebar truncates kit names with no tooltip** — `LibrarySidebar.tsx:182`. 22 kits, single-input filter, no tags/categories. (`f51f74a`)
- [ ] **No "kit dirty" indicator** — save-kit-as is the only save (no overwrite); no signal that in-memory differs from disk.
- [ ] **No error/failure feedback on WS commands** — save kit, load preset, etc. fire-and-forget.

---

## P1 — Kit differentiation (empirical findings)

All 22 kit presets sound homogeneous despite distinct themes. Audit across `presets/kits/*.toml`:

- [ ] **72% of voices share `attack = 1.0` ms** — 238 of 329 voices. Identical transient = identical ear-lock across every kit. Only `Drift.toml` systematically varies attack times.
- [ ] **Zero kits use the mod matrix or LFOs** — `grep` finds no `mods` arrays and no `lfo1_freq`/`lfo2_freq` in any TOML. Full system (`src/dsp/modulation_engine.rs`, `AudioCommand::SetLfo`) shipped, completely unused at the preset layer.
- [ ] **PostFx dead in 16/22 kits** — only Linn Lite, Office, Ratchet & Wheeze, Foundry (partial), Volca Hybrid (partial), 909 cymbals apply bitcrush. Industrial Glitch, Tokyo Toms, Hexagon, Neon Night all run pristine 16-bit despite advertising gritty character.
- [ ] **Kick FM-monoculture** — 17/22 kits use FM at slot 0 in 40-85Hz with `mod_ratio` 0.5-1.0 and `mod_index` 1.5-15. The 5 Phys-kick kits read as a categorically different family — proof of engine diversity's payoff.
- [ ] **Master path is mono, dry, identical** — `src/audio.rs:61-67`: sum → ×0.7 → tanh. No EQ, no reverb, no compression, no stereo, no panning. Kits cannot differentiate by mix character.
- [ ] **Hybrid mode ratios `[1.0, 1.52, 2.11]` hardcoded** — `src/dsp/hybrid.rs:108`. Every Hybrid voice across every kit shares the same inharmonic fingerprint regardless of `freq`/`metallic`/`noise_color`.
- [ ] **Inharmonicity clusters in 0.4-0.6 and 0.85-0.95** — only 5 voices below 0.1, none above 0.95. Range under-used; push extremes for radical character difference.

---

## P1 — Track A: TOML-only differentiation (no Rust changes)

Aggressive use of systems already shipped. ~1 day total. No regression risk.

- [ ] **Wider PostFx application — kit-wide for "should be dirty" kits:**
  - `Industrial_Glitch.toml` — `bits = 5.0, rate = 3.0` kit-wide.
  - `Tokyo_Toms.toml` — `bits = 12.0, rate = 2.0` kit-wide (RX5 was 12-bit @ 25kHz).
  - `Hexagon.toml` — `bits = 8.0` kit-wide (Simmons was 8-bit).
  - `Polar_Kick.toml`, `Rytm_Lab.toml` — at least cymbal-side crush.
- [ ] **First-pass mod matrix usage — one assignment per kit, minimum:**
  - `808_Reborn.toml` slot 0: `mod_index` ← Envelope depth +5.0 (click-then-thud sweep).
  - `Glass_Forest.toml` slot 1 (Wine Glass): `lfo1_freq = 4.5`, `freq` ← Lfo1 ±0.5% (vibrato).
  - `Drift.toml` slot 2 (CH): `lfo1_freq = 0.3`, `grain_size` ← Lfo1 ±20ms (swirl).
  - `Foundry.toml` slot 8 (Bell of Doom): `inharmonicity` ← Velocity depth +0.1.
  - `Hexagon.toml` slot 0 (Hex Kick): `freq` ← Envelope depth +400Hz (Simmons pew).
- [ ] **Polarise inharmonicity per kit:**
  - `Karplus_Forge.toml` Marimba Toms (slots 4-7): `inharmonicity = 0.02` (pure harmonic xylophone).
  - `Glass_Forest.toml` Wine Glass / Glass Bowl / Singing Bowl: `inharmonicity = 0.98`.
  - `Foundry.toml` Anvil Toms 1-4: vary 0.70/0.85/0.95/1.0 per-tom (break tom-set uniformity).
- [ ] **Attack-time variety inside kits — break the 1.0ms monoculture:**
  - `Neon_Night.toml` — Bit Click 0.1, Glow Shaker 30, Phase Conga 8, Sub Zap 0.05.
  - `Tokyo_Toms.toml` — bell-toms 5/8/12/15ms.
- [ ] **Frequency footprint shifts — give kits non-overlapping spectral identities:**
  - `Office_After_Hours.toml` — hats to 3-4kHz, kick up to 110-130Hz (desk-thump not subs).
  - `Glass_Forest.toml` — push everything up: kick 110Hz, whole kit 110Hz-8kHz (weightless).
  - `Kitchen_Sink_Symphony.toml` — kick 90Hz, cymbal 4kHz, nothing above 6kHz.

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

- [x] **ModalEngine** — integration suite landed in `tests/modal_engine_tests.rs`. (`70e70d8`)
- [x] **PostFx** — integration suite landed in `tests/postfx_tests.rs`; per-slot routing covered. (`70e70d8`)
- [x] **BPM engine** — `tests/bpm_engine_tests.rs` covers autocorrelation + tactus + sub-harmonic logic. (`7625557`)
- [x] **`commands.rs`** — WS dispatcher coverage landed in `tests/commands_tests.rs`. (`39a7f34`)
- [x] **`persistence.rs`** — atomic write + worker resilience tests in `tests/persistence_tests.rs`. (`c641dfe`)
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

## P2/P3 — Track B: Engine + architecture for kit differentiation

Items unlocked by the empirical findings above. Ordered by impact-per-effort.

- [ ] **Stereo output + per-voice pan + per-kit master plate reverb with per-voice send** (P2, the bold move) — `src/audio.rs:61-67` currently sums all voices mono and applies one tanh. Stereo+pan+send is the single biggest "kits feel different" unlock; same 22 kits would sound like 22 different rooms before changing any voice param. Affects: `audio.rs`, `kit.rs` (`DrumSound` gains `pan`, `reverb_send`; `DrumKit` gains `[master]` table with `reverb_size`, `reverb_decay`, `master_gain`), `commands.rs` (new SET_PAN / SET_SEND / SET_MASTER_*), UI (master panel).
- [ ] **Per-voice level/gain** (P2) — currently every voice plays at `amp_env × velocity` only. No mix balance possible at the preset layer. Add `level: Option<f32>` to `DrumSound`, multiply in `KitEngine::tick`.
- [ ] **Per-voice drive/saturation on FM and Phys** (P2) — most percussion timbre difference comes from harmonic distortion at the transient. Add `drive: Option<f32>` to `DrumSound`; `tanh(x * (1 + drive*4))` after the engine's tick. For Phys, optionally inject a nonlinearity inside the K-S loop (`src/dsp/phys.rs:160-168`) — turns ringing-string into buzzing-sitar.
- [ ] **Configurable Hybrid mode ratios** (P2) — `src/dsp/hybrid.rs:108` hardcodes `[1.0, 1.52, 2.11]`. Promote to three modulatable params (`ratio_b`, `ratio_c` or similar) so snare-hybrid (1.0/1.78/2.45), metallic-hybrid (1.0/2.76/5.4), wooden-hybrid (1.0/1.41/1.95) stop sharing a fingerprint.
- [ ] **Expose FM pitch envelope per-voice** (P2) — `src/dsp/fm.rs:35-36` hardcodes `pitch_env.set_params(0.001, 0.05)`. Promote `pitch_bend`, `pitch_attack`, `pitch_decay` to per-voice TOML params so Hexagon delivers the Simmons sweep it advertises.
- [ ] **Per-kit master FX chain** (P3) — `[master]` TOML section: `eq_low`, `eq_mid`, `eq_high`, `reverb_send_default`, `saturation`. A 3-band tilt EQ alone would let 909 Warehouse vs Kitchen Sink read as different mixes, not different patches in the same mix.
- [ ] **Character macros** (P3, UX) — single `character` knob per voice mapping to drive + brightness + slight detune. Cheap discovery for new users; no DSP work beyond mapping.

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

1. **Kit differentiation Track A** — TOML-only sweep using mod matrix + LFOs + PostFx + attack variety + frequency shifts. ~1 day, zero risk, huge audible impact.
2. P1 velocity bugs (Hybrid + Granular) — one-line fixes; combine with Track A so velocity mod-matrix routes actually do something.
3. **Kit differentiation Track B step 1: stereo + pan + master reverb send** — the bold move. Single biggest "kits feel different" unlock.
4. P1 UI bugs in `KitEditorView.tsx` — engine selector, PostFx UI, mod slots, schema-late crashes.
5. P1 WS reconnect re-fetch — eliminates stale-data class.
6. **Track B step 2: per-voice drive + configurable Hybrid ratios + FM pitch env exposure** — completes the "harmonic character" axis.
7. P1 audio cleanup — cpal leak + error callback + Auto-Sync semantics.
8. P2 hygiene sweep — fmt, clippy, eslint, gitignore, settings.example.toml — unblocks CI.
9. P2 test coverage for Modal / PostFx / BPM / commands / persistence.
10. P3 design milestones — schema-driven UI + enum-tagged DrumSound + structured logging together motivate kit-schema v2.
