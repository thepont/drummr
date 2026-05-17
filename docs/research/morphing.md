# Morphing Drum Voices — Velocity-Driven & Time-Evolving Sound Design

## Why morphing matters

Acoustic drums are not single-timbre instruments. A snare struck with brushes, dragged with rods, rim-shotted, or hit with a mallet produces four different sounds — same vessel, four different exciters. A ride cymbal's bell rings pitched at low velocity, washes into shimmer harder, crashes if you lean on it. Velocity on a real kit is a *timbre axis*, not a volume axis.

drummr today applies velocity as a final-sample multiplier (`fm.rs:106`, `phys.rs:121`) plus, optionally, as a modulation source. Each `DrumSound` holds exactly one `Voice` (`kit.rs:19`); engine choice is fixed at kit-build time. A user can route vel→`mod_index` for parametric morph within an engine, but cannot blend a brush sample into a stick sample, and cannot have a kick start as wood-click and decay into metallic ring. This document scopes what it takes to give every slot a true A→B character and recommends a path.

## What's possible today (without code changes)

`ModulationEngine` (`src/dsp/modulation_engine.rs`) caches four sources per voice — `Envelope`, `Lfo1`, `Lfo2`, `Velocity` — and sums them into any `ModulatableParam`. Each engine exposes its character knobs:

- **FM**: `freq`, `mod_ratio`, `mod_index`, `noise_level`.
- **Phys**: `freq`, `brightness`, `dampening`.
- **Granular**: `freq`, `density`, `grain_size`, `jitter`.
- **Hybrid**: `freq`, `noise_color`, `metallic`.
- **Modal**: `freq`, `brightness`, `dampening`, `inharmonicity`.

Multi-source routing on one param is supported (`ModulatableParam::mod_slots: Vec<ModAmount>`), so velocity AND envelope can both steer the same knob. `TODO.md` P1 Track A flags that zero shipped kits use `mods` or LFOs — Track A is itself a morphing exercise.

The hard limit: one engine per slot. You cannot crossfade FM into Modal during the decay, and you cannot dispatch from Granular at low velocity to Hybrid at high velocity. That is the genuinely missing piece.

## Morphing axes

**Categorical (velocity-driven).** (1) Velocity zones: N layers, each in a velocity sub-range — hard transitions, each layer can be a different engine. (2) Velocity crossfade: continuous blend between two layers as velocity sweeps 0→1. (3) Velocity → engine selection: discrete dispatch (Modal soft, Hybrid medium, FM loud). (4) Velocity → character morph: single engine, multiple params driven at once (vel→`mod_index` + vel→`noise_level` + vel→`brightness`). Parametric, shippable today.

**Time-evolving.** (1) A→B crossfade over the envelope: wood click → metallic ring. (2) Envelope-stage switching: attack = noise burst, middle = pitched body, tail = wash. (3) Spectral evolution via modulation: single engine, deep envelope sweep. Modal already does a resonator-activation cascade — `brightness.powi(i)` mode rolloff (`src/dsp/modal.rs:389`) means env→`brightness` activates higher modes sequentially. (4) Wavetable position scrub: envelope sweeps between stored timbres. (5) Resonator activation cascade with per-mode envelopes — Modal would need mode-specific decays to do this properly (all 12 modes share one AD envelope today).

**Hybrid: velocity-keyed time evolution.** Velocity controls both the layer mix AND the morph speed. Soft hits → simple short layer; loud hits → complex layer that morphs A→B fast (stick → resonance → wash). The most expressive option.

## Implementation paths

### Path 1: Sub-engines per slot (~300-500 LOC)

`DrumSound` becomes `DrumSound { layers: Vec<DrumSubvoice>, layer_mode: LayerMode }`. `LayerMode` ∈ {`VelocityZones`, `VelocityCrossfade`, `TimeCrossfade`, `EngineStages`}. `KitEngine` holds `Vec<Voice>` per slot, mixes per `LayerMode` in `tick()`. Pros: unlocks all four categorical morphs and time-crossfade in one shape; reuses every existing engine; the layer abstraction is orthogonal to engine internals. Cons: per-slot CPU scales with layer count; schema migration needed (good moment to merge with the enum-tagged `DrumSound` refactor in `TODO.md` P3); UI gains a layer dimension. Schema sketch:
```toml
[[sounds]]
name = "Brush→Stick Snare"
layer_mode = "VelocityCrossfade"
[[sounds.layers]]
engine_type = "granular"
freq = 220.0; density = 0.3; grain_size = 40.0; decay = 180.0
[[sounds.layers]]
engine_type = "hybrid"
freq = 220.0; noise_color = 0.7; metallic = 0.55; decay = 220.0
```

