# Exotic Physical Models & Out-of-Worldly Synthesis

drummr currently has 6 engines: 1D Karplus-Strong (`phys.rs`), 12-mode bandpass bank
(`modal.rs`), 2-op FM (`fm.rs`), grain-table granular (`granular.rs`), 3-sine + noise
hybrid (`hybrid.rs`), noise (`noise.rs`). That covers the *spectra* of struck
membranes/strings but not the *surfaces* themselves, and nothing outside conventional
Western percussion. This document maps the missing territory.

---

## Section A — Surface physics (drum-head families)

Models that solve the 2-D wave equation rather than collapsing it to a modal
spectrum. Distinct from `modal.rs` because they preserve the *spatial field*: strike
position, edge mutes, asymmetric damping all become free parameters.

### A.1 2-D digital waveguide mesh (Van Duyne & Smith, 1993)

Grid of bidirectional delay units at scattering junctions; coincides with FDTD
on the 2-D wave equation. Strike position changes the spectrum: centre =
fundamental-heavy, edge = brighter. Inharmonicity emerges, not declared.

- **CPU:** O(N²) junctions × ~4 flops. 12×12 mesh × 16 voices ≈ 440 Mflop/s.
  Borderline. 8×8 = 200 Mflop/s, manageable for 4 voices.
- **LOC:** 250-400. No drop-in crates; implement from Smith's CCRMA text.
- **Verdict:** the *real* drum-head model. Different to `modal.rs`.

### A.2 Mass-spring lattice

Each junction = point mass + neighbour springs. Linear-spring limit = mesh; the
nonlinear-spring extension ($F = -k x - \kappa x^3$) gives gong pitch-glide and
high-frequency build-up under loud strikes. ~300 LOC. Refs: IRCAM Modalys, AAS
Chromaphone.

### A.3 Finite Difference / NESS plates (Bilbao)

Direct PDE discretisation. NESS (Edinburgh 2012-17) modelled nonlinear plates in
3-D acoustic rooms on GPU. The killer feature: nonlinear thin-plate behaviour
$\partial_{tt} u = -\kappa^2 \Delta^2 u + \text{NL}(u)$ — real cymbal/gong
crash, the spectral pile-up no modal bank can fake. ~500 LOC + stability work.
Worth it only for cymbals/gongs.

### A.4 Cylindrical / conical waveguide

1D waveguide on a tube. The *resonator body* of conga/tabla/marimba — drummr's
biggest gap. ~50 LOC beyond `phys.rs`. Best as an optional post-stage rather
than a top-level engine.

---

## Section B — Struck-surface variants

- **Tabla / mridangam** — `syahi` paste makes modes 1-5 near-harmonic (Raman 1934).
  `modal.rs` with `explicit_modes` gets ~80% there; the rim/centre coupling
  needs a two-zone strike model.
- **Steelpan / hang** — coupled tuned dimples. Each note rings *and* excites
  neighbours through the shared shell. Needs N coupled modal banks with a
  sparse coupling matrix. `modal.rs` does one note in isolation.
- **Cuíca** (Brazilian friction drum) — stick-slip friction on a membrane
  (McIntyre-Schumacher-Woodhouse 1983 friction model applied to a 2-D surface).
  Sounds like a squeaking dolphin. **No drum synth ships this.**
- **Glass armonica** — same friction primitive applied to a high-Q glass-bowl
  modal bank.
- **Waterphone / bowed metal** — rod-modes + friction. Horror-film stock.

Common primitives: `friction_excite(velocity, pressure)` and `coupled_modal_bank(N)`.
Build them once, get cuíca/armonica/waterphone/steelpan/tabla-rim for free.

---

## Section C — Out-of-worldly synthesis

### C.1 Walsh / Hadamard oscillators

Periodic basis of $\pm 1$ functions. Hard-edged, square-stack, bit-flippy tones
with no analogue equivalent. ~80 LOC. Pairs well as an exciter for `modal.rs`.

### C.2 Chaotic oscillators (Lorenz / Chua / Rössler)

ODE integration at sample rate. Each system has *bifurcation points* where the
dynamics qualitatively change. **Map velocity to the bifurcation parameter**:
soft hit = fixed point (click), medium = limit cycle (pitched), hard = chaos
(spray). Velocity becomes a *timbral* axis. ~60 LOC/oscillator. *The* signature
candidate.

### C.3 Benjolin / Rungler (Hordijk)

Two square VCOs + SVF + 8-bit shift register where OSC-A is data and OSC-B is
clock; last 3 bits go through a tiny DAC into Hordijk's "stepped havoc wave."
Cross-feed back to VCO frequencies and you get pseudo-random patterns with
sudden bifurcations. Iconic noise/glitch source. ~120 LOC.

### C.4 Buchla complex / wavefolder

