use crate::dsp::fm::FmVoice;
use crate::dsp::modal::ExplicitMode;
use crate::dsp::noise::NoiseVoice;
use crate::dsp::postfx::PostFx;
use crate::dsp::timing::BeatDivision;
use crate::dsp::utils::Xorshift;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use std::collections::VecDeque;
use std::sync::Once;
use std::sync::atomic::{AtomicU64, Ordering};

/// Maximum number of pending triggers the queue can hold at any moment.
/// Pre-sized at construction so `tick()` never allocates on the audio
/// thread. Sized for the theoretical worst case: 16 active slots ×
/// `MAX_PATTERN_STEPS_PER_PRIMARY` (32) pattern steps = 512. A single
/// primary can also queue up to `MAX_SUB_HITS_PER_PRIMARY` (8) sub-hits
/// and 1 ghost on top of its pattern steps, but those add a constant
/// per slot rather than scaling with `MAX_PATTERN_STEPS_PER_PRIMARY`,
/// so 512 still covers any combination short of "every slot maxed out
/// simultaneously, mid-decay." Overflow is observed via
/// `KitEngine::pending_overflows` and a one-shot `eprintln!` on first
/// occurrence (see `queue_pending`).
pub const PENDING_TRIGGER_CAPACITY: usize = 512;

/// Hard cap on sub-hits queued per primary trigger. Anything past this
/// is silently dropped; prevents a runaway TOML from flooding the audio
/// thread with thousands of deferred fires.
pub const MAX_SUB_HITS_PER_PRIMARY: usize = 8;

/// Hard cap on pattern steps queued per primary trigger.
pub const MAX_PATTERN_STEPS_PER_PRIMARY: usize = 32;

/// Guards the per-session "pending queue overflowed" warning so we only
/// print it once, on the audio thread, the first time a `queue_pending`
/// call gets dropped. Per-engine telemetry still ticks every overflow
/// via `KitEngine::pending_overflows`, but a single stderr line is
/// enough for an operator to notice.
static PENDING_OVERFLOW_WARN: Once = Once::new();

/// Wire contract used by the `SCHEMA:<slot>|<json>` WebSocket broadcast to
/// describe one engine parameter to the UI. Each `Voice::schema()` returns
/// a `Vec<ParamSchema>` listing the engine's modulatable params so the UI
/// can render sliders without knowing engine internals.
///
/// `unit` is a free-form display hint and the UI keys layout off it.
/// Conventions in use today:
/// - `"Hz"` — fundamental frequency or any rate-in-cycles-per-second.
/// - `"ms"` — durations (`attack`, `decay`, `grain_size`).
/// - `"ratio"` — pure dimensionless ratios (`mod_ratio`).
/// - `"index"` — FM modulation index (no real-world unit, just a depth knob).
/// - `"level"` — normalised 0..1 amplitudes (`noise_level`).
/// - `""` (empty) — engines that have no canonical unit for the param
///   (`brightness`, `dampening`, `inharmonicity`, `density`, `jitter`,
///   `noise_color`, `metallic`).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ParamSchema {
    pub name: String,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub unit: String,
}

use crate::dsp::modulation::ModSource;

