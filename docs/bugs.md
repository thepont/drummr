# drummr Bug Log

Last updated: 2026-05-17 (today)

This file aggregates findings from a focused bug hunt across the codebase
post-clock-aware feature shipping. Issues are categorized by severity:

- **HIGH** — incorrect behavior under normal usage; can produce wrong audio / crashes / data loss
- **MEDIUM** — wrong behavior under edge cases; recoverable
- **LOW** — minor inconsistencies, off-by-ones with no audible impact, doc/code mismatches

For each: a one-line summary, file:line, expected vs actual, reproduction notes, suggested fix description.

## HIGH

### 1. MIDI-track BPM publication is silently clobbered ~100 ms after playback starts — `src/main.rs:110-121`
**Symptom**: The whole point of commit `28da264` ("preview-kit playback publishes track BPM to SharedState") is defeated by the always-on broadcast loop in `main.rs`. The MIDI player calls `shared_state.store_bpm(parsed.bpm)` in `midi_player.rs:212`, but the tokio task in `main.rs` runs every 100 ms and overwrites `current_bpm_bits` with `BpmEngine::get_bpm()` (or 120.0 fallback). Live MIDI input is never fed by the player's note-stream (the BpmEngine is only seeded by `start_midi`'s callback), so `get_bpm()` stays at 0.0 → fallback 120.0 stomps the track's authoritative tempo within one tick.
**Repro**: Load a clock-aware kit (e.g. `Cathedral_Forever` whose Modal voices use `decay_division = "TwoBars"`). Press the Preview-Kit button on `rock_140_fill` (or any non-120-BPM track). Within 100 ms the resonator decays revert to 120-BPM-locked durations instead of the track's 140 BPM.
**Root cause**: `main.rs:114-119` unconditionally calls `store_bpm(effective)` every 100 ms. There is no "MIDI playback in progress, do not stomp" check.
**Suggested fix**: Have `spawn_playback` set a "playback owns BPM" flag on `SharedState` (or use a higher-priority atomic). The 100 ms broadcast loop should respect that flag and skip the `store_bpm` call. Clear the flag in `on_finish` and on `STOP_MIDI_PLAYBACK`. Existing test `test_playback_writes_track_bpm_to_shared_state` does not catch this — it asserts the value immediately after `spawn_playback`, before the broadcast loop runs.