### Path 2: Dedicated `Voice::Morph` engine (~150-200 LOC)

A new variant holding two `Voice` instances and crossfading per envelope position. Pros: smaller; no schema/UI churn for the A→B case. Cons: cannot do velocity zones; cannot scale past 2 layers without special-casing; `Voice::Morph(Box<Voice>, Box<Voice>)` is recursive enum awkwardness. If Path 1 ever lands, Path 2 is thrown away.

### Path 3: TOML-only via mod matrix (zero LOC)

Push the existing matrix harder:
- vel→`brightness` + env→`inharmonicity` on modal → soft = pure tone, loud opens into inharmonic spectrum evolving over decay.
- vel→`mod_index` + env→`mod_index` on FM → soft = sine-like; loud starts brassy, decays to purer tone.
- vel→`density` + env→`grain_size` on granular → density-driven dynamics, time-driven texture broadening.
- vel→`metallic` + env→`metallic` on hybrid → soft stays wooden, loud starts metallic and decays to wood.

About 70% of the audible benefit at zero LOC. Capped at one engine per slot.

### Path 4: Wavetable engine (~150-200 LOC plus tables)

New engine, `position` parameter interpolates between 2-4 stored waveform tables. Pros: classic synth idiom (PPG, Microwave, Serum); good for evolving hats and vocal-formant toms. Cons: morphs *waveforms*, not *synthesis methods* — cannot crossfade modal-resonator decay into Karplus pluck. Different value proposition than Path 1.

## Producer references

- **Gadd / Bonham / Copeland** — velocity-as-timbre separates a drummer from a beatmaker. Gadd's snare on *Aja* shifts ghost-rim → backbeat crack on the same drum. Bonham's hat on *When the Levee Breaks* moves closed-tick → sizzle wash on velocity alone.
- **Akai MPC / SP-1200 / NI Battery / FXpansion BFD / EZdrummer** — velocity-layer convention, 4 layers per pad (originally) up to 8-16 round-robin × velocity-zoned samples (modern). Path 1, directly.
- **PPG Wave / Waldorf Microwave / Serum** — wavetable position-scrub for time-evolving timbre. Path 4.

## Kit concepts unlocked by morphing

1. **Brush → Stick** (Path 1, VelocityCrossfade). Granular high-density swirl (soft) + hybrid/FM sharp transient (hard). One MIDI pattern at varying velocity shifts from cool jazz to swing-rock. Reference: Roy Haynes brushwork opening into stick attack on accents.
2. **Awakening** (Path 3, TOML-only). Modal voices, env→`brightness` 0.85 over 1.2s. Resonators silent above the fundamental at attack; upper modes fade in across the decay. Reference: Basinski *Disintegration Loops*, drum-hit compressed.
3. **Glass → Metal** (Path 1, TimeCrossfade). Layer A = phys glassy bowl. Layer B = hybrid iron sheet (`metallic = 1.0`). Envelope sweeps A→B. Reference: Aphex Twin *Bucephalus Bouncing Ball*.
4. **Wet → Dry** (Path 1, TimeCrossfade). Granular smear → FM clean tone. Starts as ambient texture, "focuses" into a clean hit — opposite of natural decay. Reference: Burial *Archangel* transient design, inverted.
5. **Drumset 4D** (Path 1, VelocityZones, 4 layers). Brush / stick tip / stick centre / rim-shot at velocity 0-0.25, 0.25-0.6, 0.6-0.9, 0.9-1.0. Rim-shot zone gets its own engine (FM high `mod_index`). Reference: a drum kit.
6. **Solar** (Path 3, TOML-only). vel→`brightness` 0.8 + env→`brightness` 0.5 on modal. Soft hits stay dim; loud hits start bright and brighten further. The kit "gets brighter only when you push it."
7. **Phoenix** (Path 1, EngineStages, 3 stages). 0-30ms = noise burst; 30-300ms = modal pitched body; 300ms+ = granular wash fading over 2s. One hit → 2.3s of drama. Reference: Tim Hecker *Ravedeath, 1972* drums-as-events.