/// Audio-thread voice dispatcher. One variant per synthesis engine; the
/// six engines share the same trigger / tick / schema / mod surface
/// through the match arms in `impl Voice`.
///
/// `Voice` is the audio-thread-facing runtime type. The serialisable
/// per-slot config lives in `DrumSound` (TOML); `voice_from_sound` is
/// the factory that maps a `DrumSound` to one of these variants. Each
/// variant owns its engine state directly (no `Box<dyn>`), so dispatch
/// is a tagged-union match — six near-identical arms per method.
///
/// Adding a new engine means: implement the engine struct with the same
/// surface (`schema`, `set_param`, `set_mod`, `set_lfo`, `trigger`, `tick`,
/// `is_active`), add a new variant here, and wire the match arms in this
/// file plus the `voice_from_sound` factory. See `CLAUDE.md` for the
/// full checklist.
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

    /// Update one of the tempo-locked division fields on this voice. `param`
    /// is one of "lfo1_division", "lfo2_division", "decay_division". A
    /// `None` value clears the division, returning the slot to static
    /// Hz / ms behaviour. Unknown params (and engines that don't carry that
    /// particular field, like Noise on the LFO divisions) are silently
    /// ignored.
    pub fn set_division(&mut self, param: &str, division: Option<crate::dsp::timing::BeatDivision>) {
        match self {
            Voice::Fm(v) => match param {
                "lfo1_division" => v.lfo1_division = division,
                "lfo2_division" => v.lfo2_division = division,
                "decay_division" => v.decay_division = division,
                _ => {}
            },
            Voice::Phys(v) => match param {
                "lfo1_division" => v.lfo1_division = division,
                "lfo2_division" => v.lfo2_division = division,
                "decay_division" => v.decay_division = division,
                _ => {}
            },
            Voice::Granular(v) => match param {
                "lfo1_division" => v.lfo1_division = division,
                "lfo2_division" => v.lfo2_division = division,
                "decay_division" => v.decay_division = division,
                _ => {}
            },
            Voice::Hybrid(v) => match param {
                "lfo1_division" => v.lfo1_division = division,
                "lfo2_division" => v.lfo2_division = division,
                "decay_division" => v.decay_division = division,
                _ => {}
            },
            Voice::Modal(v) => match param {
                "lfo1_division" => v.lfo1_division = division,
                "lfo2_division" => v.lfo2_division = division,
                "decay_division" => v.decay_division = division,
                _ => {}
            },
            Voice::Noise(v) => {
                if param == "decay_division" {
                    v.decay_division = division;
                }
            }
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

/// On-disk schema for a kit. One `DrumKit` per `kit.toml` (the live working
/// kit) and one per file under `presets/kits/*.toml` (named presets). The
/// `sounds` array is positional: index 0..16 maps to slot 0..16 in the
/// audio engine. Names are display-only — slots are addressed by index.
#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DrumKit {
    pub name: String,
    pub description: Option<String>,
    pub sounds: Vec<DrumSound>,
}

/// MIDI note number → slot index mapping for one slot. Persisted to
/// `mapping.toml`. Resolution is "first match wins" via `KitEngine::midi_map`
/// (a 128-entry lookup table); unmapped notes silently no-op.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DrumMapping {
    pub note: u8,
    pub slot: usize,
}

/// One modulation route on a `DrumSound`. Persisted in TOML as part of the
/// per-slot `mods` array. Routes are unique per `(param, source)` pair —
/// adding the same combination again overwrites the depth (see the
/// `SET_MOD:` handler in `commands.rs`). `depth == 0.0` or
/// `source == ModSource::None` entries are pruned on write.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModEntry {
    pub param: String,
    pub source: ModSource,
    pub depth: f32,
}

/// Default multiplier for `PatternStep::multiplier`. `serde(default)`
/// uses this so older patterns without an explicit multiplier still
/// parse as "exactly one division".
fn default_pattern_multiplier() -> f32 {
    1.0
}

/// A single step in a per-slot rhythm pattern. Steps are resolved to
/// sample offsets at trigger time against the live BPM, so a pattern
/// declared in beat divisions adapts when tempo changes. The `multiplier`
/// lets a single declaration cover repeats — e.g. division=Sixteenth +
/// multiplier=3.0 fires three sixteenths after the primary, without
/// requiring a separate enum variant.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PatternStep {
    /// Beat-division offset from the primary trigger.
    pub division: BeatDivision,
    /// Velocity multiplier applied to the primary's velocity.
    pub velocity_factor: f32,
    /// Multiplier on the division offset. Defaults to 1.0. Lets a
    /// step say "two sixteenths" with division=Sixteenth + multiplier=2.0
    /// rather than needing a separate Eighth step.
    #[serde(default = "default_pattern_multiplier")]
    pub multiplier: f32,
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

