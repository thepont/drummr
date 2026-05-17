import { Waves, Clock } from "@phosphor-icons/react"
import { Slider } from './ui'
import { InfoTooltip } from './InfoTooltip'

/** Beat-division name -> length in quarter notes. Mirrors
 *  `BeatDivision::to_seconds` in `src/dsp/timing.rs` so the UI can
 *  preview the locked Hz / ms without a round-trip. */
const DIVISION_QUARTERS: Record<string, number> = {
  ThirtySecond: 0.125,
  SixteenthTriplet: 1.0 / 6.0,
  Sixteenth: 0.25,
  SixteenthDotted: 0.375,
  EighthTriplet: 1.0 / 3.0,
  Eighth: 0.5,
  EighthDotted: 0.75,
  QuarterTriplet: 2.0 / 3.0,
  Quarter: 1.0,
  QuarterDotted: 1.5,
  Half: 2.0,
  Bar: 4.0,
  TwoBars: 8.0,
  FourBars: 16.0,
};

function divisionToSeconds(name: string, bpm: number): number | null {
  const q = DIVISION_QUARTERS[name];
  if (q === undefined) return null;
  return (q * 60.0) / Math.max(bpm, 0.01);
}

function lfoHint(division: string, bpm: number): string {
  const sec = divisionToSeconds(division, bpm);
  if (sec === null || !isFinite(sec) || sec <= 0) {
    return `Tempo-locked to ${division}`;
  }
  const hz = 1.0 / sec;
  return `Tempo-locked to ${division} @ ${bpm.toFixed(1)} BPM (~${hz.toFixed(2)} Hz)`;
}

export function ModulationPanel({
  lfo1_freq,
  lfo2_freq,
  onChangeLfo,
  modValues,
  lfo1_division = null,
  lfo2_division = null,
  bpm = 120.0,
}: {
  lfo1_freq: number,
  lfo2_freq: number,
  onChangeLfo: (index: number, freq: number) => void,
  modValues?: number[],
  /** If set, the engine overrides the static Hz at trigger time; the
   *  static slider is disabled and an amber tempo-lock badge is shown. */
  lfo1_division?: string | null,
  lfo2_division?: string | null,
  /** Live BPM. Used to render a "~X Hz" preview alongside the division
   *  name. Falls back to 120 so the panel still renders sensibly if a
   *  caller forgets to pass it. */
  bpm?: number,
}) {
  const lfo1Locked = !!lfo1_division;
  const lfo2Locked = !!lfo2_division;
  return (
    <section className="bg-card/30 border border-border rounded-3xl p-5 flex flex-col gap-5">
      <header className="flex items-center justify-between gap-2 text-xs font-black text-primary uppercase tracking-[0.18em]">
        <span className="flex items-center gap-2">
          <Waves size={14} />
          4. Modulation
          <InfoTooltip
            size={14}
            text="LFOs and the mod matrix. Each slot has two LFOs that run at a chosen rate (Hz, or tempo-locked to a beat division). LFOs can be routed to any modulatable parameter from the Timbre section, with positive or negative depth."
          />
        </span>
        <span className="text-[10px] font-medium text-muted-foreground italic normal-case tracking-normal hidden md:inline">
          Global LFO rates. Assign sources to parameters in Timbre.
        </span>
      </header>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-5">
        <div className="space-y-3 p-3 bg-background/30 rounded-xl border border-border/50">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <div className="text-[10px] font-black text-muted-foreground uppercase tracking-widest">LFO 1</div>
              <InfoTooltip text="How fast LFO 1 cycles. Lower = slower modulation. If a tempo-lock division is set, the rate is derived from the current BPM and this slider is disabled." />
              {lfo1Locked && (
                <span
                  title={lfoHint(lfo1_division!, bpm)}
                  className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded-full bg-amber-500/15 border border-amber-500/40 text-amber-300 text-[9px] font-bold uppercase tracking-wider"
                >
                  <Clock size={9} weight="fill" />
                  Tempo-locked
                </span>
              )}
            </div>
            {modValues && (
              <div
                aria-label="LFO 1 activity"
                className="w-2 h-2 rounded-full bg-primary shadow-[0_0_8px_var(--color-primary)] transition-all duration-75"
                style={{ opacity: (modValues[1] + 1) / 2 }}
              />
            )}
          </div>
          <Slider
            label={lfo1Locked ? "Rate (tempo-locked)" : "Rate"}
            value={lfo1_freq}
            min={0.1} max={20} step={0.1}
            format={v => `${v.toFixed(2)} Hz`}
            onChange={v => onChangeLfo(1, v)}
            disabled={lfo1Locked}
            disabledHint={lfo1Locked ? lfoHint(lfo1_division!, bpm) : undefined}
          />
        </div>

        <div className="space-y-3 p-3 bg-background/30 rounded-xl border border-border/50">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <div className="text-[10px] font-black text-muted-foreground uppercase tracking-widest">LFO 2</div>
              <InfoTooltip text="How fast LFO 2 cycles. Lower = slower modulation. If a tempo-lock division is set, the rate is derived from the current BPM and this slider is disabled." />
              {lfo2Locked && (
                <span
                  title={lfoHint(lfo2_division!, bpm)}
                  className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded-full bg-amber-500/15 border border-amber-500/40 text-amber-300 text-[9px] font-bold uppercase tracking-wider"
                >
                  <Clock size={9} weight="fill" />
                  Tempo-locked
                </span>
              )}
            </div>
            {modValues && (
              <div
                aria-label="LFO 2 activity"
                className="w-2 h-2 rounded-full bg-primary shadow-[0_0_8px_var(--color-primary)] transition-all duration-75"
                style={{ opacity: (modValues[2] + 1) / 2 }}
              />
            )}
          </div>
          <Slider
            label={lfo2Locked ? "Rate (tempo-locked)" : "Rate"}
            value={lfo2_freq}
            min={0.1} max={20} step={0.1}
            format={v => `${v.toFixed(2)} Hz`}
            onChange={v => onChangeLfo(2, v)}
            disabled={lfo2Locked}
            disabledHint={lfo2Locked ? lfoHint(lfo2_division!, bpm) : undefined}
          />
        </div>
      </div>

    </section>
  );
}
