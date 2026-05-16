import { Waves } from "@phosphor-icons/react"
import { Slider } from './ui'

export function ModulationPanel({ lfo1_freq, lfo2_freq, onChangeLfo, modValues }: { 
  lfo1_freq: number, 
  lfo2_freq: number, 
  onChangeLfo: (index: number, freq: number) => void,
  modValues?: number[]
}) {
  return (
    <section className="bg-card/30 border border-border rounded-3xl p-5 flex flex-col gap-5">
      <header className="flex items-center gap-2 text-xs font-black text-primary uppercase tracking-[0.18em]">
        <Waves size={14} />
        4. Modulation
      </header>

      <div className="grid grid-cols-1 md:grid-cols-2 2xl:grid-cols-1 gap-5">
        <div className="space-y-3 p-3 bg-background/30 rounded-xl border border-border/50">
          <div className="flex items-center justify-between">
            <div className="text-[10px] font-black text-muted-foreground uppercase tracking-widest">LFO 1</div>
            {modValues && (
              <div
                aria-label="LFO 1 activity"
                className="w-2 h-2 rounded-full bg-primary shadow-[0_0_8px_var(--color-primary)] transition-all duration-75"
                style={{ opacity: (modValues[1] + 1) / 2 }}
              />
            )}
          </div>
          <Slider
            label="Rate"
            value={lfo1_freq}
            min={0.1} max={20} step={0.1}
            format={v => `${v.toFixed(2)} Hz`}
            onChange={v => onChangeLfo(1, v)}
          />
        </div>

        <div className="space-y-3 p-3 bg-background/30 rounded-xl border border-border/50">
          <div className="flex items-center justify-between">
            <div className="text-[10px] font-black text-muted-foreground uppercase tracking-widest">LFO 2</div>
            {modValues && (
              <div
                aria-label="LFO 2 activity"
                className="w-2 h-2 rounded-full bg-primary shadow-[0_0_8px_var(--color-primary)] transition-all duration-75"
                style={{ opacity: (modValues[2] + 1) / 2 }}
              />
            )}
          </div>
          <Slider
            label="Rate"
            value={lfo2_freq}
            min={0.1} max={20} step={0.1}
            format={v => `${v.toFixed(2)} Hz`}
            onChange={v => onChangeLfo(2, v)}
          />
        </div>
      </div>

      <div className="mt-auto pt-4 border-t border-border/50">
        <div className="text-[10px] font-medium text-muted-foreground italic">
          Global LFO rates. Assign sources to parameters in the Timbre column.
        </div>
      </div>
    </section>
  );
}