/// On-disk schema for one slot. A single struct serialises every engine
/// type via per-engine `Option<f32>` parameter fields, so the TOML format
/// stays stable across engine changes. `engine_type` (an opaque key like
/// `"fm"`, `"phys"`, `"granular"`, `"hybrid"`, `"modal"`, `"noise"`)
/// selects which Option fields are read by `voice_from_sound`. Fields
/// that aren't relevant to the chosen engine are simply ignored — they
/// don't have to be `None`.
///
/// `attack` and `decay` are MILLISECONDS (the convention across the UI,
/// TOML, and `set_param`). Engines convert to seconds internally when
/// they hand the values to `AdEnvelope::set_params`.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct DrumSound {
    /// Display name shown in the UI. Slots are addressed by index, not name.
    pub name: String,
    /// Engine selector: `"fm"`, `"phys"`, `"granular"`, `"hybrid"`, `"modal"`,
    /// or `"noise"`. Defaults to `"fm"` when missing.
    pub engine_type: Option<String>,
    /// Fundamental frequency in Hz. Range varies per engine (typically
    /// 20..12000 for FM/Phys/Granular/Hybrid, 20..4000 for Modal).
    pub freq: f32,
    /// FM only. Ratio between modulator and carrier frequency (0..10).
    /// Integer ratios produce harmonic spectra; non-integer ratios
    /// produce inharmonic / bell-like timbres.
    pub mod_ratio: Option<f32>,
    /// FM only. Depth of frequency modulation (0..50). Higher = more
    /// sidebands, brighter / more complex.
    pub mod_index: Option<f32>,
    /// FM only. Noise burst mixed with the FM output (0..1). Drives
    /// the click / sizzle layer (snare crack, hat shimmer).
    pub noise_level: Option<f32>,
    /// Phys / Modal. How much high-frequency content is emphasised (0..1).
    /// 0 = dark / fundamental only; 1 = all overtones present.
    pub brightness: Option<f32>,
    /// Phys / Modal. How quickly resonance dies away (0..1).
    /// 0 = long ring; 1 = short, muted hit.
    pub dampening: Option<f32>,
    /// Granular only. Overlap of grains (0..1). Low = sparse; high = dense cloud.
    pub density: Option<f32>,
    /// Granular only. Length of each grain in ms (1..200). Short = clicky;
    /// long = smooth / textural.
    pub grain_size: Option<f32>,
    /// Granular only. Random timing variation between grains (0..1).
    /// 0 = perfectly periodic; 1 = chaotic.
    pub jitter: Option<f32>,
    /// Hybrid only. Filter character for the noise component (0..1).
    /// 0 = dark / dull; 1 = bright / sharp.
    pub noise_color: Option<f32>,
    /// Hybrid only. Mix between pitched oscillator (0) and filtered
    /// noise (1) inharmonic partials. Higher = more metallic / less tonal.
    pub metallic: Option<f32>,
    /// Modal only. 0 = harmonic series (musical xylophone bars);
    /// 1 = Bessel-zero ratios (circular drum membrane). In between
    /// gives bell-like character.
    pub inharmonicity: Option<f32>,
    /// PostFx: bit depth (1..16). 16 = clean, lower = digital crunch
    /// (SP-1200 / LinnDrum character). Applied per slot after voice mix.
    pub bits: Option<f32>,
    /// PostFx: sample-rate divisor (1..32). 1 = clean, higher = aliasing
    /// distortion. Applied per slot after voice mix.
    pub rate: Option<f32>,
    /// PostFx: transient shaper attack adjustment ([-1.0, 1.0]).
    /// 0.0 is clean, positive is snappier, negative is softer.
    pub attack_shaper: Option<f32>,
    /// PostFx: transient shaper sustain adjustment ([-1.0, 1.0]).
    /// 0.0 is clean, positive is longer/louder decay, negative is shorter/gated.
    pub sustain_shaper: Option<f32>,
    /// Envelope attack time in MILLISECONDS. Time from trigger to peak.
    pub attack: f32,
    /// Envelope decay time in MILLISECONDS. Time from peak to silence.
    /// When `decay_division` is set, this value is ignored in favour
    /// of the tempo-locked length.
    pub decay: f32,
    /// LFO 1 rate in Hz. Used when `lfo1_division` is unset.
    pub lfo1_freq: Option<f32>,
    /// LFO 2 rate in Hz. Used when `lfo2_division` is unset.
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
    /// Stereo position in `[-1, 1]`. -1 is hard left, 1 is hard right,
    /// 0 is center. Defaults to 0.0 when missing.
    pub pan: Option<f32>,
    /// Per-voice gain multiplier. 1.0 is unity; 0.0 is silent.
    pub level: Option<f32>,
    /// Per-voice saturation/overdrive. 0.0 is clean, higher is grittier.
    pub drive: Option<f32>,
    /// Per-voice mute state. True if silenced and triggers ignored.
    pub mute: Option<bool>,
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
    /// Optional per-slot rhythm pattern. Each step queues a deferred
    /// retrigger at `division * multiplier` beats after the primary,
    /// resolved against the live BPM at trigger time. Coexists with
    /// `sub_hits`: a slot can declare BOTH a millisecond clap envelope
    /// AND a tempo-locked pattern; both populate the same pending
    /// queue. Capped at `MAX_PATTERN_STEPS_PER_PRIMARY` (32) per
    /// primary.
    pub pattern: Option<Vec<PatternStep>>,
    /// Optional probability that a primary trigger actually fires.
    /// 1.0 (or unset) = always fires; 0.5 = ~half the hits drop; 0.0
    /// = no hit ever fires. When the gate drops, sub-hits and
    /// pattern steps drop with the primary (the whole hit is voided).
    /// Cures the machine-gun-roll problem on long sequences.
    pub trigger_probability: Option<f32>,
    /// Optional probability of spawning a ghost note alongside an
    /// actually-fired primary. 0.0 (or unset) = never; 0.4 = ~40%
    /// of fired primaries also schedule a soft echo at
    /// `ghost_offset_ms` with velocity `primary_velocity *
    /// ghost_velocity_factor`. Independent of `trigger_probability`
    /// — a dropped primary cannot produce a ghost.
    pub ghost_probability: Option<f32>,
    /// Milliseconds after the primary when the ghost note fires.
    /// Defaults to 60 ms (a comfortable flam-into-ghost spacing) when
    /// unset. Identical to a single `sub_hits` entry except gated by
    /// a probability roll rather than firing every time.
    pub ghost_offset_ms: Option<f32>,
    /// Velocity multiplier for the ghost note. Defaults to 0.3 when
    /// unset (the canonical "soft echo" level).
    pub ghost_velocity_factor: Option<f32>,
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
            // `sound.attack` / `sound.decay` are MILLISECONDS (the convention
            // across every engine + the UI). `AdEnvelope::set_params` takes
            // SECONDS, so divide by 1000 here. Without this conversion a
            // sound declaring `decay = 100.0` ran for 100 SECONDS.
            v.amp_env.set_params(sound.attack / 1000.0, sound.decay / 1000.0);
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

