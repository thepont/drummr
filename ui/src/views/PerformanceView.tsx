import { Folder, Pulse, Clock, SpeakerHigh, Cpu } from "@phosphor-icons/react"
import { cn } from '../components/ui'
import { MasterPeakMeter } from '../components/MasterPeakMeter'

interface PerformanceViewProps {
  ws: WebSocket | null;
  activeKitName: string;
  availableKits: string[];
  bpm: string;
  masterPeak: number;
  isMidiActive: boolean;
  syncStatus: string;
  toggleSync: () => void;
}

export default function PerformanceView({
  ws,
  activeKitName,
  availableKits,
  bpm,
  masterPeak,
  isMidiActive,
  syncStatus,
  toggleSync
}: PerformanceViewProps) {
  
  const currentIndex = availableKits.indexOf(activeKitName);
  
  const nextKit = () => {
    if (availableKits.length === 0) return;
    const nextIdx = (currentIndex + 1) % availableKits.length;
    ws?.send(`LOAD_KIT:${availableKits[nextIdx]}`);
  };

  const prevKit = () => {
    if (availableKits.length === 0) return;
    const prevIdx = (currentIndex - 1 + availableKits.length) % availableKits.length;
    ws?.send(`LOAD_KIT:${availableKits[prevIdx]}`);
  };

  // Show a few kits for quick switching
  const quickKits = availableKits.slice(0, 12);

  return (
    <div className="space-y-8 animate-in fade-in duration-500">
      {/* Visual Feedback Area */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        <div className={cn(
          "lg:col-span-2 rounded-[2rem] p-8 flex flex-col justify-center items-center gap-6 transition-all duration-75 border-4 relative overflow-hidden",
          isMidiActive 
            ? "bg-emerald-500/20 border-emerald-500 shadow-[0_0_50px_rgba(16,185,129,0.3)] scale-[1.01]" 
            : "bg-card/40 border-border/50"
        )}>
           <span className="text-[10px] font-black uppercase tracking-[0.5em] text-muted-foreground/60">Active Kit</span>
           
           <div className="flex items-center justify-between w-full gap-4">
              <button 
                onClick={prevKit}
                className="p-4 rounded-full bg-white/5 hover:bg-white/10 transition-colors"
                aria-label="Previous Kit"
              >
                <div className="w-0 h-0 border-t-[10px] border-t-transparent border-r-[15px] border-r-white/50 border-b-[10px] border-b-transparent" />
              </button>

              <h1 className="text-4xl md:text-6xl lg:text-7xl font-black uppercase tracking-tighter text-center flex-1 truncate">
                {activeKitName || "No Kit Loaded"}
              </h1>

              <button 
                onClick={nextKit}
                className="p-4 rounded-full bg-white/5 hover:bg-white/10 transition-colors"
                aria-label="Next Kit"
              >
                <div className="w-0 h-0 border-t-[10px] border-t-transparent border-l-[15px] border-l-white/50 border-b-[10px] border-b-transparent" />
              </button>
           </div>

           <div className="flex items-center gap-4 mt-4">
              <div className={cn(
                "w-4 h-4 rounded-full",
                isMidiActive ? "bg-emerald-400 shadow-[0_0_20px_#34d399]" : "bg-zinc-800"
              )} />
              <span className="text-xs font-bold uppercase tracking-widest text-muted-foreground">MIDI Activity</span>
           </div>
        </div>

        <div className="bg-card/40 border border-border/50 rounded-[2rem] p-8 flex flex-col items-center justify-center gap-2">
           <span className="text-[10px] font-black uppercase tracking-[0.5em] text-muted-foreground/60 mb-2">Engine BPM</span>
           <div className="text-7xl md:text-8xl font-black text-primary tabular-nums tracking-tighter leading-none">
              {parseFloat(bpm) > 0 ? bpm : "---"}
           </div>
           <div className="flex items-center gap-2 mt-4">
              <Clock size={16} className="text-muted-foreground" />
              <span className="text-xs font-bold uppercase tracking-widest text-muted-foreground">Internal Clock</span>
           </div>
        </div>
      </div>

      {/* Main Controls */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
         <button
            onClick={toggleSync}
            className={cn(
              "h-48 rounded-[3rem] text-3xl font-black uppercase tracking-widest transition-all border-4 flex items-center justify-center gap-4",
              syncStatus === "Running" 
                ? "bg-emerald-500 border-emerald-500 text-white shadow-[0_0_60px_rgba(16,185,129,0.5)]" 
                : "bg-background/50 border-border text-muted-foreground hover:text-foreground hover:border-primary/50"
            )}
         >
            <Pulse size={48} weight="fill" className={cn(syncStatus === "Running" && "animate-pulse")} />
            {syncStatus === "Running" ? "Master Running" : "Start Sync"}
         </button>

         <div className="bg-card/40 border border-border/50 rounded-[3rem] p-10 h-48 flex flex-col justify-center gap-6">
            <span className="text-[12px] font-black uppercase tracking-[0.5em] text-muted-foreground/60 text-center">Master Output Level</span>
            <div className="flex-1 flex items-center justify-center">
               <MasterPeakMeter peak={masterPeak} />
            </div>
         </div>
      </div>

      {/* Quick Kit Switcher */}
      <section className="space-y-4">
        <div className="flex items-center justify-between px-2">
           <h3 className="font-black text-xs uppercase tracking-widest text-muted-foreground">Quick Kit Switch</h3>
           <span className="text-[10px] font-bold text-muted-foreground/40">{availableKits.length} Kits Available</span>
        </div>
        <div className="grid grid-cols-2 sm:grid-cols-4 gap-4">
          {quickKits.map(kit => (
            <button
              key={kit}
              onClick={() => ws?.send(`LOAD_KIT:${kit}`)}
              className={cn(
                "p-6 rounded-2xl border-2 transition-all text-left flex flex-col gap-3 group relative overflow-hidden",
                kit === activeKitName 
                  ? "bg-emerald-500/10 border-emerald-500/50 shadow-lg" 
                  : "bg-card/20 border-border hover:border-primary/30 hover:bg-card/40"
              )}
            >
              <Folder size={24} weight={kit === activeKitName ? "fill" : "regular"} className={cn(
                kit === activeKitName ? "text-emerald-400" : "text-muted-foreground group-hover:text-primary"
              )} />
              <span className={cn("font-bold text-sm truncate", kit === activeKitName ? "text-emerald-400" : "text-foreground")}>
                {kit}
              </span>
            </button>
          ))}
        </div>
      </section>

      {/* Stats Cards - Small on mobile */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
         <MiniStat icon={<Cpu size={14} />} label="MIDI" value="DDTi Interface" active={isMidiActive} />
         <MiniStat icon={<SpeakerHigh size={14} />} label="Audio" value="Low Latency" />
         <MiniStat icon={<Pulse size={14} />} label="Jitter" value="0.2ms" />
         <MiniStat icon={<Clock size={14} />} label="Buffer" value="64" />
      </div>
    </div>
  )
}

function MiniStat({ icon, label, value, active }: { icon: any, label: string, value: string, active?: boolean }) {
  return (
    <div className={cn(
      "bg-card/20 border border-border/50 rounded-2xl p-4 transition-colors",
      active && "bg-emerald-500/5 border-emerald-500/30"
    )}>
       <div className="flex items-center gap-2 mb-1">
          <div className={cn("text-muted-foreground", active && "text-emerald-400")}>{icon}</div>
          <span className="text-[10px] font-black uppercase tracking-widest text-muted-foreground/60">{label}</span>
       </div>
       <div className={cn("text-xs font-bold truncate", active && "text-emerald-400")}>{value}</div>
    </div>
  )
}
