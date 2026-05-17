# drummr Code Smells

Last updated: 2026-05-17

Catalog of patterns that aren't bugs but make the codebase harder to
maintain. Triage by category, not severity — every entry is small to
medium effort to address. Bugs and actual defects are tracked
separately in `docs/bugs.md`.

## Naming

### 1. `Voice::Noise` is the only variant excluded from `set_mod`/`set_lfo`/`get_mod_values` — `src/kit.rs:80-110`
`NoiseVoice` has no `mod_engine`, so every match arm in `Voice` returns `()` / `[0.0; 4]` for it. The asymmetry is fine
behaviour-wise but the silent no-op makes the dispatcher table look like a bug. Suggest either
(a) giving `NoiseVoice` a stub `ModulationEngine` so the arms collapse, or (b) renaming the variant to make the
limitation explicit (`Voice::NoiseOnly`), or (c) introducing an `Engine` trait so the no-op default is a single
trait-method override on `NoiseVoice` rather than five copy-pasted match arms.

### 2. Per-engine "test-only" accessors lack a uniform naming convention — `src/dsp/fm.rs:146`, `src/dsp/phys.rs:245`, `src/dsp/hybrid.rs:198`, `src/dsp/granular.rs:238`, `src/dsp/modal.rs:500-509`
`FmVoice` uses `velocity_for_test()` (the `_for_test` suffix); `PhysEngine`, `HybridEngine`, `GranularEngine`,
`ModalEngine` all use `amp_env_decay_sec()` (no suffix). `ModalEngine` adds a second test accessor `decay_ms()`,
also un-suffixed. Either commit to `_for_test` everywhere or expose `pub(crate)` accessors without the suffix —
but pick one.

### 3. `BeatDivision` variant `Bar` is implicit 4/4 — `src/dsp/timing.rs:48`
Doc says "1 bar (4 beats in 4/4)" but the variant name doesn't encode that assumption. A 6/8 user would reasonably
expect `Bar.to_seconds(120.0)` to return three beats. Suggest renaming to `FourBeats` (and `TwoBars` -> `EightBeats`,
`FourBars` -> `SixteenBeats`) or accept a meter parameter. Low-risk because the project is 4/4-only today, but the
field is named `Bar` not `FourBeats` and that will mislead future maintainers.

### 4. `pub bits` / `pub rate` on `PostFx` — `src/dsp/postfx.rs:5-6`
The fields are `pub` but the only writes happen through `set_bits` / `set_rate` / `set_param`, which apply the
clamp. Direct writes via `postfx.bits = 200.0` bypass the clamp. Should be `pub(crate)` or private with a getter.

### 5. `Voice::name()` mixes display-friendly and engine-key strings — `src/kit.rs:47-56`
`Voice::Fm` -> `"FM"` (uppercase), `Voice::Phys` -> `"Physical Modeling"` (display name), `Voice::Hybrid` -> `"Hybrid"`,
`Voice::Granular` -> `"Granular"`. The values are used in `analyze_sound` as the broadcast payload (`ANALYSIS: {..., engine: ...}`).
The `engine_type` config strings (`"fm"`, `"phys"`, `"granular"`, `"hybrid"`, `"modal"`, `"noise"`) and the display
names are NOT consistent — a UI consumer can't round-trip from the display string back to the config string.

### 6. `Grain._amp` — leading-underscore field never read — `src/dsp/granular.rs:10`
Field is set to `1.0` at grain spawn (`granular.rs:168`) and never read. Either remove it or rename to `amp` and
actually use it (the comment "Increase excitation energy" suggests velocity scaling was once planned).

### 7. `_connection` field name on `SyncEngine` — `src/sync.rs:15`
Underscore prefix usually means "intentionally unused"; the `_connection` is actually load-bearing — it keeps the
midir output port alive. Rename to `_keep_alive: MidiOutputConnection` (clearer) or just `connection` and let
`#[allow(dead_code)]` carry the intent.

## Magic numbers

### 8. `1024` ring-buffer size duplicated four times — `src/main.rs:30,34,331,332`, `src/commands.rs:587-588`
Same constant in five places. Should be a `const RING_BUFFER_CAPACITY: usize = 1024;` in `state.rs` or
a module of its own.