### 2. Sub-hits / pattern steps / ghosts at `velocity_factor = 0.0` silence the still-ringing primary — `src/dsp/fm.rs:68-91`, `src/dsp/phys.rs:119-157`, `src/dsp/granular.rs:123-141`, `src/dsp/hybrid.rs:99-118`, `src/dsp/modal.rs:347-380`
**Symptom**: Every engine's `trigger()` unconditionally executes `self.velocity = velocity;` (FM/Granular/Hybrid) or `self.mod_engine.velocity = velocity;` (Phys/Modal) BEFORE the `if velocity > 0.0` guard. A pending sub-hit / pattern step that resolves to `velocity * factor` where `factor == 0.0` (or where `velocity * factor` rounds to 0) will not restart the envelope — but it WILL overwrite `self.velocity` to 0. Because `tick()` multiplies output by `self.velocity` (FM `out * amp * self.velocity` on line 130; Noise on line 47; Granular/Hybrid via similar paths), the still-decaying primary is muted from that sample onward.
**Repro**: Build a `DrumSound` with `sub_hits = [{ offset_ms = 10.0, velocity_factor = 0.0 }]` and `decay = 500.0`. Trigger at velocity 1.0. Audio plays for 10 ms, then goes silent for the remaining ~490 ms even though the AD envelope is still running.
**Root cause**: The "velocity-write" runs outside the `velocity > 0.0` gate.
**Suggested fix**: Move the `self.velocity = velocity;` and `self.mod_engine.velocity = velocity;` lines INSIDE the `if velocity > 0.0` block. (Alternatively: don't even queue pending entries when the effective velocity rounds to zero.)

### 3. NoiseVoice initialises envelope in seconds while consuming caller-supplied milliseconds — `src/dsp/noise.rs:20`, `src/kit.rs:369`
**Symptom**: `NoiseVoice::new` calls `amp_env.set_params(1.0, 50.0)` — `AdEnvelope::set_params` expects SECONDS, so the constructor wires up a 1-second attack and **50-second** decay. Then `voice_from_sound` (kit.rs:369) does `v.amp_env.set_params(sound.attack, sound.decay)`, passing the raw `attack`/`decay` fields which are documented as MILLISECONDS everywhere else in the codebase. A sound with `attack = 1.0, decay = 100.0` (intended as 1 ms / 100 ms) becomes 1 s attack / 100 s decay. By comparison, the other engines explicitly do `attack / 1000.0` and `decay / 1000.0` at trigger time.
**Repro**: Edit any kit TOML to set `engine_type = "noise"` on a slot, set `attack = 1.0, decay = 100.0`. The voice takes ~100 seconds to decay. (Dormant in production today: no shipped kit uses `engine_type = "noise"` because `KitEngine::from_config` doesn't have a `"noise"` branch — only `voice_from_sound` (used by the ANALYZE_SLOT path) does. So a user has to deliberately type the engine name to hit this.)
**Root cause**: NoiseVoice was written before the ms→sec convention was normalised on the trigger side and `voice_from_sound` doesn't apply the divide.
**Suggested fix**: Either (a) divide `sound.attack` / `sound.decay` by 1000.0 in `voice_from_sound`'s noise branch, or (b) move the `/ 1000.0` conversion inside `NoiseVoice::trigger` so it matches every other engine.

### 4. NoiseVoice retriggers the envelope even at velocity 0 — `src/dsp/noise.rs:31-38`
**Symptom**: `NoiseVoice::trigger` lacks the `if velocity > 0.0` guard that every other engine has. A pending sub-hit / pattern fire with `velocity = 0` will both stomp `self.velocity` AND call `self.amp_env.trigger()`, restarting attack from the middle of a decay. This is a strictly worse variant of bug #2.
**Repro**: Same as bug #2, with `engine_type = "noise"`.
**Root cause**: Missing velocity gate on retrigger.
**Suggested fix**: Wrap the trigger body in `if velocity > 0.0 { ... }` like the other engines.

### 5. Modal engine permanently overwrites `self.decay` when `decay_division` is set — `src/dsp/modal.rs:354-356`
**Symptom**: Unlike FM/Phys/Granular/Hybrid which read `self.decay` only as a fallback and compute the resolved value into a local `decay_sec`, Modal does `self.decay = div.to_seconds(bpm) * 1000.0;` on every trigger. Consequences:
1. The original `decay` value from the kit TOML is destroyed on first trigger — the user can no longer revert to the static-ms behaviour without an engine rebuild.
2. A subsequent `SET_PARAM:slot:decay:X` (which writes `self.decay = X` and rebuilds modes) is correctly applied — but is immediately overwritten on the next trigger, because `decay_division` is still `Some`. The user adjusts the decay slider in the UI and sees no audible change once they re-trigger the voice.
**Repro**: Load `Cathedral_Forever` (Modal slots with `decay_division`). In the UI, drag the decay slider on any of those slots. Strike the pad — the audible decay is unchanged. Strike again — still unchanged. The UI value diverges silently from the audible behaviour.
**Root cause**: `rebuild_modes()` reads `self.decay` and the author chose to write the resolved value back rather than thread the resolved value through. There is no "decay_division overrides slider" UI signal.
**Suggested fix**: Either (a) compute a local `effective_decay_ms` inside `trigger()`, pass it to `rebuild_modes(effective_decay_ms)`, and don't touch `self.decay`; OR (b) when SET_PARAM:decay arrives for a slot with `decay_division` set, clear `decay_division` so the user's slider edit wins. The first is cleaner. Either way, the UI needs to know about `decay_division` so it can warn the user.

### 6. UI / backend handshake silently ignores all six new clock-aware fields — `src/commands.rs:92-128` (`kit_to_json`) + `src/commands.rs:384-407` (`SET_PARAM` match)
**Symptom**: `kit_to_json` does not emit `sub_hits`, `pattern`, `trigger_probability`, `ghost_probability`, `ghost_offset_ms`, `ghost_velocity_factor`, or `mode_list`. `SET_PARAM`'s param-match `_ => {}` catches any UI attempt to edit them. So the UI can neither see nor adjust any of the four flagship features the recent commits ship — they exist only as TOML-authored static data. (This is also why `ui/src/views/KitEditorView.tsx` and `ui/src/components/ModulationPanel.tsx` have no references to those fields.)
**Repro**: Open the Kit Editor, select a slot from `Ghost_Maker` (whose every slot has `ghost_probability`). The UI shows attack/decay/freq/etc. but no probability widget; the user cannot toggle ghosting off or tune the offset.
**Root cause**: The UI surface was not extended to cover the new schema fields, and neither was the JSON contract.
**Suggested fix**: Extend `kit_to_json` to emit the seven fields (already serializable). Extend `SET_PARAM` to handle them (note that `lfo1_division` / `lfo2_division` / `decay_division` need to send a `BeatDivision` variant name, not a float). Add ModulationPanel / KitEditorView UI for the per-slot ghost/probability/pattern/sub-hit editors. Best done incrementally — start with `ghost_probability` and `trigger_probability` since they're single floats.

## MEDIUM

### 7. ModulationPanel's LFO Hz slider is dead for clock-aware slots — `ui/src/components/ModulationPanel.tsx:34-60`
**Symptom**: The UI offers Hz sliders that send `SET_LFO:slot:1:freq`. Backend writes the freq into `lfo1_freq`. But `voice_from_sound` only calls `set_lfo(1, sound.lfo1_freq)` if `sound.lfo1_freq` is Some; this is "the static fallback". Then at trigger time, if `lfo1_division` is `Some`, it overrides the Hz with `div.to_hz(bpm)`. So on every clock-aware slot the user's slider edit is invisible from the next trigger onward.
**Repro**: Load `Phase_Mirror` (uses `lfo1_division`). Drag the LFO Rate slider. Re-trigger the slot. LFO speed is unchanged from before the drag.
**Suggested fix**: Show a "tempo-locked" indicator on the LFO panel when `lfo*_division` is set, and disable the slider (or change it to a BeatDivision dropdown). Same applies to decay when `decay_division` is set.

### 8. `kit_to_json` does not emit `pattern` / `sub_hits` / `mode_list`, so persistence of these fields is fine but UI display is impossible — `src/commands.rs:92-128`
**Status: RESOLVED in commit `97ab143` (`feat(commands): expose clock-aware effect fields to UI via KIT_TO_JSON + SET_PARAM`).** All 10 clock-aware / generative-trigger fields (`sub_hits`, `pattern`, `mode_list`, `trigger_probability`, `ghost_probability`, `ghost_offset_ms`, `ghost_velocity_factor`, `lfo1_division`, `lfo2_division`, `decay_division`) are emitted by `kit_to_json` and covered by per-field tests in `tests/commands_tests.rs`. The umbrella regression test `test_kit_to_json_includes_all_clock_aware_fields` asserts that every key is present on each slot, so a future refactor that drops one surfaces immediately.
**Original symptom**: Already listed as part of bug #6 but worth calling out separately: a kit with patterns loaded into the snapshot via TOML parses fine, fires on the audio thread, and gets serialised back on `SaveKit` (via the entire `DrumKit` clone) — so persistence is intact. The data ONLY disappears at the JSON-to-UI boundary. So this was a "UI silently amnesia" bug, not a "user loses data" bug.
**Repro**: Save a kit, restart server, kit reload — sub_hits / pattern survive. (Was: in the UI editor they were never visible to begin with.)
**Resolution**: Per-field emission added to `kit_to_json` (`src/commands.rs:100-151`), plus umbrella test.

### 9. `ghost_probability` rolls share the trigger-probability RNG draw, but the doc-comment math is wrong — `src/kit.rs:738-747`
**Symptom**: The big comment claims "if trigger is 0.5 and ghost is 0.4 then ~40% of the SURVIVING primaries ghost (since roll is in [0, 0.5) for survivors)." That's wrong. With trigger=0.5 and ghost=0.4, the SAME roll satisfies both `roll <= 0.5` (survives) and `roll < 0.4` (ghosts). Of the survivors (rolls in [0, 0.5]), the fraction with `roll < 0.4` is `0.4 / 0.5 = 80%`, not 40%. So the implemented behaviour is "80% of survivors ghost" while the comment says 40%.
**Repro**: Read the doc comment. Trace the math.
**Suggested fix**: Either (a) rewrite the doc-comment to say "in the boolean expression `roll < ghost_p`, if ghost_p <= trigger_p, the fraction of survivors that ghost is ghost_p / trigger_p; if ghost_p > trigger_p, all survivors ghost", or (b) decouple the two rolls (one per gate) — more independent, more deterministic-test-friendly, and matches the natural reading.

### 10. `samples_processed` is bumped BEFORE `drain_pending`, making `multiplier = 0.0` / `offset_ms = 0.0` fire one sample LATE — `src/kit.rs:801-816`
**Symptom**: `tick()` does `samples_processed += 1` first, then `drain_pending`. A pending entry queued with `samples_from_now = 0` has `fire_at_sample == samples_processed_at_queue_time`. On the next tick, `samples_processed` becomes that value + 1; `fire_at <= samples_processed` is true, fires. So a zero-offset pending fires at sample N+1, while the primary fired at sample N. The "fires within 1-2 samples" test (`pattern_tests::test_multiplier_zero`) documents this without flagging it.
**Repro**: `test_multiplier_zero` itself: it asserts `drain <= 2`, which is +1 sample of skew, not 0.
**Root cause**: Order-of-operations choice in `tick()`.
**Suggested fix**: Either drain BEFORE bumping the counter, OR queue with `samples_from_now = samples_from_now.saturating_sub(1)` to compensate. Either way fixing this is a tiny audio improvement (one-sample tighter flam onset). The current behaviour is also defensible (zero-offset effectively becomes "next sample") but it's worth documenting that "fire same-sample" is impossible by design.

### 11. Pending-trigger queue can overflow with 3+ active polymetric voices — `src/kit.rs:16`, `src/kit.rs:545-564`
**Symptom**: `PENDING_TRIGGER_CAPACITY = 128`. A single primary can queue up to `MAX_SUB_HITS_PER_PRIMARY (8) + MAX_PATTERN_STEPS_PER_PRIMARY (32) + 1 ghost = 41 entries`. Three rapid primaries on different slots = 123 entries; four exceeds capacity. `queue_pending` silently returns `false` after that, dropping ghosts / late pattern steps. This is more likely than it sounds for kits like `Polymeter_Madness` whose slots have 5-15 pattern steps each.
**Repro**: Build a kit with three slots each declaring 32 pattern steps and 8 sub-hits and 1 ghost. Trigger all three in quick succession. Some pattern steps on the third primary silently disappear.
**Root cause**: Capacity sized for "typical" rather than worst-case.
**Suggested fix**: Either (a) raise `PENDING_TRIGGER_CAPACITY` to e.g. 512 (16 slots × 32 pattern steps); (b) detect overflow and log a one-shot warning; (c) prioritise pattern entries over ghosts (drop ghosts first under pressure). Adding telemetry (`pending_overflows: AtomicU64`) would surface the problem first.

### 12. `cmd_consumer` and `event_consumer` Option wrappers stay non-None across the first SELECT_AUDIO swap — `src/commands.rs:642-647`
**Symptom**: `main.rs` takes the original consumers (line 232) leaving `Some` → `None`, then `SELECT_AUDIO` recreates fresh ring buffers (line 587-588) and passes the new consumers directly into `start_audio`. The original wrapped `Option`s are never re-populated. If a future caller (or test) inspects `event_consumer_wrapped` / `cmd_consumer_wrapped` expecting them to reflect the live ring, they're stale. The current code dodges this with `let _ = event_consumer;` to silence the lint, but the data is now structurally meaningless.
**Repro**: After SELECT_AUDIO, `event_consumer_wrapped.lock().await.is_none()` is still true; the producer has been swapped but the consumer handle is gone.
**Root cause**: The Arc<Mutex<Option<Consumer>>> abstraction is no longer "the live consumer" after SELECT_AUDIO; the consumer is owned by the leaked stream closure.
**Suggested fix**: Drop the `event_consumer_wrapped` / `cmd_consumer_wrapped` Arcs entirely after first audio start. They serve no purpose post-handshake.

### 13. `LOAD_KIT` from a non-existent file silently does nothing — `src/commands.rs:348-362`
**Symptom**: If `fs::read_to_string(...)` fails (file missing, permission denied), the `if let Ok(content)` falls through, nothing is broadcast, and the UI's "kit list" still shows the old kit selected. No error response is sent.
**Repro**: Pre-populate the UI's kit list, delete one of the TOML files on disk, click "Load" on that kit. Silent no-op.
**Root cause**: Missing error broadcast.
**Suggested fix**: Send a `KIT_ERROR:<name>:<err>` broadcast on either failure path.

### 14. Modal `tail_active` keeps `is_active()` true after env idle, so post-FX / mod-state broadcasts run forever on long-decay modes — `src/dsp/modal.rs:436`, `src/dsp/modal.rs:486-492`
**Symptom**: `tail_active = (sum * OUTPUT_TRIM).abs() > TAIL_ACTIVE_THRESHOLD` only flips false when the mode-bank's instantaneous sum drops below threshold. For a 4-bar `decay_division` (8 s at 120 BPM, 16 s at 60 BPM), if `dampening` is also low, the modal resonators ring for tens of seconds — `is_active()` keeps reporting true and the audio loop keeps calling the (now silent, but expensive) tick. Also: `set_value`/`get_values` broadcasts capture the still-ticking mod values, so the UI sees a slot "active" long after audible silence.
**Repro**: Load `Cathedral_Forever`, strike a pad. Watch the UI activity LED for that slot — stays lit for ~15 s at 120 BPM (because `decay_division = "TwoBars"` resets `self.decay` to 4 s, but the modal tail itself outlasts that).
**Root cause**: Tail threshold is a one-way detector; once env is idle the engine can't be excited again, so the sum naturally decays, but slowly for high-Q modes.
**Suggested fix**: Bound `tail_active` to (a) below threshold AND (b) env is `Idle` AND (c) a cumulative "samples since env idle" counter — e.g. forcibly clear `tail_active` after `2 * decay_sec` of silence has passed.

### 15. `kit.last_bpm` is captured ONLY on `trigger()`, so `tick()`'s `drain_pending` uses a stale tempo for cross-tempo retriggers — `src/kit.rs:702-707`, `src/kit.rs:813-815`
**Symptom**: When `MIDI_TRACK_PLAYING` writes a new BPM to `shared_state`, the audio thread reads it per-block and passes it to `kit.trigger(note, vel, bpm)`. But pending entries queued under the OLD BPM still fire — and `drain_pending(self.last_bpm)` uses the BPM AT TIME OF MOST RECENT TRIGGER. If the playback ramps BPM mid-decay (or the live-detected BPM drifts), pending fires at the wrong tempo. Sample offsets were calculated correctly at queue time, so the FIRE-TIME is right; what's wrong is the BPM passed to `voice.trigger(velocity, bpm)` for the now-firing entry — so the just-spawned voice's tempo-locked LFO/decay snap to a tempo that may no longer be active.
**Repro**: Hard to repro by hand; requires BPM change between primary fire and pattern-step drain. Look at `Pattern_Demo`'s slow patterns (Quarter at 80 BPM = 750 ms offset) with a live BPM sweep in between.
**Root cause**: `last_bpm` is a single scalar shared across all pending entries instead of being stamped per-entry.
**Suggested fix**: Stamp `bpm_at_queue` onto each `PendingTrigger` (it's already `Copy`, just 4 bytes added). Use that field in `drain_pending` rather than `self.last_bpm`. This also makes the queue self-contained against `last_bpm` mutation by background threads.

### 16. `swap_remove_back` order-sensitivity in `drain_pending` can re-check an already-checked entry — `src/kit.rs:575-590`
**Symptom**: `drain_pending` walks `self.pending` with `i`, calls `swap_remove_back(i)` when an entry fires (which moves the LAST entry into position `i`), and crucially does NOT advance `i`. This is correct iteration semantics. BUT: when the LAST entry being swapped INTO position `i` has itself just been added by `queue_pending` (e.g. a freshly-arrived sub-hit), it may be checked in this same drain loop. The doc-comment says "those must NOT re-fire in the same tick" — but `drain_pending` doesn't actually enforce this. If two pending entries' offsets are 0 and they trigger in a single tick, both fire. Currently the queue is read-only inside `drain_pending` (since `voice.trigger` doesn't go through `KitEngine::trigger`, so no new entries are pushed), so the doc-comment is over-cautious — but if any future change makes voice.trigger spawn more pending entries, the safety claim is wrong.
**Repro**: Today's code doesn't hit the path. But the safety claim in the comment is incorrect.
**Suggested fix**: Either (a) snapshot the queue length at drain start and stop iteration there, or (b) update the comment to say "today voice.trigger doesn't re-enter the queue, so the loop is safe; if that changes, revisit."

## LOW

### 17. `analyze_sound` uses a fixed 120 BPM, so the clip / silent banner doesn't reflect the actual playback tempo for clock-aware kits — `src/commands.rs:48-49`
**Symptom**: Documented in the comment ("any sane BPM works"). For a Modal voice whose `decay_division = "FourBars"`, the analysed decay at 120 BPM is 8 s; at 60 BPM it'd be 16 s. The analysis loop is also capped at `total_samples.min(1_000_000)` ≈ 21 s at 48 kHz — so a long-decay voice at slow BPM gets truncated and may report `silent = true` because the peak in the first 21 s isn't enough. False positives for the silent banner on slow-tempo, long-decay kits.
**Repro**: Load `Cathedral_Forever` at hypothetical 60 BPM. The first slot's FourBars decay is 16 s; analysis still completes but rms is artificially low. Doesn't trigger `silent` today (peak > 0.05) but could on a quieter mode.
**Suggested fix**: Pass the live BPM through `ANALYZE_SLOT` (it's available on `SharedState`) so analysis matches playback tempo.

### 18. `analyze_sound` clip threshold (RAIL = 0.999) doesn't account for the master soft-clip path — `src/commands.rs:56-78`, `src/audio.rs:77`
**Symptom**: Voice outputs are summed, multiplied by `0.7`, then run through `tanh()` before reaching speakers. A voice peaking at 1.0 will reach ~0.604 after `0.7 * tanh(0.7)`. The analysis bypasses both and warns "sustained_clip" if the voice internally rails for 100+ samples, even when those samples become inaudible distortion post soft-clip. False positives for clipping warnings on the UI dots.
**Suggested fix**: Either run the analysis through the same `soft_clip(x * 0.7)` chain, or document that the warning is "voice-internal clipping" not "audible clipping" and lower the threshold accordingly.

### 19. `MIDI_TRACK_STOPPED:` broadcast on manual stop has an empty payload — `src/commands.rs:758`
**Symptom**: `STOP_MIDI_PLAYBACK` broadcasts `"MIDI_TRACK_STOPPED:"` with no track name. The UI's parser splits on `MIDI_TRACK_STOPPED:` (line 158 in App.tsx) and treats anything starting with that prefix as a stop — so the empty payload is fine for the UI, but the message is asymmetric with `MIDI_TRACK_STOPPED:<name>` from natural end. Wire-format inconsistency.
**Suggested fix**: Have STOP_MIDI_PLAYBACK include the last-known playing-track name (read from `midi_playback_handle.lock()` before taking) or remove the colon: `MIDI_TRACK_STOPPED` (no payload).

### 20. `Cathedral_Bells` and `Modal_Demo` kits use `decay_division` etc. but are NOT in the `NEW_FEATURE_KITS` allowlist — `tests/schema_robustness_tests.rs:19-31`
**Symptom**: This allowlist is consulted by `test_kits_without_new_fields_still_load` to skip kits that use the new features. Let me verify... actually grep shows `Cathedral_Bells` and `Modal_Demo` DO NOT use any of `sub_hits`, `trigger_probability`, `ghost_probability`, `pattern`, `decay_division`, `lfo*_division`. So they correctly stay OUT of the allowlist. **Not a bug after verification.** Keeping the entry here to document the (non)issue.

### 21. `PENDING_TRIGGER_CAPACITY = 128` is undocumented at the `DrumSound` level — `src/kit.rs:11-24`
**Symptom**: The cap docs say "128 covers ~8 active slots each holding a full pattern (16 steps)" but the actual `MAX_PATTERN_STEPS_PER_PRIMARY` is 32. So the math is "~4 active slots × 32 steps", not "~8 × 16". User-facing TOML schema docs in `conductor/code_styleguides` or `kit.toml` don't mention this at all.
**Suggested fix**: Update the doc-comment to reflect the actual constant; warn in the TOML schema reference.

### 22. `samples_processed: u64` wrap claim — `src/kit.rs:478-481`
**Symptom**: Doc-comment says "~12 million years to wrap" at 48 kHz. Math: 2^64 / 48000 / 60 / 60 / 24 / 365 ≈ 12.18 million years. Correct, just verifying. **Not a bug**, leaving for completeness.

### 23. `Lfo::tick()` advances phase regardless of voice-active state — `src/dsp/modulation_engine.rs:19-23`
**Symptom**: LFO phase accumulates even when no voice is active. Free-running LFO is the standard interpretation, but it does mean `lfo1_division` re-triggered at a different BPM will have an arbitrary phase relative to the new trigger — no phase reset to 0. Visible if a user listens for envelope-locked LFO behaviour ("am I synced to this note").
**Suggested fix**: Likely intentional. Documenting it; not a fix.

### 24. `KitEngine::tick` clamps output to `[-1.0, 1.0]` AFTER per-slot post-FX — `src/kit.rs:824-826`
**Symptom**: The trailing `out.clamp(-1.0, 1.0)` is then *also* multiplied by `0.7` and `soft_clip`'d in audio.rs. The kit's clamp is therefore a hard limit, not soft. Two-stage limiting is harmless but adds an undocumented clip stage that can produce harmonics on extreme summed levels. Minor.
**Suggested fix**: Either remove the kit-level clamp (let `tanh` handle it) or document why both exist.

### 25. `Xorshift::next_f32` divides u32 by `u32::MAX as f32`, which is not exactly representable — `src/dsp/utils.rs:55-60`
**Symptom**: `u32::MAX = 4_294_967_295`; the nearest `f32` is `4_294_967_296.0`. So the divisor is slightly larger than the max state. Result: `next_f32()` ranges over `[0, 0.9999999...]` and never quite hits 1.0. Documented assumption in the probability code is that the roll CAN hit 1.0 (so `> 1.0` correctly never fires) — practically fine because the comparison would still return false at 0.99999... > 1.0. But the boundary is a touch fuzzier than the comments imply.
**Suggested fix**: None needed; documenting the subtlety.

## Investigated but not bugs

### `samples_processed += 1` ordering
The drain ordering is intentional and documented (kit.rs:801-808). The one-sample-late skew for offset=0 is the LOW-severity item #10.

### Concurrent kit swap dropping pending entries
`LOAD_KIT` replaces the entire `KitEngine` (commands.rs:354-356), which discards the `pending` queue. This is the correct semantics — old pending entries should NOT fire on the new kit's slots, and they don't.

### BPM atomic Relaxed ordering
`store_bpm` / `load_bpm` use `Ordering::Relaxed`. Correct for a single non-aliased atomic on x86/ARM/etc. No torn reads possible because u32 is naturally aligned and load/store are single instructions on every supported target.

### NaN/Inf in `store_bpm`
`store_bpm` checks `is_finite()` (state.rs:69) and ignores degenerate input. **Confirmed safe.**

### WS reconnect ANALYZE_SLOT
On reconnect (`socket.onopen` in App.tsx:71), the client re-sends `GET_KIT`. The `KIT:` response (lines 111-125) iterates the kit and issues `ANALYZE_SLOT` per populated slot. So reconnect DOES re-run analysis. **Confirmed working.**

### Trigger probability boundary semantics
With `trigger_probability = 1.0` and roll = 1.0, `roll > 1.0` is false → fires. With `trigger_probability = 0.0`, roll is always > 0 (Xorshift seeded non-zero never produces 0) → never fires. **Correct.**

### Test fixtures missing new Optional fields
`cargo check --tests` compiles cleanly — every `DrumSound { ... }` literal in tests/ has been updated. **Confirmed.**

### samples_processed u64 overflow
~12 million years at 48 kHz. Not a real concern.

### kit_to_json missing fields breaks persistence
Verified that persistence uses the full `DrumKit` clone (commands.rs:425-426, persistence.rs:18) which DOES serialise the missing fields. Round-trip works; ONLY the WS-to-UI display is broken.

## Tools / commands used

- `git log --oneline -25` to identify the freshly-touched feature surface.
- `git log --name-only --pretty=format:'== %h %s' -10` to find which files each commit touched.
- `git show <commit> -- <file>` (manually) to inspect specific diffs.
- `grep -rn` across `src/`, `tests/`, `ui/src/`, `presets/kits/` to trace symbols and verify allowlists.
- `cargo check --tests` to confirm all test fixtures compile against the current `DrumSound` shape.
- Targeted read of every file in `src/` (kit.rs, state.rs, audio.rs, commands.rs, main.rs, midi_player.rs, persistence.rs) plus every engine in `src/dsp/` (fm.rs, phys.rs, granular.rs, hybrid.rs, modal.rs, noise.rs, modulation_engine.rs, timing.rs, envelope.rs, utils.rs).
- Cross-reference with `ui/src/App.tsx`, `ui/src/views/KitEditorView.tsx`, `ui/src/components/ModulationPanel.tsx`.
- Inspected `presets/kits/{Ghost_Maker, Phase_Mirror, Polymeter_Madness, Stutter_Snare, Cathedral_Forever, Pattern_Demo}.toml` to confirm feature usage matches the NEW_FEATURE_KITS allowlist.
- Spot-checked test files: `pattern_tests.rs`, `sub_hits_tests.rs`, `probability_tests.rs`, `midi_player_tests.rs`, `schema_robustness_tests.rs`, `velocity_contract_tests.rs`, `clock_aware_tests.rs`.
