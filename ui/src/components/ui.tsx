import { clsx, type ClassValue } from 'clsx'
import { twMerge } from 'tailwind-merge'

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

export function Card({ title, value, icon, className }: { title: string, value: string, icon?: React.ReactNode, className?: string }) {
  return (
    <div className={cn("bg-card border border-border p-6 rounded-2xl flex items-start gap-4 transition-all hover:border-primary/20", className)}>
      {icon && (
        <div className="w-10 h-10 rounded-xl bg-muted flex items-center justify-center text-muted-foreground">
          {icon}
        </div>
      )}
      <div className="space-y-1 min-w-0 flex-1">
        <span className="text-xs font-semibold text-muted-foreground uppercase tracking-wider">{title}</span>
        <p className="text-lg font-bold truncate">{value}</p>
      </div>
    </div>
  )
}

export function Button({ 
  children, 
  onClick, 
  variant = 'primary', 
  className,
  disabled,
  icon: Icon
}: { 
  children: React.ReactNode, 
  onClick?: () => void, 
  variant?: 'primary' | 'secondary' | 'destructive' | 'ghost',
  className?: string,
  disabled?: boolean,
  icon?: React.ReactNode
}) {
  const variants = {
    primary: "bg-primary text-primary-foreground hover:scale-105 active:scale-95 shadow-lg shadow-primary/20",
    secondary: "bg-secondary text-secondary-foreground hover:bg-muted",
    destructive: "bg-destructive/10 text-destructive hover:bg-destructive/20",
    ghost: "text-muted-foreground hover:bg-muted hover:text-foreground"
  };

  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className={cn(
        "flex items-center justify-center gap-2 px-6 py-2 rounded-full font-bold transition-all duration-200 disabled:opacity-50 disabled:pointer-events-none text-sm focus:outline-none focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-offset-2 focus-visible:ring-offset-background",
        variants[variant],
        className
      )}
    >
      {Icon}
      {children}
    </button>
  );
}

export function Slider({ label, value, min, max, step, onChange, format = (v: number) => v.toFixed(2), className, modValue, disabled = false, disabledHint }: {
  label: string,
  value: number,
  min: number,
  max: number,
  step: number,
  onChange: (v: number) => void,
  format?: (v: number) => string,
  className?: string,
  modValue?: number,
  /** When true, the slider is non-interactive and visually dimmed.
   *  Used by the tempo-lock indicator (lfo*_division / decay_division)
   *  to communicate that the static value is overridden at trigger time. */
  disabled?: boolean,
  /** Optional secondary line shown under the label (only when disabled). */
  disabledHint?: string,
}) {
  const modPos = modValue !== undefined
    ? ((modValue - min) / (max - min)) * 100
    : undefined;

  return (
    <div className={cn("space-y-2", className, disabled && "opacity-60")}>
      <div className="flex items-center justify-between gap-2">
        <span className="text-xs font-medium text-muted-foreground uppercase tracking-wider truncate">{label}</span>
        <span className="text-sm font-mono font-bold bg-muted px-2 py-1 rounded-md tabular-nums whitespace-nowrap">{format(value)}</span>
      </div>
      <div className="relative group/track">
        <input
          type="range"
          aria-label={label}
          min={min} max={max} step={step}
          value={value}
          disabled={disabled}
          onChange={(e) => onChange(parseFloat(e.target.value))}
          onDoubleClick={() => {
            if (!disabled) {
              if (min <= 0 && max >= 0) {
                onChange(0);
              } else {
                onChange((min + max) / 2);
              }
            }
          }}
          className={cn(
            "w-full h-2 bg-muted rounded-full appearance-none accent-primary transition-all focus:outline-none focus-visible:ring-2 focus-visible:ring-primary/50 focus-visible:ring-offset-2 focus-visible:ring-offset-background",
            disabled
              ? "cursor-not-allowed pointer-events-none accent-amber-400"
              : "cursor-pointer hover:accent-primary/80"
          )}
        />
        {modPos !== undefined && (
          <div
            data-testid="mod-indicator"
            aria-label={`Modulated value: ${format(modValue!)}`}
            className="absolute top-1/2 -translate-y-1/2 h-4 w-1 bg-primary/70 rounded-full shadow-[0_0_8px_var(--color-primary)] transition-all duration-75 pointer-events-none border border-primary"
            style={{ left: `${Math.max(0, Math.min(100, modPos))}%`, transform: 'translate(-50%, -50%)' }}
          />
        )}
      </div>
      {disabled && disabledHint && (
        <div className="text-[10px] font-medium text-amber-300/90 italic leading-tight">
          {disabledHint}
        </div>
      )}
    </div>
  )
}