### 9. `128` cpal buffer size hard-coded — `src/audio.rs:16`
`config.buffer_size = cpal::BufferSize::Fixed(128);` — should be a named constant with a comment ("low-latency target
~2.7 ms at 48 kHz"), so a future maintainer knows what the trade-off is.

### 10. 40 ms / 100 ms broadcast intervals — `src/main.rs:82,111`
Two separate broadcast loops at different rates (mod-state at 40 ms, BPM at 100 ms). Both are bare `Duration::from_millis(...)`
in `main.rs` with no named constants and only a passing comment. Promote to `const MOD_STATE_BROADCAST_MS: u64 = 40;`
and `const BPM_BROADCAST_MS: u64 = 100;`.

### 11. 500 ms audio-recovery pacing — `src/main.rs:374`
`tokio::time::sleep(std::time::Duration::from_millis(500))` — explained in the surrounding comment, but a named
constant `AUDIO_RECOVERY_MIN_INTERVAL_MS` makes the intent obvious.

### 12. `0.999` rail threshold duplicated across files — `src/commands.rs:56`, `tests/no_kit_clipping.rs:12`, `tests/granular_engine_tests.rs:21`, `tests/modal_engine_tests.rs:354`, `tests/analysis_tests.rs:233`
Five different files independently declare `const RAIL: f32 = 0.999;` or inline the value. Promote to a single
`pub const CLIPPING_RAIL: f32 = 0.999;` in `audio.rs` or a new `dsp::levels` module so tests and analysis use
the same source.

### 13. `sustained_clip` threshold `100` samples — `src/commands.rs:78`
`let sustained_clip = max_run > 100;` — 100 samples at 48 kHz is ~2 ms, but neither the value nor the rationale is
named. Should be `const SUSTAINED_CLIP_SAMPLES: u32 = 100;` with a "~2 ms at 48 kHz" comment.

### 14. `silent` threshold `0.05` peak — `src/commands.rs:77`
`let silent = peak < 0.05;` — also a magic number. Promote to `const SILENT_PEAK_THRESHOLD: f32 = 0.05;` so the UI
warning rule is documented and tunable.

### 15. Hybrid metallic crossfade `0.85` / `0.15` — `src/dsp/hybrid.rs:159-160`
`let osc_weight = 1.0 - metallic * 0.85;` / `let noise_weight = 0.15 + metallic * 0.85;`. The comment explains the
intent ("15% floor on each side") but the constants are unnamed. `const METALLIC_FLOOR: f32 = 0.15;` and a single
slope constant would make the intent obvious — and prevent the next change from breaking the implied invariant
that the two weights sum to a constant.

### 16. Default `ghost_offset_ms = 60.0`, `ghost_velocity_factor = 0.3` repeated in two places — `src/kit.rs:428-429,520-522`
Both `GenerativeSettings::default()` and the `DEFAULT_GEN` constant in `KitEngine::new` hard-code the same four
defaults. If one drifts the other will silently disagree. Use `DEFAULT_GEN: GenerativeSettings = GenerativeSettings::default()`
once `Default::default()` is callable in const context — until then, factor to one source.

### 17. `Mode` `0.18` decay rolloff & `0.4` gain rolloff — `src/dsp/modal.rs:270,278`
`let mode_decay = base_decay_sec / (1.0 + (i as f32) * 0.18);` and `self.modes[i].base_gain = 1.0 / (1.0 + (i as f32) * 0.4);`.
Both are empirical Bessel-mode rolloffs. Name them (`MODE_DECAY_ROLLOFF`, `MODE_GAIN_ROLLOFF`) so they're discoverable.

### 18. `Xorshift` seeds are bare hex literals — `src/dsp/fm.rs:64`, `src/dsp/phys.rs:58`, `src/dsp/granular.rs:46,62`, `src/dsp/hybrid.rs:43`, `src/dsp/modal.rs:203`, `src/dsp/noise.rs:25`, `src/kit.rs:533`
Seven different magic seeds (`12345`, `0xACE1`, `0x1234`, `0x5678`, `0x9ABC`, `0xBEEF`, `42`, `0xC10C`). They're all
independent so collision isn't a real concern, but the random selection is jarring. Either centralise in a
`SEEDS` module or accept a seed parameter on `Xorshift::new` from a unified source so test determinism is easier
to reason about.

### 19. Modal exciter burst `0.008` (8 ms) — `src/dsp/modal.rs:374`
`self.exciter_total = ((self.sample_rate * 0.008) as usize).max(1);` — named-constant candidate
(`EXCITER_BURST_SEC`).

### 20. Modal `INACTIVITY_RESET_SEC: f32 = 10.0` is `bpm_engine` only — `src/dsp/bpm_engine.rs:13`
`bpm_engine.rs` is exemplary: every magic value is named. Good model for the rest of the DSP folder.

## Duplication

### 21. Engine trigger/LFO/decay-division boilerplate is identical across six engines — `src/dsp/fm.rs:68-91`, `src/dsp/phys.rs:119-157`, `src/dsp/granular.rs:123-141`, `src/dsp/hybrid.rs:99-118`, `src/dsp/modal.rs:347-380`, `src/dsp/noise.rs:31-38`
Every engine's `trigger(velocity, bpm)` opens with the same 6-line decay-division resolution + LFO-division
override block:
```rust
let decay_sec = match self.decay_division {
    Some(div) => div.to_seconds(bpm),
    None => self.decay / 1000.0,
};
self.amp_env.set_params(self.attack / 1000.0, decay_sec);
self.amp_env.trigger();
if let Some(div) = self.lfo1_division { self.mod_engine.set_lfo(1, div.to_hz(bpm)); }
if let Some(div) = self.lfo2_division { self.mod_engine.set_lfo(2, div.to_hz(bpm)); }
```
Six near-identical copies. A free function `apply_tempo_locked(amp_env, mod_engine, attack, decay, decay_div, lfo1_div, lfo2_div, bpm)` would dedupe.

### 22. `voice_from_sound` is 100 lines of per-engine config copy — `src/kit.rs:305-403`
Each of the six engine branches reads the same `lfo1_division`, `lfo2_division`, `decay_division`, `attack`, `decay`
from `DrumSound` into the engine struct. The repeated assignments could be one helper that takes `&mut dyn TempoLocked`.
Same observation as 21 — points to a missing `trait Engine`.

### 23. `Voice` dispatch table is 7 manually-maintained match arms × 7 methods — `src/kit.rs:46-151`
`name`, `schema`, `set_param`, `set_mod`, `set_lfo`, `get_mod_values`, `trigger`, `tick`, `is_active` — every
`Voice` method is a six-arm `match self`. Adding an engine touches 9 places. A `trait Engine: Send` with
default impls and `Voice = Box<dyn Engine>` would collapse all of it.

### 24. `KitEngine::new` constructs `[PostFx; 16]` by manual repetition — `src/kit.rs:497-514`
Rust can't `[PostFx::new(); 16]` because `PostFx` isn't `Copy`. The current code lists `PostFx::new()` 16 times.
Use `std::array::from_fn(|_| PostFx::new())` — much shorter and idiom-correct.

### 25. `cn(...)` defined four times in the UI — `ui/src/App.tsx:6`, `ui/src/views/MappingView.tsx:6`, `ui/src/components/PreviewKitButton.tsx:6`, `ui/src/components/ui.tsx:4`
The canonical export is in `ui/src/components/ui.tsx`; `LibrarySidebar` already imports it from there. Delete the
three duplicates and import from `./components/ui` everywhere.

### 26. `handle_command` is a 600-line `if/else if` chain — `src/commands.rs:145-760`
29 distinct command prefixes handled in one async fn. Most have nearly-identical structure (parse, mutate snapshot,
push to persistence, broadcast). A `HashMap<&str, Handler>` or per-command modules would be a maintainability win.

### 27. WS broadcast/parse strings are duplicated between Rust and TS as bare literals — `src/commands.rs:147,157,167,170,178...` vs `ui/src/App.tsx:101-166`
Every `text == "LIST_MIDI"` and `text.starts_with("SET_PARAM:")` on the Rust side is mirrored by a
`data.startsWith('LIST_MIDI: ')` / `data.startsWith('SET_PARAM:')` on the TS side. No shared constants. A simple
TS `const COMMANDS = { GetKit: 'GET_KIT', ... }` and matching Rust enum, derived once and imported on both ends,
would prevent silent prefix drift (and already happened — `"LIST_MIDI: "` has a trailing space, `"KIT_LIST:"` does
not, this kind of inconsistency is invisible until it bites).

### 28. Ring-buffer rebuild logic duplicated between `commands.rs` and `main.rs` — `src/main.rs:331-365` vs `src/commands.rs:587-634`
Same recreate-rings + swap-producers + `start_audio` + leak-stream + warn-on-leak pattern in two places (audio recovery
task and SELECT_AUDIO handler). Factor to `state::reswap_audio_pipeline(...)`.

### 29. Settings-save boilerplate `Settings::load(); s.last_audio_device = Some(name); let _ = s.save();` in three places — `src/main.rs:260-262,361-363`, `src/commands.rs:638-640`
Three near-identical snippets to persist the active audio device. Extract `Settings::record_audio_device(name: &str)`.

### 30. `kit_to_json` repeats the engine-param table that `voice_from_sound` already knows — `src/commands.rs:92-128`
Inline JSON construction with hard-coded defaults (`mod_ratio.unwrap_or(1.0)`, `density.unwrap_or(0.5)`, etc.).
These defaults are also in `voice_from_sound`. Drift risk: if a default ever changes in one place the wire
contract differs from the runtime. Extract default-resolution into a `DrumSound::resolved()` method.

### 31. Inline `serde_json::json!` payload for ANALYSIS — `src/commands.rs:671-680`
The fields mirror `VoiceAnalysis`'s definition (`peak`, `rms`, `clipped_samples`, ...). Should be `#[derive(Serialize)] struct VoiceAnalysis` so the wire shape is statically guaranteed to match the Rust struct.

## File / function size

### 32. `src/commands.rs` — 761 lines, single `handle_command` function spans 600+ of them — `src/commands.rs:130-761`
Dominant smell in the backend. Hard to read, hard to test (only the harness tests reach it), and every new command
adds to the chain. Split into per-domain modules (`commands/kit.rs`, `commands/preset.rs`, `commands/sync.rs`,
`commands/audio.rs`, ...) or a dispatch table.

### 33. `src/kit.rs` — 901 lines, holding both the `Voice` enum and `KitEngine` — `src/kit.rs:1-902`
Several concerns mashed together: `DrumKit`/`DrumSound`/`DrumMapping` (serde models), `Voice` dispatch enum,
`KitEngine` runtime, generative-trigger logic, and unit tests. Split: `kit/config.rs` (serde), `kit/voice.rs`
(enum), `kit/engine.rs` (runtime), `kit/generative.rs`.

### 34. `src/dsp/modal.rs` — 552 lines — `src/dsp/modal.rs:1-552`
Large single file but the structure is clean (Mode struct → ModalEngine struct → public API → tests). Within
budget but worth a follow-up to extract the explicit-mode-list logic if it grows.

### 35. `ui/src/App.tsx` — 504 lines, 16-prefix onmessage switch — `ui/src/App.tsx:97-182`
Single `onmessage` handler with 16 `startsWith` branches. Each branch is small but together they obscure the
WebSocket-protocol surface. Extract to a `dispatchMessage(data, setters)` helper, or use a `Record<string, (payload: string) => void>` registry.

### 36. `ui/src/views/KitEditorView.tsx` — 539 lines, single component — `ui/src/views/KitEditorView.tsx:59-540`
After the horizontal-signal-flow refactor, the file holds all five sections (Source / Shape / Timbre / Modulation / FX)
inline. Each section is a candidate component. The `selectedSound ? (() => { ... })() : ...` IIFE pattern at line
284 is doing work that would be cleaner as `<SelectedSoundEditor sound={selectedSound} ... />`.

### 37. `ui/src/components/ui.tsx` — 512 lines, multiple unrelated components — `ui/src/components/ui.tsx:1-512`
Holds `cn`, `Card`, `Button`, `Slider`, `Sparkline`, `PredictiveGraph`, `FrequencyVisualizer`, `ParamController`,
`ModSlot`. Should be split into per-component files in `ui/src/components/`.

## Error handling

### 38. `let _ = settings.save()` swallows persistence errors — `src/main.rs:262,363`, `src/commands.rs:640`, `src/app_utils.rs:55`
Settings.toml save failures (disk full, permissions, locked file) are silently ignored. At minimum, log via
`eprintln!` so a user reporting "my port reset on restart" has a diagnostic trail.

### 39. `serde_json::to_string(...).unwrap_or_default()` across `commands.rs` — `src/commands.rs:127,181,208,684`
Every UI broadcast falls back to an empty string on serialise failure. Serde failures on these simple structs are
near-impossible, but the silent-empty pattern means a future field with a non-serialisable type would just produce
empty broadcasts. Use `expect("serialising X is infallible")` so a regression panics loudly.

### 40. `SyncEngine::new` panics if midir output creation fails — `src/sync.rs:23`
`MidiOutput::new("drummr-sync").expect("Failed to create MIDI output");` — a midir failure crashes the entire
engine, but the comment on line 27 says we expect it might fail. Should return `Result` or fall back to a no-op
sync engine.

### 41. `sync.rs` uses `.lock().unwrap()` inside a hot polling thread — `src/sync.rs:75,85`
`while *is_running_shared.lock().unwrap() { ... let is_auto = *auto_sync_shared.lock().unwrap(); ... }`. A poisoned
mutex from a panicking thread would bring this loop down. Acceptable today because writers never panic, but it's
a latent crasher.

### 42. `bpm_engine.rs:86,175` use `.unwrap()` where `if let` would be safer — `src/dsp/bpm_engine.rs:86,175`
`self.onsets.back().unwrap().t` after a `self.onsets.len() < 3` guard — sound today but brittle.
`scores.iter().min_by(...).unwrap()` is reachable if `scores` ever empties for an out-of-range lag config — convert
to `expect("scores non-empty by construction")` so the invariant is documented.

### 43. `kit.rs:578` `swap_remove_back(i).unwrap()` after a manual index check — `src/kit.rs:578`
Same pattern: index is bounds-checked manually one line above, so the unwrap is infallible by construction. Spell
it out: `.expect("index bounded by while-condition")`.

### 44. Persistence-tx sends in `commands.rs` swallowed silently 7 times — `src/commands.rs:221,236,262,302,334,352,425,476,505,543`
`let _ = persistence_tx.send(...)`. If the persistence thread has died, the user gets no warning. Use
`if persistence_tx.send(...).is_err() { eprintln!("persistence channel closed; edits will not be saved"); }` —
once, behind a watchdog, not 10 times.

### 45. `ws.send(...)?.send(...)` in browser code has no try/catch — `ui/src/App.tsx`, `ui/src/views/KitEditorView.tsx`
Every `ws.send(...)` is unchecked. WebSocket send can throw if the socket transitions to `CLOSING` mid-message;
the user sees an uncaught error. Wrap in a helper `safeSend(ws, msg)` that checks `readyState === OPEN` and
swallows / logs the failure.

## Documentation gaps

### 46. `BeatDivision` variant docs are accurate but the module-level doc lacks the meter caveat — `src/dsp/timing.rs:1-11`
The module says "Assumes 4/4" only inside `to_seconds`; bring that warning into the module-level comment
since `Bar`/`TwoBars`/`FourBars` variant names suggest meter-aware behaviour.

### 47. `Voice` enum is `pub` but has zero rustdoc — `src/kit.rs:37-44`
`pub enum Voice { Fm(FmVoice), Phys(...), ... }` — no `///` describing what it represents or why all six variants
share the same trait surface. Add a one-paragraph rustdoc.

### 48. `ParamSchema` is the wire contract with the UI but undocumented — `src/kit.rs:27-33`
Used in `GET_SCHEMA:` broadcasts; the UI relies on `name`/`min`/`max`/`default`/`unit`. No doc comment explains
the unit conventions (Hz vs ms vs ratio vs index vs level vs "").

### 49. `KitEngine` public fields (`pub voices`, `pub postfx`, `pub sample_rate`, etc.) have no rustdoc on the struct itself — `src/kit.rs:451-492`
Individual fields have comments but the struct itself doesn't say "you own the audio-thread mutable state here."

### 50. `AdEnvelope::trigger` has a misleading comment — `src/dsp/envelope.rs:51-53`
"Don't reset value to allow for re-triggering from current level if desired — but for drums, often we just snap to
0 or current." The code doesn't snap to either — it leaves `value` at whatever the previous tick left it (so a
re-trigger mid-decay attacks UP from the current level, which is musically what you want, but the comment reads as
indecision).

