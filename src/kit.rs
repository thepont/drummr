use crate::dsp::fm::FmVoice;
use crate::dsp::modal::ExplicitMode;
use crate::dsp::noise::NoiseVoice;
use crate::dsp::postfx::PostFx;
use crate::dsp::timing::BeatDivision;
use crate::dsp::utils::Xorshift;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use std::collections::VecDeque;

/// Maximum number of pending triggers the queue can hold at any moment.
/// Pre-sized at construction so `tick()` never allocates on the audio
/// thread. 128 covers ~8 active slots each holding a full pattern (16
/// steps) plus a few stragglers — well outside any realistic kit's needs
/// at any single instant.
pub const PENDING_TRIGGER_CAPACITY: usize = 128;

/// Hard cap on sub-hits queued per primary trigger. Anything past this
/// is silently dropped; prevents a runaway TOML from flooding the audio
/// thread with thousands of deferred fires.
pub const MAX_SUB_HITS_PER_PRIMARY: usize = 8;

/// Hard cap on pattern steps queued per primary trigger.
pub const MAX_PATTERN_STEPS_PER_PRIMARY: usize = 32;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ParamSchema {
    pub name: String,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub unit: String,
}

use crate::dsp::modulation::ModSource;

pub enum Voice {
    Fm(FmVoice),
    Phys(crate::dsp::phys::PhysEngine),
    Granular(crate::dsp::granular::GranularEngine),
    Hybrid(crate::dsp::hybrid::HybridEngine),
    Modal(crate::dsp::modal::ModalEngine),
    Noise(NoiseVoice),
}

impl Voice {
    pub fn name(&self) -> &str {
        match self {
            Voice::Fm(_) => "FM",
            Voice::Phys(_) => "Physical Modeling",
            Voice::Granular(_) => "Granular",
            Voice::Hybrid(_) => "Hybrid",
            Voice::Modal(_) => "Modal",
            Voice::Noise(_) => "Noise",
        }
    }

    pub fn schema(&self) -> Vec<ParamSchema> {
        match self {
            Voice::Fm(v) => v.schema(),
            Voice::Phys(v) => v.schema(),
            Voice::Granular(v) => v.schema(),
            Voice::Hybrid(v) => v.schema(),
            Voice::Modal(v) => v.schema(),
            Voice::Noise(v) => v.schema(),
        }
    }

    pub fn set_param(&mut self, name: &str, value: f32) {
        match self {
            Voice::Fm(v) => v.set_param(name, value),
            Voice::Phys(v) => v.set_param(name, value),
            Voice::Granular(v) => v.set_param(name, value),
            Voice::Hybrid(v) => v.set_param(name, value),
            Voice::Modal(v) => v.set_param(name, value),
            Voice::Noise(v) => v.set_param(name, value),
        }
    }

    pub fn set_mod(&mut self, param: &str, source: ModSource, depth: f32) {
        match self {
            Voice::Fm(v) => v.set_mod(param, source, depth),
            Voice::Phys(v) => v.set_mod(param, source, depth),
            Voice::Granular(v) => v.set_mod(param, source, depth),
            Voice::Hybrid(v) => v.set_mod(param, source, depth),
            Voice::Modal(v) => v.set_mod(param, source, depth),
            Voice::Noise(_) => {}
        }
    }

    pub fn set_lfo(&mut self, index: usize, freq: f32) {
        match self {
            Voice::Fm(v) => v.set_lfo(index, freq),
            Voice::Phys(v) => v.mod_engine.set_lfo(index, freq),
            Voice::Granular(v) => v.mod_engine.set_lfo(index, freq),
            Voice::Hybrid(v) => v.mod_engine.set_lfo(index, freq),
            Voice::Modal(v) => v.mod_engine.set_lfo(index, freq),
            Voice::Noise(_) => {}
        }
    }

    pub fn get_mod_values(&self) -> [f32; 4] {
        match self {
            Voice::Fm(v) => v.get_mod_values(),
            Voice::Phys(v) => v.mod_engine.get_all_source_values(),
            Voice::Granular(v) => v.mod_engine.get_all_source_values(),
            Voice::Hybrid(v) => v.mod_engine.get_all_source_values(),
            Voice::Modal(v) => v.mod_engine.get_all_source_values(),
            Voice::Noise(_) => [0.0; 4],
        }
    }

