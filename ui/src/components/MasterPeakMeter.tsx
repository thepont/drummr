import { useEffect, useState, useRef } from 'react';

export function MasterPeakMeter({ isActive }: { isActive: boolean }) {
  const [level, setLevel] = useState(0);
  const requestRef = useRef<number | null>(null);

  useEffect(() => {
    if (isActive) {
      setLevel(0.9 + Math.random() * 0.1);
    }
  }, [isActive]);

  const animate = () => {
    setLevel(prev => {
      const decay = 0.93;
      const next = prev * decay;
      return next < 0.01 ? 0 : next;
    });
    requestRef.current = requestAnimationFrame(animate);
  };

  useEffect(() => {
    requestRef.current = requestAnimationFrame(animate);
    return () => {
      if (requestRef.current !== null) cancelAnimationFrame(requestRef.current);
    };
  }, []);

  return (
    <div className="flex flex-col gap-1 w-32 group">
      <div className="flex justify-between items-center px-1">
        <span className="text-[8px] font-black text-muted-foreground uppercase tracking-tighter">Master Peak</span>
        <span className="text-[8px] font-bold text-muted-foreground">{(level * 100).toFixed(0)}%</span>
      </div>
      <div className="h-2 bg-zinc-900 rounded-full overflow-hidden border border-white/5 relative">
        <div 
          className="h-full bg-gradient-to-r from-emerald-500 via-emerald-400 to-amber-400 transition-all duration-75 ease-out"
          style={{ width: `${level * 100}%` }}
        />
        {/* Peak markers */}
        <div className="absolute inset-0 flex justify-between px-1 pointer-events-none opacity-20">
           {[...Array(10)].map((_, i) => (
             <div key={i} className="w-[1px] h-full bg-white/20" />
           ))}
        </div>
      </div>
    </div>
  );
}