### 51. `SharedState::audio_stream_leak_count` is documented; `mod_values` is not — `src/state.rs:8`
The `[AtomicU32; 16 * 5]` is the only inter-thread channel for "live mod values" — that's load-bearing context
that should be on the field doc-comment.

### 52. `KitEngine.pending` is `pub` with a doc comment but no constraint on external mutation — `src/kit.rs:472-476`
Exposed for integration tests to inspect; nothing prevents a future change from `kit.pending.clear()`-ing from
outside the engine. Either make it `pub(crate)` plus a `#[cfg(test)] pub`-getter, or add an explicit
"do not mutate externally" warning.

### 53. `DrumSound`'s trigger-time fields (`sub_hits`, `pattern`, `trigger_probability`, `ghost_*`) ARE documented — `src/kit.rs:264-293`
Good model for the rest of the file. Mention positively here so the next docs sweep emulates the style.

### 54. The 100 ms BPM broadcast loop comments say "10 Hz" but use `100` not `1000/10` — `src/main.rs:111` vs `src/state.rs:11`
`state.rs` doc says "10 Hz BPM broadcast task in `main.rs`" — fine — but the `main.rs` value is `100` ms which is
the inverse of 10 Hz; a quick reader has to mentally convert. Name the constant: `BPM_BROADCAST_HZ = 10` and
derive the ms from it.

