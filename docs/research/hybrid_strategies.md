# Research: Hybrid Drum Synthesis Strategies

This document synthesizes research on state-of-the-art (SOTA) hybrid drum synthesis techniques and architectural patterns, providing a technical blueprint for the "drummr" polymorphic sound engine factory.

## 1. Architectural Patterns

### 1.1 Transient/Sustain Splitting
The most common and effective hybrid architecture. It treats the percussive sound as two distinct functional components:
- **Transient (The "Click"):** A short, high-energy burst (often sample-based or noise) that provides the initial attack.
- **Sustain (The "Body"):** The resonating part of the sound (often FM, Physical Modeling, or a pitch-swept oscillator) that provides the tone and weight.
- **Implementation Note:** Each `SoundEngine` should ideally support an "Attack" vs "Body" internal structure, even if both use the same synthesis method.

### 1.2 Component-Based Synthesis
Building sounds from modular functional blocks:
- **Resonators:** Physical models or high-Q filters that simulate vibrating materials.
- **Exciters:** Impulses, noise, or samples that "strike" the resonators.
- **Re-Synthesis:** Decomposing a sample into tonal and noise parts to allow independent manipulation of pitch and texture.

## 2. Core Synthesis Engines

### 2.1 Enhanced FM Engine
- **Strategy:** Classic Operator-Modulator pairs but with dynamic modulation depth (mod_index) tied to velocity.
- **Innovation:** Using a separate noise carrier for high-frequency "sizzle" on snares and hats.

### 2.2 Physical Modeling (Karplus-Strong Drum Variant)
- **Algorithm:** A filtered delay line with a probabilistic blend factor ($b$).
- **Logic:** 
    - $y[n] = 0.5 \times (y[n-L] + y[n-L-1])$ with probability $b$.
    - $y[n] = -0.5 \times (y[n-L] + y[n-L-1])$ with probability $(1-b)$.
- **Percussive Tuning:** $b=0.5$ for noisy drum-like decay; $b=1.0$ for tonal string-like decay.
- **Excitation:** A filtered white noise burst or a single-cycle sine wave "impulse."

### 2.3 Granular Synthesis for Percussion
- **Technique:** Layering many tiny grains (5–50ms) with high density.
- **Drum Texture:** 
    - High-density grains for the transient "crack."
    - Temporal jitter and grain-size modulation for evolving shakers or "coral harmony" textures.
- **Playhead Modulation:** Moving the grain window through a sample to find sweet spots for different "hit" intensities.

### 2.4 Noise/Additive Hybrid
- **Goal:** Specialized for metallic percussion (Cymbals, Cowbells).
- **Technique:** Summing several non-harmonic oscillators (Additive) passed through a dense noise-modulated filter.
- **Dynamics:** Using a shared AD envelope to keep the "thwack" and "wash" synchronized.

## 3. UI & Control Considerations

### 3.1 Schema-Driven Parameters
To support these diverse engines, the UI must render controls dynamically based on a schema provided by each engine:
- **Standard Params:** Level, Pan, Pitch.
- **Engine-Specific:** Mod Ratio (FM), Dampening (KS), Density (Granular).

### 3.2 Macro-Morphing
A unified "Timbre" or "Morph" control that maps to multiple engine-specific parameters simultaneously, allowing for expressive performance without deep-diving into individual sliders.