export interface ModSlotData {
  source: string;
  depth: f32;
}

type f32 = number;

import { useRef, useEffect } from 'react';

export function Sparkline({ value, min, max, className }: { value: number, min: number, max: number, className?: string }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const historyRef = useRef<number[]>([]);
  const frameRef = useRef<number>(0);

  useEffect(() => {
    const history = historyRef.current;
    history.push(value);
    if (history.length > 60) history.shift(); // Keep last 60 points (~1.5s at 40ms updates)

    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const draw = () => {
      const { width, height } = canvas;
      ctx.clearRect(0, 0, width, height);
      
      if (history.length < 2) return;

      ctx.beginPath();
      ctx.strokeStyle = 'rgba(52, 211, 153, 0.5)'; // primary-400 with opacity
      ctx.lineWidth = 2;
      ctx.lineJoin = 'round';

      for (let i = 0; i < history.length; i++) {
        const x = (i / (history.length - 1)) * width;
        const normalized = (history[i] - min) / (max - min);
        const y = height - (normalized * height);
        
        if (i === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      }
      ctx.stroke();
    };

    frameRef.current = requestAnimationFrame(draw);
    return () => cancelAnimationFrame(frameRef.current);
  }, [value, min, max]);

  return (
    <canvas 
      ref={canvasRef} 
      width={100} 
      height={30} 
      className={cn("bg-muted/20 rounded opacity-50", className)} 
    />
  );
}

export function PredictiveGraph({ 
  base, min, max, mods, attack, decay, lfo1_freq, lfo2_freq, className 
}: { 
  base: number, 
  min: number, 
  max: number, 
  mods: ModSlotData[], 
  attack: number, 
  decay: number,
  lfo1_freq?: number,
  lfo2_freq?: number,
  className?: string 
}) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const draw = () => {
      const { width, height } = canvas;
      ctx.clearRect(0, 0, width, height);

      const totalSeconds = 2.0;
      const points = 100;
      const range = max - min;
      const baseNorm = (base - min) / range;
      const baseY = height - (baseNorm * height);

      const activeMods = {
        env: mods.some(m => m.source === 'Envelope' && Math.abs(m.depth) > 0.01),
        lfo1: mods.some(m => m.source === 'Lfo1' && Math.abs(m.depth) > 0.01),
        lfo2: mods.some(m => m.source === 'Lfo2' && Math.abs(m.depth) > 0.01),
      };

      // 1. Draw Individual Components (Faint)
      const drawComponent = (color: string, calc: (t: number) => number) => {
        ctx.beginPath();
        ctx.strokeStyle = color;
        ctx.setLineDash([2, 2]);
        ctx.lineWidth = 1;
        for (let i = 0; i <= points; i++) {
          const t = (i / points) * totalSeconds;
          const val = calc(t);
          const x = (i / points) * width;
          const y = height - (((base + val * (range * 0.5)) - min) / range) * height;
          if (i === 0) ctx.moveTo(x, y);
          else ctx.lineTo(x, y);
        }
        ctx.stroke();
        ctx.setLineDash([]);
      };

      if (activeMods.env) {
        drawComponent('rgba(251, 191, 36, 0.3)', (t) => {
          const a = attack / 1000, d = decay / 1000;
          const depth = mods.find(m => m.source === 'Envelope')?.depth || 0;
          return (t < a ? t / a : (t < a + d ? 1.0 - (t - a) / d : 0)) * depth;
        });
      }

      if (activeMods.lfo1 || activeMods.lfo2) {
        drawComponent('rgba(96, 165, 250, 0.2)', (t) => {
          const m1 = mods.find(m => m.source === 'Lfo1'), m2 = mods.find(m => m.source === 'Lfo2');
          const v1 = m1 ? Math.sin(2 * Math.PI * (lfo1_freq || 1) * t) * m1.depth : 0;
          const v2 = m2 ? Math.sin(2 * Math.PI * (lfo2_freq || 1) * t) * m2.depth : 0;
          return v1 + v2;
        });
      }

      // 2. Draw Final Combined Path (Bold)
      ctx.beginPath();
      ctx.strokeStyle = 'rgba(52, 211, 153, 0.8)';
      ctx.lineWidth = 2;
      
      const grad = ctx.createLinearGradient(0, 0, 0, height);
      grad.addColorStop(0, 'rgba(52, 211, 153, 0.2)');
      grad.addColorStop(1, 'rgba(52, 211, 153, 0)');
      ctx.fillStyle = grad;

      for (let i = 0; i <= points; i++) {
        const t = (i / points) * totalSeconds;
        const a = attack / 1000, d = decay / 1000;
        const env = t < a ? t / a : (t < a + d ? 1.0 - (t - a) / d : 0);
        const lfo1 = Math.sin(2 * Math.PI * (lfo1_freq || 1) * t);
        const lfo2 = Math.sin(2 * Math.PI * (lfo2_freq || 1) * t);

        let totalMod = 0;
        mods.forEach(m => {
          if (m.source === 'Envelope') totalMod += env * m.depth;
          if (m.source === 'Lfo1') totalMod += lfo1 * m.depth;
          if (m.source === 'Lfo2') totalMod += lfo2 * m.depth;
          if (m.source === 'Velocity') totalMod += 1.0 * m.depth;
        });

        const scaledVal = base + (totalMod * (range * 0.5)); 
        const x = (i / points) * width;
        const normalized = (scaledVal - min) / range;
        const y = height - (normalized * height);

        if (i === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      }
      ctx.stroke();
      ctx.lineTo(width, baseY);
      ctx.lineTo(0, baseY);
      ctx.fill();
    };

    draw();

  }, [base, min, max, mods, attack, decay, lfo1_freq, lfo2_freq]);

  return (
    <div className="relative group">
      <canvas ref={canvasRef} width={120} height={40} className={cn("bg-muted/10 rounded-lg", className)} />
      <div className="absolute inset-0 flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity bg-background/40 backdrop-blur-[1px] rounded-lg">
         <span className="text-[8px] font-black uppercase text-primary">Lifecycle</span>
      </div>
    </div>
  );
}