### 55. `MAX_SUB_HITS_PER_PRIMARY` is documented but the silent-drop behaviour is opaque — `src/kit.rs:21-22`
Says "silently dropped" — for a UI consumer that means "your 9th clap tap won't fire and you'll wonder why." Add
"the WS broadcast does not warn; check `from_config` truncation logs at startup" or actually emit a warning.

## Allocation hot paths

### 56. `Voice::name()` returns `&'static str` (good); `Voice::schema()` allocates `Vec<ParamSchema>` every call (acceptable, only called off-thread, but worth a note) — `src/kit.rs:58-67`, `src/dsp/*.rs::schema()`
Each engine's `schema()` allocates ~6 `String`s and a `Vec`. Called once per `GET_SCHEMA:` (UI), never from the audio
thread. Fine, but mark the function `pub fn schema(&self) -> Vec<ParamSchema>` with a doc-comment "off-thread only;
allocates" so future audio-thread callers don't sneak in.

### 57. `KitEngine::trigger` clones `SubHit` and `PatternStep` entries — `src/kit.rs:765,782`
`let sub = self.sub_hits[slot][i].clone();` and `let step = self.pattern[slot][i].clone();`. Both are `Copy`-able
small structs (two/three `f32`s). Drop the `.clone()`, mark the structs `#[derive(Copy)]`, save the copy bytecode.