## Recommended path

**Path 3 immediately, Path 1 as the strategic next step.**

Path 3 costs zero LOC and ships Awakening and Solar plus parametric velocity-character morph across every existing kit. It dovetails with the P1 Track A roadmap work — those mod-matrix routings *are* parametric morphing. An afternoon of TOML editing demonstrates the value proposition without DSP risk.

Path 1 is the architectural commitment when morphing graduates from "sometimes we do this with the matrix" to "this is how drummr expresses dynamics." It unlocks categorical velocity zones — the reason acoustic drum samplers exist as a category — on top of every existing engine without rewriting any. The schema migration is the natural moment to merge with the enum-tagged `DrumSound` refactor (`TODO.md` P3): both touch `kit.rs`, TOML, and UI. One ~500 LOC commit instead of two.

Path 2 is a trap: small until you discover it can't do velocity zones; then you build Path 1 and discard Path 2. Path 4 is a different feature — *waveform* morph, not *drum-method* morph — worth doing as a fifth engine eventually but doesn't answer the user's question.

## Engine-specific morphing already possible (TOML examples)

**Modal "Awakening" snare** — sine-pure → full inharmonic spectrum over the decay:
```toml
[[sounds]]
name = "Awakening Snare"
engine_type = "modal"
freq = 220.0; brightness = 0.1; dampening = 0.3; inharmonicity = 0.4
attack = 2.0; decay = 1200.0
mods = [
  {param="brightness", source="Envelope", depth=0.85},
  {param="inharmonicity", source="Envelope", depth=0.4},
]
```

**FM "click-into-thud" kick** — hard hits start as a click and decay into body:
```toml
[[sounds]]
name = "Velocity Kick"
engine_type = "fm"
freq = 55.0; mod_ratio = 1.0; mod_index = 2.0
attack = 1.0; decay = 350.0
mods = [
  {param="mod_index", source="Velocity", depth=8.0},
  {param="mod_index", source="Envelope", depth=-6.0},
  {param="noise_level", source="Velocity", depth=0.4},
]
```

**Hybrid "metal-to-wood" tom** — starts metallic, decays to wood:
```toml
[[sounds]]
name = "Morph Tom"
engine_type = "hybrid"
freq = 140.0; noise_color = 0.5; metallic = 0.9
attack = 1.0; decay = 600.0
mods = [{param="metallic", source="Envelope", depth=-0.7}]
```

**Granular "sparse-to-dense" hat** — soft = three grains, loud = swarm:
```toml
[[sounds]]
name = "Vel Density Hat"
engine_type = "granular"
freq = 6500.0; density = 0.1; grain_size = 25.0; jitter = 0.4
attack = 1.0; decay = 180.0
mods = [
  {param="density", source="Velocity", depth=0.7},
  {param="grain_size", source="Velocity", depth=-10.0},
]
```

## What can't be done with current tools

- **Cross-engine A→B over the envelope.** No `Voice` shape supports two engines coexisting.
- **Categorical velocity zones.** Engine selection is fixed at kit-build time; the matrix can modulate params but can't dispatch.
- **Round-robin variation.** Multiple variants per pad to break repetition — sampler-table territory.
- **Per-mode envelopes in modal.** 12 resonators share one AD envelope; sequential mode "blooming" is approximated only via `brightness.powi(i)`.
- **Mid-engine output interleaving** (one engine's output exciting another's resonator). Path 1 mixes at the slot summer; tighter coupling is outside scope.

## Sources

- NI Battery 4 manual; FXpansion BFD3 docs; Toontrack EZdrummer 3.
- Henkjan Honing, "Structure and Interpretation of Rhythm and Timing" (2002).
- Wolfgang Palm, *The Whole Story of PPG and Waldorf Synthesizers* (2018); Serum manual.
- Smith, J.O., *Physical Audio Signal Processing*, CCRMA online edition (2010), modal synthesis chapter.
- Producer references inline: Gadd *Aja*, Bonham *When the Levee Breaks*, Burial *Archangel*, Tim Hecker *Ravedeath, 1972*, William Basinski *Disintegration Loops*, Aphex Twin *Bucephalus Bouncing Ball*.