export function ParamController({ 
  label, value, min, max, step, onChange, format, 
  mods = [], onModChange,
  modValue,
  attack = 1,
  decay = 200,
  lfo1_freq = 1.0,
  lfo2_freq = 1.0
}: { 
  label: string, 
  value: number, 
  min: number, 
  max: number, 
  step: number, 
  onChange: (v: number) => void,
  format?: (v: number) => string,
  mods?: ModSlotData[],
  onModChange?: (index: number, source: string, depth: number) => void,
  modValue?: number,
  attack?: number,
  decay?: number,
  lfo1_freq?: number,
  lfo2_freq?: number
}) {
  return (
    <div className="space-y-4 group/param">
      <div className="flex items-end gap-4">
        <div className="flex-1">
          <Slider 
            label={label} 
            value={value} 
            min={min} 
            max={max} 
            step={step} 
            onChange={onChange} 
            format={format} 
            modValue={modValue}
          />
        </div>
        <PredictiveGraph 
          base={value}
          min={min} 
          max={max} 
          mods={mods}
          attack={attack}
          decay={decay}
          lfo1_freq={lfo1_freq}
          lfo2_freq={lfo2_freq}
        />
      </div>
      
      {mods.length > 0 && (
        <div className="flex flex-wrap gap-3 items-end pl-2 border-l-2 border-primary/20">
          {mods.map((mod, idx) => (
            <ModSlot 
              key={idx} 
              source={mod.source} 
              depth={mod.depth} 
              onChange={(source, depth) => onModChange?.(idx, source, depth)} 
            />
          ))}
        </div>
      )}
    </div>
  );
}

