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

export function Slider({ label, value, min, max, step, onChange, format = (v: number) => v.toFixed(2) }: { 
  label: string, 
  value: number, 
  min: number, 
  max: number, 
  step: number, 
  onChange: (v: number) => void,
  format?: (v: number) => string
}) {
  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <span className="text-sm font-medium text-muted-foreground">{label}</span>
        <span className="text-sm font-mono font-bold bg-muted px-2 py-0.5 rounded">{format(value)}</span>
      </div>
      <input 
        type="range" 
        min={min} max={max} step={step}
        value={value}
        onChange={(e) => onChange(parseFloat(e.target.value))}
        className="w-full h-1.5 bg-muted rounded-full appearance-none cursor-pointer accent-primary hover:accent-primary/80 transition-all"
      />
    </div>
  )
}
