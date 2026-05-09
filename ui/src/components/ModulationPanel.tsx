import { Waves } from "@phosphor-icons/react"
import { Slider } from './ui'

export function ModulationPanel({ lfo1_freq, lfo2_freq, onChangeLfo }: { 
  lfo1_freq: number, 
  lfo2_freq: number, 
  onChangeLfo: (index: number, freq: number) => void 
}) {
  return (
    <section className="bg-card/30 border border-border rounded-3xl p-6 flex flex-col gap-6">
      <header className="flex items-center gap-2 text-sm font-bold text-muted-foreground uppercase tracking-wider">
        <Waves size={18} />
        Modulation Sources
      </header>
      
      <div className="grid grid-cols-1 md:grid-cols-2 gap-8">
        <div className="space-y-4">
          <div className="text-[10px] font-black text-primary uppercase tracking-[0.2em]">LFO 1</div>
          <Slider 
            label="LFO 1 Rate" 
            value={lfo1_freq} 
            min={0.1} max={20} step={0.1} 
            format={v => `${v.toFixed(2)} Hz`}
            onChange={v => onChangeLfo(1, v)}
          />
        </div>

        <div className="space-y-4">
          <div className="text-[10px] font-black text-primary uppercase tracking-[0.2em]">LFO 2</div>
          <Slider 
            label="LFO 2 Rate" 
            value={lfo2_freq} 
            min={0.1} max={20} step={0.1} 
            format={v => `${v.toFixed(2)} Hz`}
            onChange={v => onChangeLfo(2, v)}
          />
        </div>
      </div>
    </section>
  );
}