### 58. `KitEngine::tick()` allocations: none in the per-sample path — `src/kit.rs:801-826`
Spot check — confirmed allocation-free. `drain_pending` is also allocation-free (`swap_remove_back` reuses storage).
Audio-thread hot path is clean. Mention here as a baseline so the next change can be measured against it.

### 59. WS broadcast loop allocates a fresh `Vec<Vec<f32>>` every 40 ms — `src/main.rs:85-94`
`let mut values = Vec::with_capacity(16);` then `slot_vals.push(...)` 5 × 16 times. 25 Hz allocation, low impact,
but if it bothers the GC profile during long sessions, replace with `serde_json::to_string` over a
`[[f32; 5]; 16]` array.

### 60. `KitEngine.from_config` rebuilds `midi_map` from scratch on every kit load — `src/kit.rs:655-677`
Not a hot path (only on kit load), but the function name `build_midi_map` is on `&self` and could be `pub fn build_midi_map(voices: &[Option<Voice>; 16], mappings: &[DrumMapping])` as a free function — the `&self` parameter only
reads `self.voices[idx].is_some()`.

## Sync/async mixing

### 61. `app_utils.rs:30` calls `blocking_lock` on a `tokio::sync::Mutex` from a midir thread — `src/app_utils.rs:30`
`let mut bpm = bpm_clone.blocking_lock();` — only safe because the midir callback runs on midir's own thread,
not tokio's. If we ever wrap this in `tokio::spawn`, the blocking lock will deadlock. Add a `#[doc(hidden)]`
inline comment "MUST NOT be called from a tokio worker; midir runs the callback on its own thread".