export function FrequencyVisualizer({ value, min, max, onChange, modValue }: { 
  value: number, 
  min: number, 
  max: number, 
  onChange: (v: number) => void,
  modValue?: number
}) {
  const freqToNote = (f: number) => {
    const notes = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    const semi = 12 * (Math.log2(f / 440)) + 69;
    const noteIdx = Math.round(semi) % 12;
    const octave = Math.floor(Math.round(semi) / 12) - 1;
    return `${notes[noteIdx]}${octave}`;
  };

  const getLogPos = (f: number) => {
    return (Math.log2(f / min) / Math.log2(max / min)) * 100;
  };

  const handleMouseDown = (e: React.MouseEvent<HTMLDivElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    const update = (moveEvent: MouseEvent) => {
      const x = Math.max(0, Math.min(rect.width, moveEvent.clientX - rect.left));
      const percent = x / rect.width;
      // Inverse of log mapping
      const newVal = min * Math.pow(max / min, percent);
      onChange(newVal);
    };

    const handleMouseUp = () => {
      window.removeEventListener('mousemove', update);
      window.removeEventListener('mouseup', handleMouseUp);
    };

    window.addEventListener('mousemove', update);
    window.addEventListener('mouseup', handleMouseUp);
    update(e.nativeEvent as any);
  };

  const pos = getLogPos(value);
  const mPos = modValue ? getLogPos(modValue) : undefined;

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <span className="text-sm font-medium text-muted-foreground italic">Pitch Spectrum</span>
        <span className="text-sm font-mono font-black text-primary bg-primary/10 px-2 py-0.5 rounded border border-primary/20">
          {freqToNote(modValue ?? value)} ({Math.round(modValue ?? value)}Hz)
        </span>
      </div>
      
      <div 
        className="h-12 bg-muted/30 rounded-xl relative overflow-hidden border border-border/50 cursor-ew-resize group"
        onMouseDown={handleMouseDown}
      >
        {/* Background Piano-style grid */}
        <div className="absolute inset-0 flex justify-between px-2 opacity-20 pointer-events-none">
           {[60, 110, 220, 440, 880, 1760].filter(f => f >= min && f <= max).map(f => (
             <div key={f} className="h-full w-[1px] bg-border relative" style={{ left: `${getLogPos(f)}%` }}>
                <span className="absolute bottom-1 left-1 text-[8px] font-bold">{f}Hz</span>
             </div>
           ))}
        </div>

        {/* Base Value Marker */}
        <div 
          className="absolute top-0 bottom-0 w-1 bg-muted-foreground/30 z-10 transition-all duration-200"
          style={{ left: `${pos}%`, transform: 'translateX(-50%)' }}
        />

        {/* Modulated Value Active Bar */}
        {mPos !== undefined && (
          <div 
            className="absolute top-0 bottom-0 bg-primary/20 border-x border-primary/40 shadow-[0_0_15px_var(--color-primary)] transition-all duration-75"
            style={{ 
              left: `${Math.min(pos, mPos)}%`, 
              width: `${Math.abs(mPos - pos)}%` 
            }}
          />
        )}
        
        {/* The "Glow" Head */}
        <div 
          className="absolute top-0 bottom-0 w-1.5 bg-primary shadow-[0_0_10px_var(--color-primary)] z-20 transition-all duration-75"
          style={{ left: `${mPos ?? pos}%`, transform: 'translateX(-50%)' }}
        />
      </div>
      <div className="text-[9px] font-bold text-muted-foreground uppercase tracking-widest text-center opacity-50">
        Logarithmic Piano Spectrum
      </div>
    </div>
  );
}