/// Resolved-at-build-time generative trigger settings for one slot.
/// Defaults map to the pre-feature behaviour (probability=1, no ghost)
/// so a kit that doesn't set any of the new DrumSound fields is
/// bit-for-bit identical to the original code path.
#[derive(Debug, Clone, Copy)]
pub struct GenerativeSettings {
    /// Probability in `[0, 1]` that a primary trigger fires at all.
    pub trigger_probability: f32,
    /// Probability in `[0, 1]` that a fired primary also spawns a
    /// ghost note. Independent of `trigger_probability` — a dropped
    /// primary never produces a ghost.
    pub ghost_probability: f32,
    /// Delay from the primary to the ghost, in milliseconds.
    pub ghost_offset_ms: f32,
    /// Velocity multiplier for the ghost note.
    pub ghost_velocity_factor: f32,
}

impl Default for GenerativeSettings {
    fn default() -> Self {
        Self {
            trigger_probability: 1.0,
            ghost_probability: 0.0,
            ghost_offset_ms: 60.0,
            ghost_velocity_factor: 0.3,
        }
    }
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
    /// BPM active at the time this hit was queued. Ensures that
    /// tempo-locked features on the deferred hit (ghosts, pattern
    /// steps) resolve against the correct tempo even if `last_bpm`
    /// changes before fire-time.
    pub bpm_at_queue: f32,
}