### 62. `persistence.rs` uses `std::thread::spawn` consuming a tokio mpsc — `src/persistence.rs:15-16`
`while let Some(cmd) = rx.blocking_recv() { ... }` — `blocking_recv` is the documented tokio bridge for non-async
consumers; clean. Worth a one-line comment "uses tokio's documented sync-bridge; safe across runtimes".

## UI smells

### 63. `: any` annotations across UI — 8 occurrences — `ui/src/App.tsx:47,50,116,408,460`, `ui/src/components/LibrarySidebar.tsx:9`, `ui/src/views/KitEditorView.tsx:27,31,40,41,352,462`, `ui/src/views/MappingView.tsx:83,84`
Function-parameter `: any`s (`SidebarContent`, `DashboardView`, `selectedSoundId`) and prop types (`Sound.id: any`,
`[key: string]: any`). For `Sound`, the slot index is a `number`; type it. The `: any` on `selectedSoundId` is
propagated through 5+ components.

### 64. `type f32 = number` inside `ui/src/components/ui.tsx:110` — `ui/src/components/ui.tsx:108-110`
```ts
export interface ModSlotData {
  source: string;
  depth: f32;
}
type f32 = number;
```
Mimicking Rust's `f32` in TypeScript adds no information (TS numbers are all `f64`) and obscures the type. Either
delete the alias and use `number`, or move it into a `wireTypes.ts` module that comments the cross-language
mapping.

### 65. Inline `style={{...}}` instead of Tailwind — `ui/src/components/ui.tsx:97,415,424,431,441,498,506`, `ui/src/components/ModulationPanel.tsx:30,50`, `ui/src/components/MasterPeakMeter.tsx:38`
Most are dynamic values (`left: ${x}%`, `width: ${level * 100}%`) that genuinely need inline styles — those are
fine. The `appearance: 'slider-vertical' as any` on line 498 is the smell: cast-to-any to set a CSS property that
TS doesn't know about. Use a CSS class or update the React typings.

### 66. Prefix strings duplicated and inconsistently formatted — `ui/src/App.tsx:101-166`
`'PORT: '` (with trailing space-after-colon), `'KIT_LIST:'` (no space), `'BPM:'` (no space), `'MIDI: '` (with space).
The format is a Rust-side decision and inconsistent. Pick "always trailing space" or "never" and align both ends.

### 67. `'ANALYSIS:'.length` literal used as a slice offset — `ui/src/App.tsx:130-131`
```ts
if (pipeIdx > 'ANALYSIS:'.length) {
  const slot = parseInt(data.substring('ANALYSIS:'.length, pipeIdx));
```
Three references to `'ANALYSIS:'`, hard to refactor. Hoist into a `const ANALYSIS_PREFIX = 'ANALYSIS:'`.

### 68. `triggerPreview` and `requestAnalysis` share no debouncing semantic — `ui/src/views/KitEditorView.tsx:150-154` and `ui/src/App.tsx:202-213`
The 500 ms debounce is owned by `App.tsx` (`analyzeTimersRef`) but the consumer in `KitEditorView` doesn't know
that. Document the debounce contract on the callback's TSDoc.

### 69. `selectedSound[param.name]` indexed access bypasses type safety — `ui/src/views/KitEditorView.tsx:457,465`
`selectedSound[param.name] ?? param.default` works because `Sound` has `[key: string]: any`. Once the index
signature is removed (per 63), these accesses need a proper discriminated union per engine.

### 70. `setTimeout(..., 80)` / `setTimeout(..., 40)` for MIDI flash — `ui/src/App.tsx:176,179`
Two different flash durations for note-on vs note-off, both unnamed magic numbers. `const FLASH_NOTE_ON_MS = 80;`
and `const FLASH_NOTE_OFF_MS = 40;` with a comment "shorter for note-offs so a sustained pad is visually
distinguishable".

## API surface