Two sines cross-modulating into a Lockhart/Serge folder. $y = \text{fold}(\sin(a
\sin(b x) + c x))$ → brassy, FM-adjacent, blooms with velocity. ~100 LOC.

### C.5 Karplus-Strong with in-loop nonlinearity

Add $\tanh$ inside the K-S feedback loop. **Lowest-cost biggest-character-win**:
~10 LOC turns ringing-string into sitar buzz (drive=0.3), jaw-harp twang (0.6),
didgeridoo overblow (1.0+). Already TODO P2.

### C.6 Vocal formants (CHANT / Klatt)

CHANT (Rodet 1984): 5 FOF generators at vowel formants, retriggered at f₀. Klatt
(1980): formant cascade + glottal pulse. Drums that say "ah/ee/oh." Arca / Holly
Herndon / SOPHIE aesthetic. ~150 LOC.

### C.7 Phase distortion (Casio CZ)

Ramp into nonlinear phase-shaping table. Cheap curiosity; drummr already has
better via FM + folders.

### C.8 FFT spectral resynthesis

Magnitude-frame hold, phase randomise, IFFT. Spectra unobtainable any other way,
but not free (4k-FFT overlap-add ≈ 1 Mflop/s/voice). Better placed as a
post-FX freeze than a per-voice engine.

---

## Section D — Ever-decaying / time-stretched models

- **K-S loop-gain ≥ 1.0** — self-oscillating delay line, perpetual ring. One-line
  tweak to `phys.rs`. LFO on dampening keeps it alive.
- **Energy-injection resonator** — trigger *adds* $\alpha \cdot \text{velocity}$
  to the modal-bank state instead of resetting. Resonators evolve across bars;
  bar-4 snare differs from bar-1 because mode 7 has been ringing all along. ~5
  LOC. **The most under-explored idea in drum synthesis.**
- **Reverberant freeze** — capture-and-hold FFT frame. Not physical; place in
  the effects doc, not here.
- **Damped Schrödinger / complex-coefficient PDE** — too academic. Rejected.

---

## Section E — Top 5 to add next

1. **Karplus nonlinearity** (~10 LOC). Cheapest unlock of entirely new
   territory: sitar/jaw-harp/didgeridoo. Already a TODO P2 item.

2. **Chaotic oscillator engine with velocity-bifurcation** (~250 LOC). The
   signature engine. Velocity becomes timbral, not just amplitude. CPU trivial
   (~23 Mflop/s @ 16 voices). Nothing else in commercial drum synths does this.

3. **2-D waveguide mesh, 12×12** (~400 LOC). First *real* drum-head model.
   Strike position, edge mute, centre/edge morph. CPU borderline; restrict to a
   4-voice pool. Unlocks A.1 + tabla/steelpan variants on top.

4. **Friction-exciter primitive + coupled-modal pairing** (~150 LOC). Unlocks
   cuíca, glass armonica, waterphone, bowed cymbal, steelpan halo for the price
   of one primitive.

5. **Energy-injection mode for `modal.rs`** (~20 LOC). Triggers compound
   instead of resetting. Turns the rhythm grid into a temporal modulation
   source. Zero CPU overhead.

---

## Section F — One radical idea (signature engine)

**`Bifurcator`: 3-attractor chaotic oscillators + velocity-bifurcation + inter-voice
coupling bus.**

1. Each voice runs a Lorenz/Rössler/Chua attractor (selectable).
2. Velocity maps to the bifurcation parameter (e.g., Lorenz ρ: <13.93 fixed,
   13.93-24.06 limit cycle, >24.06 chaos). Soft hits click, medium hits sing,
   hard hits spray.
3. **Inter-voice coupling bus**: voice A's output is summed into voice B's $\dot x$
   term. The kick's strange attractor *physically perturbs* the snare's. Not
   sidechain — actual dynamical-system coupling.
4. **The whole kit is one coupled ODE system.** A four-on-the-floor kick
   imprints 4 Hz into the ride's attractor; change the kick pattern and the
   ride's chaos changes too.

Math is well-trodden (Lorenz 1963, Chua 1984), implementation cost ~300+80 LOC,
conceptual payoff unique: **a drum kit that is literally one nonlinear coupled
dynamical system.** Velocity, pattern, and inter-voice relations all become
*physical* parameters of the same shared dynamics. No other drum synth ships
this.

---

## Section G — Kit concepts unlocked

1. **Tabla Machine** — modal + friction-rim + cylindrical body. (Talvin Singh,
   Karsh Kale, Nitin Sawhney.)
2. **Steelpan Halo** — coupled modal banks; every hit excites neighbours.
   (Calypso; also Jon Hopkins-style harmonic kits.)
3. **Glass Cathedral** — bowed glass armonica + infinite-feedback K-S drone bed.
   (Sigur Rós, Stars of the Lid.)
