import { useEffect, useState, useRef } from 'react';
import { cn } from './ui';

export function MasterPeakMeter({ peak, className }: { peak: number, className?: string }) {
  const [level, setLevel] = useState(0);
  const requestRef = useRef<number | null>(null);
  const levelRef = useRef(0);

  // Keep ref synchronized with state to read inside callback without stale closures
  useEffect(() => {
    levelRef.current = level;
  }, [level]);

  // Self-terminating animation frame loop
  const animate = () => {
    const next = levelRef.current * 0.93; // Graceful decay multiplier
    if (next < 0.005) {
      setLevel(0);
      requestRef.current = null; // Mark loop as idle
    } else {
      setLevel(next);
      requestRef.current = requestAnimationFrame(animate);
    }
  };

  // Trigger attack instantly on new peak and boot animation if idle
  useEffect(() => {
    if (peak > levelRef.current) {
      setLevel(peak);
      if (requestRef.current === null) {
        requestRef.current = requestAnimationFrame(animate);
      }
    }
  }, [peak]);

  // Prevent memory leaks on component unmount
  useEffect(() => {
    return () => {
      if (requestRef.current !== null) {
        cancelAnimationFrame(requestRef.current);
      }
    };
  }, []);

  const percentage = Math.min(100, Math.max(0, level * 100));
  const isClipping = level >= 0.99;

  return (
    <div className={cn("flex flex-col gap-2 w-full group", className)}>
      <div className="flex justify-between items-center px-1">
        <span className="text-[10px] font-black text-muted-foreground uppercase tracking-[0.2em]">Peak Level</span>
        <span className={`text-[10px] font-bold transition-colors duration-75 ${isClipping ? 'text-rose-400 font-black animate-pulse' : 'text-muted-foreground'}`}>
          {isClipping ? 'OVER' : `${(level * 100).toFixed(1)}%`}
        </span>
      </div>
      <div className="h-6 bg-zinc-900/50 rounded-lg overflow-hidden border border-white/10 relative shadow-inner">
        <div 
          className={`h-full transition-all duration-75 ease-out ${isClipping ? 'bg-gradient-to-r from-emerald-500 via-rose-500 to-rose-600 shadow-[0_0_20px_rgba(225,29,72,0.4)]' : 'bg-gradient-to-r from-emerald-500 via-emerald-400 to-amber-400 shadow-[0_0_15px_rgba(16,185,129,0.2)]'}`}
          style={{ width: `${percentage}%` }}
        />
        {/* Graticule Markers */}
        <div className="absolute inset-0 flex justify-between px-1 pointer-events-none opacity-30">
          {[...Array(20)].map((_, i) => (
            <div key={i} className="w-[1px] h-full bg-white/10" />
          ))}
        </div>
      </div>
    </div>
  );
}