### 71. `KitEngine.voices`, `.postfx`, `.sample_rate`, `.midi_map`, `.sub_hits`, `.pattern`, `.generative`, `.pending`, `.samples_processed`, `.rng`, `.last_bpm` are all `pub` — `src/kit.rs:452-492`
The audio thread reaches in to several of these (e.g. `kit.voices.get_mut(slot)` in `audio.rs:34`). The rest are
pub mainly because the integration tests reach in. Many should be `pub(crate)`; a few (`last_bpm`, `rng`, `samples_processed`)
could be encapsulated behind methods.

### 72. `SharedState.kit` and `.kit_snapshot` are `pub` — `src/state.rs:17,21`
Necessary for `commands.rs` (which lives in the same crate) but exposes the locks publicly. Move to `pub(crate)`
once the public-API surface is intentional.

### 73. `Voice` is `pub` and `pub enum` with `pub variants` — `src/kit.rs:37`
External consumers can construct a `Voice::Fm(FmVoice::new(48000.0))` directly. Adding a new variant is a breaking
change to any downstream. Mark `#[non_exhaustive]` or use a builder.

### 74. `start_audio` is `pub fn` returning `Result<cpal::Stream>` — `src/audio.rs:7-13`
Used by both `main.rs` and `commands.rs` (internal callers). `pub(crate)` would be sufficient.

### 75. `comm.rs::subscribe` and `bpm_engine.rs::register_onset_at` are `#[cfg(any(test, feature = "test-helpers"))]` gated — but `subscribe` is not — `src/comm.rs:81`
`subscribe` is `pub fn` always available — not feature-gated. The doc-comment says "Used by integration tests"
but the function is always compiled. If subscribe is meant to be production API (e.g. for the MIDI-player reuse
path), document that; otherwise feature-gate it.

## TODO / FIXME inventory

Source grep `grep -rn "TODO\|FIXME\|XXX\|HACK" src/ tests/ ui/src/`:

- `tests/commands_tests.rs:12-14` — "Out of scope" TODO listing untested commands: `LIST_MIDI`, `LIST_AUDIO`,
  `SELECT_MIDI`, `SELECT_AUDIO`, `TEST_TRIGGER`. Tracked as a known coverage gap.
- `tests/velocity_contract_tests.rs:169` — "If this fails the velocity contract gap from TODO.md has..."
  — references `TODO.md`; live, not stale.

No TODOs / FIXMEs in `src/` or `ui/src/`. The codebase is clean on this dimension.

## Cargo.toml / dependencies

### 76. `lazy_static` is used once (`src/dsp/utils.rs:68`) — `Cargo.toml:20`
For a single static, `OnceLock<FastSine>` (stable since Rust 1.70) avoids the external crate.

### 77. `serde_with` is in `[dependencies]` but only referenced once with `#[serde_as]` and no `serde_as` attributes inside — `src/kit.rs:153-154`
```rust
#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DrumKit { ... }
```
`#[serde_as]` is a no-op on a struct without any `serde_as(...)` attributes on fields. Either drop the import and
the macro or use a `serde_as` annotation on one of the fields (probably `description` or `sounds`).

### 78. `url` and `futures-util` are declared but rarely used — `Cargo.toml:17-18`
A surface grep shows `futures-util` only in `comm.rs` (for `SinkExt`/`StreamExt`). `url` doesn't appear in `src/`
at all. Quick `cargo machete` candidates.

### 79. `[features] test-helpers = []` is correctly used by `bpm_engine.rs` only — `Cargo.toml:28-29`, `src/dsp/bpm_engine.rs:65,205`
The feature gates `register_onset_at` and `get_bpm_at`. `comm.rs::subscribe` is mentioned in the test docs but is
NOT gated. Either gate it for consistency or document why it's intentionally always-available.

## CLAUDE.md / README accuracy

### 80. README says "27 kit presets" but there are 35 — `README.md:16` vs `ls presets/kits/ | wc -l == 35`
The 5 flagship kits added in commit `3a57138` and the 3 demo kits aren't reflected. Bump or replace with
"30+ kits, including demos".

### 81. CLAUDE.md says BPM broadcast loop is at "100 ms" — accurate — `CLAUDE.md:41`
Cross-references `main.rs:111`. Source-of-truth match.

### 82. CLAUDE.md `Adding a new engine` instructions list 4 steps — accurate but worth a note that step 1 (`schema()`, `set_param`, etc.) is exactly the protocol that an `Engine` trait would formalise — `CLAUDE.md:55`
Not wrong, just an opportunity: cross-link to the suggested trait refactor in the strategic refactors section
below.

## Frontend test failures