/// Audio-thread mutable kit state. Owns the 16 voice slots, their PostFx
/// chains, the MIDI note → slot lookup, deferred-trigger queue (sub-hits /
/// pattern steps / ghost notes), and the generative-trigger RNG. Built
/// from a `DrumKit` via `from_config`; mutated in place from the audio
/// thread for live-fire activity and from the WS dispatcher (via
/// `AudioCommand`s through the rtrb ring) for parameter edits.
///
/// Most fields are `pub` to keep the audio callback in `audio.rs` and the
/// integration tests in `tests/` zero-cost — but external mutation of
/// `pending`, `samples_processed`, `last_bpm`, and `rng` is undefined
/// behaviour from the engine's point of view. See `docs/code_smells.md`
/// for the encapsulation backlog (entries 71-72).
pub struct KitEngine {
    pub voices: [Option<Voice>; 16],
    /// Stereo position per slot in `[-1, 1]`.
    pub pans: [f32; 16],
    /// Per-voice gain in `[0, 2]`.
    pub levels: [f32; 16],
    /// Per-voice saturation in `[0, 1]`.
    pub drives: [f32; 16],
    /// Per-slot post-FX (bitcrusher + sample-rate reducer). Always present so
    /// the audio thread can run unconditionally; defaults to a pass-through.
    pub postfx: [PostFx; 16],
    /// Per-slot mute state. Checked at trigger and summing time.
    pub mutes: [bool; 16],
    pub sample_rate: f32,
    pub midi_map: [Option<usize>; 128], // note -> slot index
    /// Per-slot deferred-fire metadata used by trigger-time features
    /// (sub-hits, patterns, ghost notes). Indexed by slot. Owned by
    /// the engine so the audio thread doesn't reach back into the
    /// (synchronously-locked) `DrumSound` config to resolve each
    /// primary's recipe — those clones happen at kit-build time and
    /// only on the (non-realtime) configuration path.
    pub sub_hits: [Vec<SubHit>; 16],
    /// Per-slot rhythm pattern. See `DrumSound::pattern`.
    pub pattern: [Vec<PatternStep>; 16],
    /// Per-slot generative-trigger settings. Resolved into the
    /// audio-thread-friendly `GenerativeSettings` form at kit-build
    /// time so the trigger path doesn't have to unwrap Options on
    /// every hit.
    pub generative: [GenerativeSettings; 16],
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
    /// Monotonic counter of how many times `queue_pending` has had to
    /// drop a deferred-fire request because the queue was already at
    /// `PENDING_TRIGGER_CAPACITY`. Bumped on the audio thread; readable
    /// off-thread for telemetry (tests, debug dashboards). A non-zero
    /// value indicates either a runaway TOML (too many simultaneous
    /// polymetric voices) or that the cap needs lifting.
    pub pending_overflows: AtomicU64,
}