    /// Trigger this voice with the given velocity and tempo (BPM). The BPM
    /// is consumed by tempo-locked features (`lfo*_division`, `decay_division`
    /// on the engine struct); engines that have no such features set will
    /// ignore it. Off-thread callers that don't have a live tempo handy
    /// (e.g. the ANALYZE_SLOT path in `commands.rs`) pass a sensible default
    /// like 120.0.
    pub fn trigger(&mut self, velocity: f32, bpm: f32) {
        match self {
            Voice::Fm(v) => v.trigger(velocity, bpm),
            Voice::Phys(v) => v.trigger(velocity, bpm),
            Voice::Granular(v) => v.trigger(velocity, bpm),
            Voice::Hybrid(v) => v.trigger(velocity, bpm),
            Voice::Modal(v) => v.trigger(velocity, bpm),
            Voice::Noise(v) => v.trigger(velocity, bpm),
        }
    }

    pub fn tick(&mut self) -> f32 {
        match self {
            Voice::Fm(v) => v.tick(),
            Voice::Phys(v) => v.tick(),
            Voice::Granular(v) => v.tick(),
            Voice::Hybrid(v) => v.tick(),
            Voice::Modal(v) => v.tick(),
            Voice::Noise(v) => v.tick(),
        }
    }

    pub fn is_active(&self) -> bool {
        match self {
            Voice::Fm(v) => v.is_active(),
            Voice::Phys(v) => v.is_active(),
            Voice::Granular(v) => v.is_active(),
            Voice::Hybrid(v) => v.is_active(),
            Voice::Modal(v) => v.is_active(),
            Voice::Noise(v) => v.is_active(),
        }
    }
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DrumKit {
    pub name: String,
    pub description: Option<String>,
    pub sounds: Vec<DrumSound>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DrumMapping {
    pub note: u8,
    pub slot: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModEntry {
    pub param: String,
    pub source: ModSource,
    pub depth: f32,
}

/// A single deferred re-trigger of the same slot, scheduled in real
/// milliseconds relative to the primary hit. Used to construct the
/// LinnDrum / TR-909 multi-tap clap (four noise bursts ~12 ms apart)
/// and similar flam / drag effects without writing a sequencer in
/// TOML. The offset is in milliseconds because clap multi-taps are
/// real-time phenomena, not musical-time ones — they don't stretch
/// with BPM. See `PatternStep` for the BPM-locked variant.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SubHit {
    /// Delay from the primary trigger, in milliseconds.
    pub offset_ms: f32,
    /// Multiplier on the primary trigger's velocity (0.0..=1.0+).
    /// Cap-style claps typically use a decay curve like 1.0, 0.85,
    /// 0.70, 0.55 to mimic the natural release of the original
    /// envelope.
    pub velocity_factor: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DrumSound {
    pub name: String,
    pub engine_type: Option<String>,
    pub freq: f32,
    pub mod_ratio: Option<f32>,
    pub mod_index: Option<f32>,
    pub noise_level: Option<f32>,
    pub brightness: Option<f32>,
    pub dampening: Option<f32>,
    pub density: Option<f32>,
    pub grain_size: Option<f32>,
    pub jitter: Option<f32>,
    pub noise_color: Option<f32>,
    pub metallic: Option<f32>,
    pub inharmonicity: Option<f32>,
    pub bits: Option<f32>,
    pub rate: Option<f32>,
    pub attack: f32,
    pub decay: f32,
    pub lfo1_freq: Option<f32>,
    pub lfo2_freq: Option<f32>,
    /// Tempo-locked LFO 1 division. When set, overrides `lfo1_freq` at trigger
    /// time using the live BPM (`dsp::timing::BeatDivision::to_hz`). Backwards
    /// compatible: kits without this field parse identically (Option = None).
    pub lfo1_division: Option<BeatDivision>,
    /// Tempo-locked LFO 2 division. See `lfo1_division`.
    pub lfo2_division: Option<BeatDivision>,
    /// Tempo-locked envelope decay. When set, overrides `decay` (ms) at
    /// trigger time using the live BPM. Lets a kit declare "decay over one
    /// bar" or "ring for 4 bars" without committing to a specific tempo.
    pub decay_division: Option<BeatDivision>,
    pub mods: Option<Vec<ModEntry>>,
    /// Optional explicit mode list for the modal engine. When present, the
    /// modal voice uses these exact `{freq, q, gain}` triples (up to 12) in
    /// place of the harmonic/Bessel interpolation. Ignored by all other
    /// engine types. Named `mode_list` to avoid TOML collision with the
    /// existing `mods` (modulation routes) field.
    pub mode_list: Option<Vec<ExplicitMode>>,
    /// Optional fixed-millisecond multi-taps. Each entry queues an
    /// additional retrigger of the same slot at `offset_ms` after the
    /// primary, scaled by `velocity_factor`. Used for the classic
    /// 4-tap clap envelope and analogous flam / drag effects.
    /// Capped at `MAX_SUB_HITS_PER_PRIMARY` (8) per primary; entries
    /// beyond the cap are silently dropped to keep TOML mistakes from
    /// flooding the audio thread.
    pub sub_hits: Option<Vec<SubHit>>,
}

/// Construct a single `Voice` from a `DrumSound`, applying engine-specific
/// parameter mapping, modulation routings, and LFO frequencies. This is the
/// per-slot factory shared between `KitEngine::from_config` (which builds a
/// 16-slot engine) and the analysis path (which builds one throwaway voice
/// off the audio thread to measure its envelope without making a sound).
///
/// Returns `None` if the engine type is not recognised. The default fall-through
/// for unknown / missing `engine_type` is FM, matching the prior behaviour of
/// `from_config`.
pub fn voice_from_sound(sound: &DrumSound, sample_rate: f32) -> Option<Voice> {
    let engine_type = sound.engine_type.as_deref().unwrap_or("fm");
    let mut voice: Voice = match engine_type {
        "phys" => {
            let mut v = crate::dsp::phys::PhysEngine::new(sample_rate);
            v.frequency.base_value = sound.freq;
            v.brightness.base_value = sound.brightness.unwrap_or(0.5);
            v.dampening.base_value = sound.dampening.unwrap_or(0.5);
            v.attack = sound.attack;
            v.decay = sound.decay;
            v.lfo1_division = sound.lfo1_division;
            v.lfo2_division = sound.lfo2_division;
            v.decay_division = sound.decay_division;
            Voice::Phys(v)
        }
        "granular" => {
            let mut v = crate::dsp::granular::GranularEngine::new(sample_rate);
            v.frequency.base_value = sound.freq;
            v.density.base_value = sound.density.unwrap_or(0.5);
            v.grain_size.base_value = sound.grain_size.unwrap_or(50.0);
            v.jitter.base_value = sound.jitter.unwrap_or(0.2);
            v.attack = sound.attack;
            v.decay = sound.decay;
            v.lfo1_division = sound.lfo1_division;
            v.lfo2_division = sound.lfo2_division;
            v.decay_division = sound.decay_division;
            Voice::Granular(v)
        }
        "hybrid" => {
            let mut v = crate::dsp::hybrid::HybridEngine::new(sample_rate);
            v.frequency.base_value = sound.freq;
            v.noise_color.base_value = sound.noise_color.unwrap_or(0.5);
            v.metallic.base_value = sound.metallic.unwrap_or(0.5);
            v.attack = sound.attack;
            v.decay = sound.decay;
            v.lfo1_division = sound.lfo1_division;
            v.lfo2_division = sound.lfo2_division;
            v.decay_division = sound.decay_division;
            Voice::Hybrid(v)
        }
        "modal" => {
            let mut v = crate::dsp::modal::ModalEngine::new(sample_rate);
            v.frequency.base_value = sound.freq;
            v.brightness.base_value = sound.brightness.unwrap_or(0.7);
            v.dampening.base_value = sound.dampening.unwrap_or(0.5);
            v.inharmonicity.base_value = sound.inharmonicity.unwrap_or(0.3);
            v.attack = sound.attack;
            v.decay = sound.decay;
            v.lfo1_division = sound.lfo1_division;
            v.lfo2_division = sound.lfo2_division;
            v.decay_division = sound.decay_division;
            // Install the explicit mode list (if any) AFTER the standard
            // params so the explicit path is what `rebuild_modes()` sees on
            // the next trigger. Cloning is one-shot at kit-build time and
            // not on the audio thread.
            v.set_explicit_modes(sound.mode_list.clone());
            Voice::Modal(v)
        }
        "noise" => {
            // `NoiseVoice` exposes only attack/decay via its AD envelope;
            // there is no separate noise-engine block in `from_config` today,
            // so the noise branch is reachable from `voice_from_sound` but
            // only exercised by the analysis path.
            let mut v = NoiseVoice::new(sample_rate);
            v.amp_env.set_params(sound.attack, sound.decay);
            v.decay_division = sound.decay_division;
            Voice::Noise(v)
        }
        _ => {
            let mut v = FmVoice::new(sample_rate);
            v.frequency.base_value = sound.freq;
            v.mod_ratio.base_value = sound.mod_ratio.unwrap_or(1.0);
            v.mod_index.base_value = sound.mod_index.unwrap_or(1.0);
            v.noise_level.base_value = sound.noise_level.unwrap_or(0.0);
            v.attack = sound.attack;
            v.decay = sound.decay;
            v.lfo1_division = sound.lfo1_division;
            v.lfo2_division = sound.lfo2_division;
            v.decay_division = sound.decay_division;
            v.pitch_bend = 150.0;
            v.pitch_env.set_params(0.001, 0.05);
            Voice::Fm(v)
        }
    };

    if let Some(mods) = &sound.mods {
        for m in mods {
            voice.set_mod(&m.param, m.source, m.depth);
        }
    }
    if let Some(f) = sound.lfo1_freq {
        voice.set_lfo(1, f);
    }
    if let Some(f) = sound.lfo2_freq {
        voice.set_lfo(2, f);
    }

    Some(voice)
}

/// Internal trigger event deferred to a future sample. Owned by
/// `KitEngine::pending` so the audio thread can fire flams, drags, pattern
/// steps, ghost notes, and any other time-shifted hit from a single
/// uniform mechanism. Never serialised — purely runtime state.
#[derive(Debug, Clone, Copy)]
pub struct PendingTrigger {
    /// Slot index (0..16) to retrigger when the timer expires.
    pub slot: usize,
    /// Velocity to pass into `Voice::trigger`. Already includes any
    /// `velocity_factor` scaling from the source feature.
    pub velocity: f32,
    /// Absolute sample index (relative to `KitEngine::samples_processed`)
    /// at which the trigger should fire. Stored absolute so the queue
    /// doesn't need to mutate every entry every tick.
    pub fire_at_sample: u64,
}

pub struct KitEngine {
    pub voices: [Option<Voice>; 16],
    /// Per-slot post-FX (bitcrusher + sample-rate reducer). Always present so
    /// the audio thread can run unconditionally; defaults to a pass-through.
    pub postfx: [PostFx; 16],
    pub sample_rate: f32,
    pub midi_map: [Option<usize>; 128], // note -> slot index
    /// Per-slot deferred-fire metadata used by trigger-time features
    /// (sub-hits, patterns, ghost notes). Indexed by slot. Owned by
    /// the engine so the audio thread doesn't reach back into the
    /// (synchronously-locked) `DrumSound` config to resolve each
    /// primary's recipe — those clones happen at kit-build time and
    /// only on the (non-realtime) configuration path.
    pub sub_hits: [Vec<SubHit>; 16],
    /// Time-deferred trigger queue. Drained in `tick()` against
    /// `samples_processed`; entries whose `fire_at_sample` has elapsed
    /// fire their slot and are removed. Pre-allocated to
    /// `PENDING_TRIGGER_CAPACITY` so the audio thread never reallocates.
    pub pending: VecDeque<PendingTrigger>,
    /// Monotonic sample counter incremented once per `tick()`. The
    /// reference clock for `PendingTrigger::fire_at_sample`. Wraps at u64
    /// max — irrelevant on any human-time scale (a 48 kHz stream would
    /// take ~12 million years to wrap).
    pub samples_processed: u64,
    /// Per-engine RNG used by trigger-time generative features
    /// (probability gate, ghost notes). Single shared stream so two
    /// voices' rolls are independent in expectation but reproducible
    /// when seeded.
    pub rng: Xorshift,
    /// Last BPM seen on `trigger()`. Cached so `tick()` (which has no
    /// BPM argument) can fire pending sub-hits and pattern steps at a
    /// sensible tempo. The audio thread already snapshots BPM per
    /// block before draining note events, so this is always recent.
    pub last_bpm: f32,
}

impl KitEngine {
    pub fn new(sample_rate: f32) -> Self {
        const NO_VOICE: Option<Voice> = None;
        let postfx = [
            PostFx::new(),
            PostFx::new(),
            PostFx::new(),
            PostFx::new(),
            PostFx::new(),
            PostFx::new(),
            PostFx::new(),
            PostFx::new(),
            PostFx::new(),
            PostFx::new(),
            PostFx::new(),
            PostFx::new(),
            PostFx::new(),
            PostFx::new(),
            PostFx::new(),
            PostFx::new(),
        ];
        const EMPTY_SUB_HITS: Vec<SubHit> = Vec::new();
        Self {
            voices: [NO_VOICE; 16],
            postfx,
            sample_rate,
            midi_map: [None; 128],
            sub_hits: [EMPTY_SUB_HITS; 16],
            pending: VecDeque::with_capacity(PENDING_TRIGGER_CAPACITY),
            samples_processed: 0,
            rng: Xorshift::new(0xC10C),
            last_bpm: 120.0,
        }
    }

    /// Reseed the per-engine RNG. Used by tests that need deterministic
    /// sequences out of the probability / ghost-note features. Audio-thread
    /// safe; just overwrites the existing state.
    pub fn set_rng_seed(&mut self, seed: u32) {
        self.rng = Xorshift::new(seed);
    }

    /// Push a deferred trigger onto the pending queue. Silently drops the
    /// request if the queue is already at `PENDING_TRIGGER_CAPACITY` so
    /// `tick()` is guaranteed never to allocate. Returns `true` if the
    /// trigger was queued, `false` if the queue was full.
    pub fn queue_pending(
        &mut self,
        slot: usize,
        velocity: f32,
        samples_from_now: u64,
    ) -> bool {
        if self.pending.len() >= PENDING_TRIGGER_CAPACITY {
            return false;
        }
        self.pending.push_back(PendingTrigger {
            slot,
            velocity,
            fire_at_sample: self.samples_processed.wrapping_add(samples_from_now),
        });
        true
    }

    /// Drain any pending triggers whose `fire_at_sample` has elapsed,
    /// firing each one against its slot's voice. Called once per
    /// `tick()` before the audio sum.
    fn drain_pending(&mut self, bpm: f32) {
        // Find and fire all expired entries. We swap-remove against a
        // small temporary because firing may itself queue more deferred
        // triggers (sub-hits on the spawned voice) — those must NOT
        // re-fire in the same tick. Capacity is bounded; the temporary
        // is short-lived.
        let mut i = 0;
        while i < self.pending.len() {
            if self.pending[i].fire_at_sample <= self.samples_processed {
                let entry = self.pending.swap_remove_back(i).unwrap();
                if entry.slot < 16 {
                    if let Some(voice) = &mut self.voices[entry.slot] {
                        self.postfx[entry.slot].reset();
                        voice.trigger(entry.velocity, bpm);
                    }
                }
                // Don't advance `i` — swap_remove_back put a different
                // element in this slot. Re-check it.
            } else {
                i += 1;
            }
        }
    }

    pub fn from_config(config: DrumKit, sample_rate: f32, mappings: Vec<DrumMapping>) -> Self {
        let mut engine = Self::new(sample_rate);

        for (idx, sound) in config.sounds.into_iter().enumerate() {
            if idx >= 16 {
                break;
            }
            if let Some(voice) = voice_from_sound(&sound, sample_rate) {
                engine.voices[idx] = Some(voice);
                engine.postfx[idx].set_bits(sound.bits.unwrap_or(16.0));
                engine.postfx[idx].set_rate(sound.rate.unwrap_or(1.0));

                // Copy sub-hits into the engine. Cap defensively at
                // MAX_SUB_HITS_PER_PRIMARY so the per-trigger queueing
                // path never has to re-check.
                if let Some(subs) = sound.sub_hits {
                    let mut bounded = subs;
                    if bounded.len() > MAX_SUB_HITS_PER_PRIMARY {
                        bounded.truncate(MAX_SUB_HITS_PER_PRIMARY);
                    }
                    engine.sub_hits[idx] = bounded;
                }
            }
        }

        engine.set_mapping(&mappings);

        engine
    }

    /// Build the midi_map array for a given set of mappings, falling back to
    /// `36 + slot` for any active slot that doesn't have an explicit entry.
    /// Pure function so it can be shared by `from_config` and `set_mapping`.
    fn build_midi_map(&self, mappings: &[DrumMapping]) -> [Option<usize>; 128] {
        let mut map: [Option<usize>; 128] = [None; 128];

        for mapping in mappings {
            if mapping.note < 128 && mapping.slot < 16 {
                map[mapping.note as usize] = Some(mapping.slot);
            }
        }

        // Ensure every active slot has AT LEAST a default mapping (36 + slot) if not already mapped
        for idx in 0..16 {
            if self.voices[idx].is_some() {
                if !map.iter().any(|&s| s == Some(idx)) {
                    let default_note = 36 + idx as u8;
                    if default_note < 128 && map[default_note as usize].is_none() {
                        map[default_note as usize] = Some(idx);
                    }
                }
            }
        }

        map
    }

    /// Update the midi note -> slot map in place without touching any voice
    /// state. Used by UPDATE_MAPPING / SAVE_MAPPING so that re-pad-assigning
    /// during playback doesn't drop envelopes, grain buffers, or delay lines.
    pub fn set_mapping(&mut self, mappings: &[DrumMapping]) {
        self.midi_map = self.build_midi_map(mappings);
    }

    pub fn set_param(&mut self, slot: usize, param: &str, value: f32) {
        if slot < 16 {
            if let Some(voice) = &mut self.voices[slot] {
                voice.set_param(param, value);
            }
        }
    }

    /// Adjust the per-slot post-FX (bitcrusher / sample-rate reducer).
    /// `param` is one of "bits", "rate".
    pub fn set_postfx(&mut self, slot: usize, param: &str, value: f32) {
        if slot < 16 {
            self.postfx[slot].set_param(param, value);
        }
    }

    pub fn trigger(&mut self, note: u8, velocity: f32, bpm: f32) {
        // Cache BPM so `tick()` (no BPM argument) can fire any pending
        // sub-hits or pattern steps at the same tempo we saw on the
        // primary. Cached even if the note didn't resolve to a slot —
        // pending entries already in flight still need a tempo.
        self.last_bpm = bpm;
        if note >= 128 {
            return;
        }
        let Some(slot) = self.midi_map[note as usize] else {
            return;
        };
        if slot >= 16 {
            return;
        }
        // Primary hit.
        if let Some(voice) = &mut self.voices[slot] {
            self.postfx[slot].reset();
            voice.trigger(velocity, bpm);
        } else {
            return;
        }

        // Sub-hits: queue each entry as a pending fire at its
        // millisecond offset. The MAX_SUB_HITS cap is already applied
        // at kit-build time (in `from_config`), so this loop is
        // bounded; we also re-cap here so a runtime mutation can't
        // bypass the audio-thread guarantee. Index-based iteration
        // sidesteps the borrow conflict with `queue_pending`.
        let sub_count = self.sub_hits[slot].len().min(MAX_SUB_HITS_PER_PRIMARY);
        if sub_count > 0 {
            let sr = self.sample_rate;
            for i in 0..sub_count {
                let sub = self.sub_hits[slot][i].clone();
                let samples_offset = (sub.offset_ms.max(0.0) * sr / 1000.0) as u64;
                let sub_vel = (velocity * sub.velocity_factor).clamp(0.0, 1.0);
                self.queue_pending(slot, sub_vel, samples_offset);
            }
        }
    }

    pub fn get_schema(&self, slot: usize) -> Option<Vec<ParamSchema>> {
        if slot < 16 {
            if let Some(voice) = &self.voices[slot] {
                return Some(voice.schema());
            }
        }
        None
    }

    pub fn tick(&mut self) -> f32 {
        // Bump the monotonic counter first so a "fire 100 samples from
        // now" entry queued at sample N actually fires on the 100th
        // subsequent tick (i.e. when `samples_processed` reaches N+100).
        // The counter is the index of the sample currently being
        // computed, not the count of samples already emitted.
        self.samples_processed = self.samples_processed.wrapping_add(1);

        // Drain any pending triggers whose absolute fire time has
        // arrived. Done BEFORE the audio sum so a freshly-fired voice
        // contributes to this same output sample (no perceived
        // one-sample latency vs the primary).
        if !self.pending.is_empty() {
            let bpm = self.last_bpm;
            self.drain_pending(bpm);
        }

        let mut out = 0.0;
        for (i, voice_opt) in self.voices.iter_mut().enumerate() {
            if let Some(voice) = voice_opt {
                let raw = if voice.is_active() { voice.tick() } else { 0.0 };
                out += self.postfx[i].process(raw);
            }
        }
        out.clamp(-1.0, 1.0)
    }
}

#[cfg(test)]
mod kit_engine_pending_tests {
    use super::*;

    const SR: f32 = 48000.0;

    /// Build a minimal kit with a single FM voice in slot 0 so we can
    /// observe trigger firings via voice activity.
    fn one_slot_kit() -> KitEngine {
        let mut engine = KitEngine::new(SR);
        let mut fm = FmVoice::new(SR);
        fm.decay = 50.0;
        engine.voices[0] = Some(Voice::Fm(fm));
        engine
    }

    #[test]
    fn test_pending_queue_starts_empty_and_counter_zero() {
        let engine = KitEngine::new(SR);
        assert_eq!(engine.pending.len(), 0);
        assert_eq!(engine.samples_processed, 0);
    }

    #[test]
    fn test_pending_trigger_fires_at_correct_sample() {
        let mut engine = one_slot_kit();
        engine.last_bpm = 120.0;
        // Queue a trigger 100 samples into the future.
        let queued = engine.queue_pending(0, 1.0, 100);
        assert!(queued, "expected queue to accept entry");
        assert_eq!(engine.pending.len(), 1);

        // Tick 99 samples — should not fire.
        for _ in 0..99 {
            engine.tick();
        }
        assert_eq!(engine.pending.len(), 1, "should not have fired yet at 99 samples");

        // Tick the 100th — the trigger fires.
        engine.tick();
        assert_eq!(engine.pending.len(), 0, "trigger should have fired");

        // The voice should now be active.
        let active = match &engine.voices[0] {
            Some(Voice::Fm(v)) => v.is_active(),
            _ => false,
        };
        assert!(active, "voice should be active after pending trigger fired");
    }

    #[test]
    fn test_pending_queue_capacity_cap() {
        let mut engine = KitEngine::new(SR);
        // Fill to capacity.
        for _ in 0..PENDING_TRIGGER_CAPACITY {
            assert!(engine.queue_pending(0, 1.0, 1000));
        }
        // One more should be rejected (no allocation, returns false).
        assert!(!engine.queue_pending(0, 1.0, 1000));
        assert_eq!(engine.pending.len(), PENDING_TRIGGER_CAPACITY);
    }

    #[test]
    fn test_pending_does_not_fire_when_slot_empty() {
        let mut engine = KitEngine::new(SR);
        // No voice in slot 0; queue should still pop cleanly without panic.
        engine.queue_pending(0, 1.0, 5);
        for _ in 0..10 {
            engine.tick();
        }
        assert_eq!(engine.pending.len(), 0);
    }
}