4. **Bifurcator** — the signature engine, velocity-bifurcation coupled chaos.
   (Autechre, Aphex *drukqs*, Holly Herndon.)
5. **Speaks-in-Tongues** — vocal-formant CHANT engine per voice. (Arca, SOPHIE.)
6. **Cuíca Jungle** — friction-membrane percussion with pitch-bend strokes.
   Brazilian carnival × IDM.
7. **Karplus Cathedral** — `phys.rs` at loop-gain 0.999 with LFO on dampening; a
   drone that *is* the snare body. (Ben Frost, Tim Hecker.)
8. **NESS Plate** — nonlinear-plate cymbal/gong. Big-budget signature kit.

---

## Implementation roadmap

Primitives first; each unlocks multiple engines.

**Phase 1 — Cheap big wins:**
1. Add nonlinearity to `phys.rs` loop (E.1).
2. Energy-injection mode in `modal.rs` (E.5).
3. Walsh oscillator (C.1) as alternative `modal.rs` exciter.

**Phase 2 — Primitive library:**

4. Shared SVF (needed for Benjolin / Buchla / formants). *Build this first per
   the project's "SVF first" pattern.*
5. Friction-exciter primitive (McIntyre-Schumacher-Woodhouse stick-slip).
6. Wavefolder primitive (Lockhart cascade) — Buchla mode + post-FX folding.
7. Coupled-modal pairing (sparse coupling matrix).

**Phase 3 — New top-level engines:**

8. `chaos.rs` — Lorenz/Chua/Rössler velocity-bifurcation (E.2).
9. `mesh.rs` — 2-D waveguide mesh (E.3).
10. `formant.rs` — CHANT/Klatt.

**Phase 4 — Stretch:**

11. Benjolin/Rungler.
12. NESS-style nonlinear plate (cymbal/gong only).
13. Inter-voice coupling bus — full Bifurcator vision.

---

## Sources

- Van Duyne, S. A. & Smith, J. O. (1993). [Physical Modeling with the 2-D Digital Waveguide Mesh](https://ccrma.stanford.edu/~jos/pdf/mesh.pdf). ICMC.
- Smith, J. O. [Physical Audio Signal Processing](https://ccrma.stanford.edu/~jos/pasp/) (CCRMA online text — mesh, waveguide, K-S, friction chapters).
- [Modelling a Drum by Interfacing 2-D and 3-D Waveguide Meshes](https://quod.lib.umich.edu/i/icmc/bbp2372.2000.112?rgn=main;view=fulltext), ICMC 2000.
- Bilbao, S. (2009). *Numerical Sound Synthesis: Finite Difference Schemes and Simulation in Musical Acoustics.* Wiley. [(SS)](https://www.semanticscholar.org/paper/Numerical-Sound-Synthesis:-Finite-Difference-and-in-Bilbao/a4f6e67a96ed08b566f49fd29e397c33d7f353b4)
- Bilbao, S. et al. [The NESS Project: Physical Modeling, Algorithms and Sound Synthesis](https://dl.acm.org/doi/10.1162/comj_a_00516). CMJ 43(2-3), 2019.
- Bilbao, S. [Sound Synthesis for Nonlinear Plates](https://www.academia.edu/8728423/SOUND_SYNTHESIS_FOR_NONLINEAR_PLATES).
- McIntyre, M. E., Schumacher, R. T., Woodhouse, J. (1983). *On the oscillations of musical instruments.* JASA 74(5) — friction-string canon.
- Karplus, K. & Strong, A. (1983). *Digital synthesis of plucked-string and drum timbres.* CMJ 7(2).
- Rodet, X. et al. (1984). *The CHANT Project.* CMJ 8(3).
- Klatt, D. (1980). *Software for a cascade/parallel formant synthesizer.* JASA 67(3).
- Lorenz, E. N. (1963). *Deterministic Nonperiodic Flow.* J. Atmos. Sci. 20(2).
- Chua, L. (1984). *The double scroll family.* IEEE TCAS.
- Hordijk, R. — [Harnessing Chaos: the Legacy of the Benjolin](https://www.perfectcircuit.com/signal/rob-hordijk-benjolin); [Rungler wiki](https://sdiy.info/wiki/Rob_Hordijk_Rungler).
- Mutable Instruments. [Plaits documentation](https://pichenettes.github.io/mutable-instruments-documentation/modules/plaits/) — reference for the "16 micro-engines in one module" architecture.
- Raman, C. V. (1934). *The Indian musical drums.* Proc. Indian Acad. Sci. — tabla harmonic modes.
- IRCAM Modalys; AAS Chromaphone — commercial mass-spring/modal references.
- Surge XT / Vital — open-source reference impls of wavefolders, formants, phase distortion.