impl KitEngine {
    pub fn new(sample_rate: f32) -> Self {
        const NO_VOICE: Option<Voice> = None;
        let mut postfx = [
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
        for fx in &mut postfx {
            fx.update_coefficients(sample_rate);
        }
        const EMPTY_SUB_HITS: Vec<SubHit> = Vec::new();
        const EMPTY_PATTERN: Vec<PatternStep> = Vec::new();
        const DEFAULT_GEN: GenerativeSettings = GenerativeSettings {
            trigger_probability: 1.0,
            ghost_probability: 0.0,
            ghost_offset_ms: 60.0,
            ghost_velocity_factor: 0.3,
        };
        Self {
            voices: [NO_VOICE; 16],
            pans: [0.0; 16],
            levels: [1.0; 16],
            drives: [0.0; 16],
            mutes: [false; 16],
            postfx,
            sample_rate,
            midi_map: [None; 128],
            sub_hits: [EMPTY_SUB_HITS; 16],
            pattern: [EMPTY_PATTERN; 16],
            generative: [DEFAULT_GEN; 16],
            pending: VecDeque::with_capacity(PENDING_TRIGGER_CAPACITY),
            samples_processed: 0,
            rng: Xorshift::new(0xC10C),
            last_bpm: 120.0,
            pending_overflows: AtomicU64::new(0),
        }
    }

    /// Reseed the per-engine RNG. Used by tests that need deterministic
    /// sequences out of the probability / ghost-note features. Audio-thread
    /// safe; just overwrites the existing state.
    pub fn set_rng_seed(&mut self, seed: u32) {
        self.rng = Xorshift::new(seed);
    }

    /// Push a deferred trigger onto the pending queue. Drops the
    /// request if the queue is already at `PENDING_TRIGGER_CAPACITY` so
    /// `tick()` is guaranteed never to allocate. Returns `true` if the
    /// trigger was queued, `false` if the queue was full.
    ///
    /// On overflow: bumps `self.pending_overflows` (an `AtomicU64` that
    /// tests / dashboards can read off-thread) and emits a single
    /// `eprintln!` for the whole session via `PENDING_OVERFLOW_WARN`.
    /// Recurring overflows after the first are silent on stderr but
    /// keep accumulating in the counter.
    pub fn queue_pending(
        &mut self,
        slot: usize,
        velocity: f32,
        samples_from_now: u64,
        bpm: f32,
    ) -> bool {
        if self.pending.len() >= PENDING_TRIGGER_CAPACITY {
            self.pending_overflows.fetch_add(1, Ordering::Relaxed);
            PENDING_OVERFLOW_WARN.call_once(|| {
                eprintln!(
                    "drummr: pending-trigger queue overflowed at capacity {} \
                     — dropping deferred fires. Raise PENDING_TRIGGER_CAPACITY \
                     or reduce simultaneous polymetric voices.",
                    PENDING_TRIGGER_CAPACITY
                );
            });
            return false;
        }
        self.pending.push_back(PendingTrigger {
            slot,
            velocity,
            fire_at_sample: self.samples_processed.wrapping_add(samples_from_now),
            bpm_at_queue: bpm,
        });
        true
    }

    /// Drain any pending triggers whose `fire_at_sample` has elapsed,
    /// firing each one against its slot's voice. Called once per
    /// `tick()` before the audio sum.
    fn drain_pending(&mut self) {
        // Iterate with `swap_remove_back` so we don't pay an O(N) shift
        // on every fire. Today this is safe: `voice.trigger` runs the
        // engine-level `trigger()` (not `KitEngine::trigger`), so it
        // never pushes new entries onto `self.pending`. Any element
        // swapped into position `i` was already in the queue before
        // this drain started, so re-checking it just resolves a stale
        // entry that was always pending in this tick.
        //
        // If any future change makes `voice.trigger` (or anything it
        // calls) spawn new pending entries — e.g. recursive flams, or
        // re-entering `KitEngine::trigger` — this loop will need a
        // fixed snapshot of the original length to avoid re-checking
        // just-pushed entries that should fire next tick.
        let mut i = 0;
        while i < self.pending.len() {
            if self.pending[i].fire_at_sample <= self.samples_processed {
                let entry = self.pending.swap_remove_back(i).unwrap();
                if entry.slot < 16 {
                    if let Some(voice) = &mut self.voices[entry.slot] {
                        self.postfx[entry.slot].reset();
                        voice.trigger(entry.velocity, entry.bpm_at_queue);
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
                engine.pans[idx] = sound.pan.unwrap_or(0.0).clamp(-1.0, 1.0);
                engine.levels[idx] = sound.level.unwrap_or(1.0).clamp(0.0, 2.0);
                engine.drives[idx] = sound.drive.unwrap_or(0.0).clamp(0.0, 1.0);
                engine.mutes[idx] = sound.mute.unwrap_or(false);
                engine.postfx[idx].set_bits(sound.bits.unwrap_or(16.0));
                engine.postfx[idx].set_rate(sound.rate.unwrap_or(1.0));
                engine.postfx[idx].set_attack_shaper(sound.attack_shaper.unwrap_or(0.0));
                engine.postfx[idx].set_sustain_shaper(sound.sustain_shaper.unwrap_or(0.0));

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

                // Same treatment for the per-slot rhythm pattern.
                if let Some(pat) = sound.pattern {
                    let mut bounded = pat;
                    if bounded.len() > MAX_PATTERN_STEPS_PER_PRIMARY {
                        bounded.truncate(MAX_PATTERN_STEPS_PER_PRIMARY);
                    }
                    engine.pattern[idx] = bounded;
                }

                // Resolve generative settings. Each field defaults to
                // its non-feature value (probability=1, ghost=0, etc.)
                // when the DrumSound doesn't set it, so a slot opts in
                // by setting any one field rather than all of them.
                engine.generative[idx] = GenerativeSettings {
                    trigger_probability: sound
                        .trigger_probability
                        .unwrap_or(1.0)
                        .clamp(0.0, 1.0),
                    ghost_probability: sound
                        .ghost_probability
                        .unwrap_or(0.0)
                        .clamp(0.0, 1.0),
                    ghost_offset_ms: sound.ghost_offset_ms.unwrap_or(60.0).max(0.0),
                    ghost_velocity_factor: sound
                        .ghost_velocity_factor
                        .unwrap_or(0.3)
                        .clamp(0.0, 1.0),
                };
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
                // Skip fallback if the user has explicitly defined a mapping for this slot (even a sentinel note >= 128)
                if !mappings.iter().any(|m| m.slot == idx) {
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
                match param {
                    "level" => self.levels[slot] = value.clamp(0.0, 2.0),
                    "drive" => self.drives[slot] = value.clamp(0.0, 1.0),
                    "mute" => self.mutes[slot] = value > 0.5,
                    _ => voice.set_param(param, value),
                }
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

    /// Mutate one generative-trigger setting for a slot. `param` is one of
    /// "trigger_probability", "ghost_probability", "ghost_offset_ms",
    /// "ghost_velocity_factor". Values are clamped on entry so the audio
    /// thread never observes out-of-range probabilities. Unknown params /
    /// out-of-range slots are silently ignored.
    pub fn set_generative(&mut self, slot: usize, param: &str, value: f32) {
        if slot >= 16 {
            return;
        }
        let g = &mut self.generative[slot];
        match param {
            "trigger_probability" => g.trigger_probability = value.clamp(0.0, 1.0),
            "ghost_probability" => g.ghost_probability = value.clamp(0.0, 1.0),
            "ghost_offset_ms" => g.ghost_offset_ms = value.max(0.0),
            "ghost_velocity_factor" => g.ghost_velocity_factor = value.clamp(0.0, 1.0),
            _ => {}
        }
    }

    /// Mutate a tempo-locked beat division on a slot's voice. See
    /// `Voice::set_division` for the supported `param` values. A `None`
    /// value clears the division.
    pub fn set_division(
        &mut self,
        slot: usize,
        param: &str,
        division: Option<crate::dsp::timing::BeatDivision>,
    ) {
        if slot < 16 {
            if let Some(voice) = &mut self.voices[slot] {
                voice.set_division(param, division);
            }
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
        if self.mutes[slot] {
            return;
        }

        // Generative gate: a single RNG roll decides BOTH whether the
        // primary fires and (if it does) whether a ghost note spawns.
        // One roll keeps the test sequence reproducible against a
        // seeded RNG. The roll is consumed unconditionally so the
        // sequence is the same regardless of which slot fires.
        let gen_settings = self.generative[slot];
        let roll = self.rng.next_f32();
        if roll > gen_settings.trigger_probability {
            // Primary suppressed; no sub-hits, no pattern, no ghost.
            return;
        }

        // Primary hit.
        if let Some(voice) = &mut self.voices[slot] {
            self.postfx[slot].reset();
            voice.trigger(velocity, bpm);
        } else {
            return;
        }

        // Ghost note: the SAME roll that gated the primary also gates
        // the ghost. Because the primary survived, `roll <=
        // trigger_probability` is already known; the ghost additionally
        // requires `roll < ghost_probability`. This couples the two
        // gates — exact conditional rates:
        //   * If ghost_probability <= trigger_probability, the fraction
        //     of SURVIVORS that ghost is
        //     `ghost_probability / trigger_probability`.
        //     e.g. trigger=0.5, ghost=0.4 → 0.4/0.5 = 80% of survivors
        //     ghost. (Not 40% — both gates pass on the same roll.)
        //   * If ghost_probability > trigger_probability, every survivor
        //     ghosts (the survivor's roll is already in
        //     [0, trigger_probability], which is a subset of
        //     [0, ghost_probability)).
        // The unconditional ghost rate (per hit, not per survivor) is
        // `min(trigger_probability, ghost_probability)` because a hit
        // ghosts iff `roll < min(trigger_p, ghost_p)`. This shared-roll
        // semantic is intentional: one RNG draw per primary keeps the
        // sequence reproducible against a seeded RNG. Authors who want
        // statistically-independent gates should set
        // `ghost_probability = trigger_probability * desired_conditional`.
        if gen_settings.ghost_probability > 0.0 && roll < gen_settings.ghost_probability {
            let sr = self.sample_rate;
            let samples_offset = (gen_settings.ghost_offset_ms * sr / 1000.0) as u64;
            let ghost_vel =
                (velocity * gen_settings.ghost_velocity_factor).clamp(0.0, 1.0);
            self.queue_pending(slot, ghost_vel, samples_offset, bpm);
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
                self.queue_pending(slot, sub_vel, samples_offset, bpm);
            }
        }

        // Rhythm pattern: each step resolves to an offset of
        // `division.to_seconds(bpm) * multiplier` seconds, converted
        // to samples at the engine's sample_rate. Same bounded
        // iteration pattern as sub-hits.
        let pat_count = self.pattern[slot]
            .len()
            .min(MAX_PATTERN_STEPS_PER_PRIMARY);
        if pat_count > 0 {
            let sr = self.sample_rate;
            for i in 0..pat_count {
                let step = self.pattern[slot][i].clone();
                let mult = step.multiplier.max(0.0);
                let offset_sec = step.division.to_seconds(bpm) * mult;
                let samples_offset = (offset_sec * sr).max(0.0) as u64;
                let step_vel = (velocity * step.velocity_factor).clamp(0.0, 1.0);
                self.queue_pending(slot, step_vel, samples_offset, bpm);
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

    pub fn tick(&mut self) -> (f32, f32) {
        // Drain BEFORE bumping the counter so a "fire 0 samples from now"
        // entry (i.e. queued with `samples_from_now == 0`) fires on the
        // VERY first tick after `queue_pending`, contributing to the same
        // audio sample as the primary. Previously we bumped first, which
        // meant a zero-offset pending had `fire_at == sp_at_queue` but
        // was compared against `sp_at_queue + 1` — so it fired one tick
        // late.
        //
        // Invariant maintained: a pending queued with `samples_from_now
        // = K` fires on the K-th subsequent tick (counting from 1).
        // For K=0 that means "this very next tick"; for K=100 that
        // means "the 100th subsequent tick" (so existing offset-100
        // tests still pass — the K-th tick is still K ticks away
        // because we still bump once per tick, just after the drain).
        if !self.pending.is_empty() {
            self.drain_pending();
        }

        // Bump the monotonic counter AFTER the drain. The counter now
        // represents "the index of the sample we're about to emit"
        // (1-based; the very first call to tick() emits sample 1).
        self.samples_processed = self.samples_processed.wrapping_add(1);

        let mut out_l = 0.0;
        let mut out_r = 0.0;
        for (i, voice_opt) in self.voices.iter_mut().enumerate() {
            if let Some(voice) = voice_opt {
                // Inactive-voice fast-path: no engine work AND no PostFx
                // process call.
                if !voice.is_active() {
                    continue;
                }
                let raw = voice.tick();
                
                // Per-voice drive (saturation)
                let drive = self.drives[i];
                let saturated = if drive > 0.0 {
                    // Fast saturation: mix between raw and soft-clipped signal.
                    // Scale input to soft-clip to hit the knee harder as drive increases.
                    let driven = raw * (1.0 + drive * 3.0);
                    let clipped = crate::dsp::utils::soft_clip(driven);
                    raw + (clipped - raw) * drive
                } else {
                    raw
                };

                // Per-voice level
                let level = if self.mutes[i] { 0.0 } else { self.levels[i] };
                let amplified = saturated * level;

                let postfx = &mut self.postfx[i];
                let signal = if postfx.is_passthrough() {
                    amplified
                } else {
                    postfx.process(amplified)
                };

                // Constant-power panning
                let pan = self.pans[i];
                let p = (pan + 1.0) * 0.5; // 0.0 to 1.0
                let angle = p * std::f32::consts::FRAC_PI_2;
                out_l += signal * angle.cos();
                out_r += signal * angle.sin();
            }
        }
        (out_l.clamp(-1.0, 1.0), out_r.clamp(-1.0, 1.0))
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
        // Queue a trigger 100 samples into the future. Because `tick()`
        // drains BEFORE bumping `samples_processed`, the entry fires on
        // the (samples_from_now + 1)-th subsequent tick: by the start
        // of that tick `samples_processed` (= K) has reached the
        // queue's `fire_at_sample` (= K), so the drain's `<=` test is
        // first true there.
        let queued = engine.queue_pending(0, 1.0, 100, 120.0);
        assert!(queued, "expected queue to accept entry");
        assert_eq!(engine.pending.len(), 1);

        // Tick 100 samples — should not fire (drain runs first while
        // samples_processed is still 99, so 100 <= 99 is false).
        for _ in 0..100 {
            engine.tick();
        }
        assert_eq!(engine.pending.len(), 1, "should not have fired yet at 100 samples");

        // Tick the 101st — drain sees samples_processed == 100, fires.
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
            assert!(engine.queue_pending(0, 1.0, 1000, 120.0));
        }
        // One more should be rejected (no allocation, returns false).
        assert!(!engine.queue_pending(0, 1.0, 1000, 120.0));
        assert_eq!(engine.pending.len(), PENDING_TRIGGER_CAPACITY);
    }

    #[test]
    fn test_pending_does_not_fire_when_slot_empty() {
        let mut engine = KitEngine::new(SR);
        // No voice in slot 0; queue should still pop cleanly without panic.
        engine.queue_pending(0, 1.0, 5, 120.0);
        for _ in 0..10 {
            engine.tick();
        }
        assert_eq!(engine.pending.len(), 0);
    }
}