### 83. Three pre-existing failing frontend tests — `ui/src/App.test.tsx:* `, `ui/src/components/ModulationPanel.test.tsx:*`, `ui/src/views/KitEditorView.test.tsx:*`
`npm test` reports 4 failures, 19 passes. The failures are all "text not found" assertions whose copy has
drifted:
- `App.test.tsx` looks for `/Connecting to drummr engine/i` — no such string in `App.tsx`.
- `App.test.tsx` looks for `/ENGINE CONNECTED/i` — sidebar uses just `"Connected"` now.
- `ModulationPanel.test.tsx` looks for `/LFO 1 Rate/` — current component uses different labels.
- `KitEditorView.test.tsx` looks for `/Base Pitch: 55/` — the test mock predates the `FrequencyVisualizer`
  component split.
The tests document obsolete copy, not real regressions. Either update the assertions to match current copy or
attach `data-testid`s so the tests survive copy changes.

## Tools used

- `wc -l src/**/*.rs ui/src/**/*.{ts,tsx}` to size files.
- `grep -rn "TODO|FIXME|XXX|HACK" src/ tests/ ui/src/` — zero in source.
- `grep -rn "unwrap()" src/` — 9 calls, all reviewed (see error-handling).
- `grep -rn "let _ = " src/` — 33 silent-failure points, mostly in `commands.rs` and `main.rs`.
- `grep -rn ": any" ui/src/` — 8 occurrences.
- `grep -n "DrumSound {" tests/*.rs` — 12 test files build literal `DrumSound`s; helper functions vary.
- `npm test` to identify pre-existing failures.

## Quick-win summary

The 5 highest-leverage cleanups (small effort, big readability win):

1. **Promote the duplicated `1024`, `0.999`, `100ms`, `40ms` magic numbers to named constants** (entries 8–14)
   — single batch edit, ~30 minutes, removes the "is this number meaningful" question from every reader.
2. **Delete the three duplicate `cn(...)` definitions in `App.tsx`, `MappingView.tsx`, `PreviewKitButton.tsx`**
   (entry 25) — 4 lines deleted, 3 imports added, one canonical helper.
3. **Replace `[PostFx::new(); 16]` repetition with `std::array::from_fn(|_| PostFx::new())`** (entry 24)
   — `KitEngine::new` becomes 15 lines shorter.
4. **Update the 4 failing frontend tests to current copy** (entry 83) — restores the "all tests pass" baseline
   so CI signals are meaningful again.
5. **Hoist WS prefix strings into module-level `const`s on both Rust and TS sides** (entries 27, 66, 67)
   — prevents the trailing-space-after-colon inconsistency from biting the next protocol addition.

## Strategic refactors

Bigger items worth their own milestone:

### Engine trait (HIGHEST IMPACT)
Entries 21, 22, 23, 32, 33 all point to the same missing abstraction. A single `pub trait Engine: Send` with
methods `schema`, `set_param`, `set_mod`, `set_lfo`, `trigger`, `tick`, `is_active`, plus a default impl for
`apply_tempo_locked(decay_division, lfo1_division, lfo2_division, bpm)`, would:
- Collapse `Voice`'s nine-arm match dispatch (`kit.rs:46-151`) into trait-method calls
- Remove the per-engine `lfo*_division` / `decay_division` repetition
- Make `voice_from_sound` a single `match engine_type { ... }` returning `Box<dyn Engine>`
- Add a new engine in one file instead of editing six places (engine impl + `Voice` variant + match arms
  in `kit.rs` × 7 methods + UI engine pill list)

Touches ~6 files, ~150 line diff in the engines, but unlocks an enormous amount of future work.

### Split `handle_command` (entry 32)
`commands.rs:130-761` is the dominant readability bottleneck. A `HashMap<&'static str, fn(...) -> Pin<Box<...>>>`
or per-command module would let each command's parsing + mutation + broadcast be one ~30-line function instead
of a branch in a 600-line chain. Pairs naturally with entry 27 (shared command constants).

### Enum-tagged `DrumSound` (the P3 TODO)
`DrumSound`'s ~22 `Option<f32>` fields encode an implicit discriminated union over `engine_type`. The serde
format is already painful; the runtime `unwrap_or(0.5)` defaults in `from_config` and `kit_to_json` drift apart.
Move to:
```rust
enum DrumSound {
    Fm { freq: f32, mod_ratio: f32, ... },
    Phys { freq: f32, brightness: f32, ... },
    ...
}
```
with `#[serde(tag = "engine_type")]`. Big migration (existing kit TOMLs need a converter), but it eliminates
entries 30, 31, 69, and a class of UI casting smells.

### Module split for `kit.rs`
Once the Engine trait is in place, `kit.rs` naturally splits into `kit/config.rs` (serde models), `kit/engine.rs`
(`KitEngine`), `kit/generative.rs` (PendingTrigger, GenerativeSettings, sub-hits / pattern dispatch). Each becomes
testable in isolation.

### Shared protocol constants
Generate the WS command/broadcast prefixes from a single source — either a Rust `enum` whose `Display` impl is
the wire format and a TS `.d.ts` derived from it, or just hand-mirror as `const`s on both sides with a code-review
checklist item. Removes the trailing-space-after-colon class of bugs entirely (entries 27, 66, 67).