import { X } from "@phosphor-icons/react"

export function ModSlot({ source, depth, onChange }: { 
  source: string, 
  depth: number, 
  onChange: (source: string, depth: number) => void 
}) {
  // Per-source color coding so the user can scan a mod matrix at a glance.
  const sourceColor =
    source === 'Envelope' ? "bg-amber-500/15 border-amber-500/50 text-amber-400" :
    source === 'Lfo1'     ? "bg-sky-500/15 border-sky-500/50 text-sky-400" :
    source === 'Lfo2'     ? "bg-violet-500/15 border-violet-500/50 text-violet-400" :
    source === 'Velocity' ? "bg-rose-500/15 border-rose-500/50 text-rose-400" :
                            "bg-muted border-border/60 text-muted-foreground hover:border-border hover:text-foreground";

  return (
    <div className="flex flex-col gap-1 items-center group relative">
      <div className="relative min-w-[4rem]">
        <select
          value={source}
          onChange={(e) => onChange(e.target.value, depth)}
          aria-label={`Modulation source for this slot`}
          className={cn(
            "text-[10px] font-black uppercase tracking-wider transition-all px-2.5 py-1 rounded-md border text-center appearance-none cursor-pointer focus:outline-none focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-offset-1 focus-visible:ring-offset-background w-full",
            sourceColor
          )}
        >
          <option value="None" className="bg-card text-muted-foreground">None</option>
          <option value="Envelope" className="bg-card text-amber-400">Shape</option>
          <option value="Lfo1" className="bg-card text-sky-400">LFO 1</option>
          <option value="Lfo2" className="bg-card text-violet-400">LFO 2</option>
          <option value="Velocity" className="bg-card text-rose-400">Hit</option>
        </select>
      </div>

      <div 
        className="h-16 w-1.5 bg-muted rounded-full relative overflow-hidden flex items-end cursor-ns-resize mt-1"
        onDoubleClick={() => onChange(source, 0.0)}
        title="Drag to adjust depth, double-click to reset"
      >
         <input 
          type="range" 
          aria-label="depth"
          min="-1" max="1" step="0.01"
          value={depth}
          onChange={(e) => onChange(source, parseFloat(e.target.value))}
          onDoubleClick={() => onChange(source, 0.0)}
          className="absolute inset-0 w-full h-full opacity-0 cursor-ns-resize z-10"
          style={{ appearance: 'slider-vertical' as any }}
        />
        <div 
          className={cn(
            "w-full transition-all duration-75",
            source === 'None' ? "bg-muted-foreground/20" :
            depth >= 0 ? "bg-primary" : "bg-destructive"
          )}
          style={{ 
            height: `${Math.min(50, Math.abs(depth) * 50)}%`, 
            bottom: depth >= 0 ? '50%' : 'auto', 
            top: depth < 0 ? '50%' : 'auto', 
            position: 'absolute' 
          }}
        />
        <div className="absolute top-1/2 left-0 w-full h-[1px] bg-border/50" />
      </div>

      {source !== 'None' && (
        <button
          onClick={() => onChange('None', 0.0)}
          className="opacity-0 group-hover:opacity-100 transition-opacity absolute -top-2 -right-2 bg-rose-600/90 text-white rounded-full p-0.5 hover:bg-rose-600 shadow-md z-20"
          title="Remove modulation"
        >
          <X size={8} weight="bold" />
        </button>
      )}
    </div>
  )
}
