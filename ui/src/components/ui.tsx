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
      <div className="space-y-1">
        <span className="text-xs font-semibold text-muted-foreground uppercase tracking-wider">{title}</span>
        <p className="text-lg font-bold truncate max-w-[180px]">{value}</p>
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
        "flex items-center justify-center gap-2 px-6 py-2 rounded-full font-bold transition-all duration-200 disabled:opacity-50 disabled:pointer-events-none text-sm",
        variants[variant],
        className
      )}
    >
      {Icon}
      {children}
    </button>
  );
}

export function Slider({ label, value, min, max, step, onChange, format = (v: number) => v.toFixed(2), className, modValue }: { 
  label: string, 
  value: number, 
  min: number, 
  max: number, 
  step: number, 
  onChange: (v: number) => void,
  format?: (v: number) => string,
  className?: string,
  modValue?: number
}) {
  const modPos = modValue !== undefined 
    ? ((modValue - min) / (max - min)) * 100 
    : undefined;

  return (
    <div className={cn("space-y-3", className)}>
      <div className="flex items-center justify-between">
        <span className="text-sm font-medium text-muted-foreground">{label}</span>
        <span className="text-sm font-mono font-bold bg-muted px-2 py-0.5 rounded">{format(value)}</span>
      </div>
      <div className="relative group/track">
        <input 
          type="range" 
          min={min} max={max} step={step}
          value={value}
          onChange={(e) => onChange(parseFloat(e.target.value))}
          className="w-full h-1.5 bg-muted rounded-full appearance-none cursor-pointer accent-primary hover:accent-primary/80 transition-all"
        />
        {modPos !== undefined && (
          <div 
            data-testid="mod-indicator"
            className="absolute top-1/2 -translate-y-1/2 h-3 w-1 bg-primary/40 rounded-full shadow-[0_0_8px_var(--color-primary)] transition-all duration-75 pointer-events-none border border-primary/20"
            style={{ left: `${Math.max(0, Math.min(100, modPos))}%`, transform: 'translate(-50%, -50%)' }}
          />
        )}
      </div>
    </div>
  )
}

export interface ModSlotData {
  source: string;
  depth: f32;
}

type f32 = number;

export function ParamSlider({ 
  label, value, min, max, step, onChange, format, 
  mods = [], onModChange,
  modValue
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
  modValue?: number
}) {
  return (
    <div className="space-y-4">
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
      
      {mods.length > 0 && (
        <div className="flex gap-4 items-end pl-2 border-l-2 border-primary/20">
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

export function ModSlot({ source, depth, onChange }: { 
  source: string, 
  depth: number, 
  onChange: (source: string, depth: number) => void 
}) {
  const sources = ['None', 'Envelope', 'Lfo1', 'Lfo2', 'Velocity'];
  
  const cycleSource = () => {
    const currentIndex = sources.indexOf(source);
    const nextIndex = (currentIndex + 1) % sources.length;
    onChange(sources[nextIndex], depth);
  };

  const sourceLabel = source === 'Envelope' ? 'Env' : 
                     source === 'Lfo1' ? 'LFO 1' :
                     source === 'Lfo2' ? 'LFO 2' :
                     source === 'Velocity' ? 'Vel' : '---';

  return (
    <div className="flex flex-col gap-1 items-center group">
      <button 
        onClick={cycleSource}
        className={cn(
          "text-[9px] font-black uppercase tracking-tighter transition-all px-1.5 py-0.5 rounded border",
          source !== 'None' 
            ? "bg-primary/10 border-primary/30 text-primary" 
            : "bg-muted border-transparent text-muted-foreground hover:border-border"
        )}
      >
        {sourceLabel}
      </button>
      <div className="h-20 w-1.5 bg-muted rounded-full relative overflow-hidden flex items-end cursor-ns-resize mt-1">
         <input 
          type="range" 
          aria-label="depth"
          min="-1" max="1" step="0.01"
          value={depth}
          onChange={(e) => onChange(source, parseFloat(e.target.value))}
          className="absolute inset-0 w-full h-full opacity-0 cursor-ns-resize z-10"
          style={{ appearance: 'slider-vertical' as any }}
        />
        <div 
          className={cn(
            "w-full transition-all duration-300",
            source === 'None' ? "bg-muted-foreground/20" :
            depth >= 0 ? "bg-primary" : "bg-destructive"
          )}
          style={{ height: `${Math.abs(depth) * 50}%`, bottom: depth >= 0 ? '50%' : 'auto', top: depth < 0 ? '50%' : 'auto', position: 'absolute' }}
        />
        <div className="absolute top-1/2 left-0 w-full h-[1px] bg-border/50" />
      </div>
    </div>
  )
}
