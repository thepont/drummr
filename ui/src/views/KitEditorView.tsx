import { useState, useEffect, useCallback, useRef } from 'react'
import { Play, FloppyDisk, Sparkle, Waveform, SquaresFour, X } from "@phosphor-icons/react"
import { clsx, type ClassValue } from 'clsx'
import { twMerge } from 'tailwind-merge'

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

interface SoundParams {
  id: string;
  name: string;
  freq: number;
  mod_ratio: number;
  mod_index: number;
  attack: number;
  decay: number;
}

const PRESETS: Partial<SoundParams>[] = [
  { name: 'Deep 808 Kick', freq: 45, mod_ratio: 0.5, mod_index: 2.0, attack: 0.005, decay: 0.8 },
  { name: 'Snappy Snare', freq: 180, mod_ratio: 1.5, mod_index: 15.0, attack: 0.001, decay: 0.2 },
  { name: 'Laser Tom', freq: 120, mod_ratio: 0.8, mod_index: 25.0, attack: 0.01, decay: 0.4 },
  { name: 'Space Hat', freq: 600, mod_ratio: 4.2, mod_index: 40.0, attack: 0.001, decay: 0.05 },
  { name: 'Industrial Clang', freq: 80, mod_ratio: 2.7, mod_index: 45.0, attack: 0.002, decay: 0.6 },
  { name: 'Soft Pulse', freq: 60, mod_ratio: 1.0, mod_index: 0.0, attack: 0.05, decay: 1.2 },
];

export default function KitEditorView({ ws }: { ws: WebSocket | null }) {
  const [sounds, setSounds] = useState<SoundParams[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [showGallery, setShowGallery] = useState(false);

  const selectedSound = sounds.find(s => s.id === selectedId);

  const updateParam = useCallback((id: string, param: keyof SoundParams, value: any) => {
    setSounds(prev => prev.map(s => s.id === id ? { ...s, [param]: value } : s));
    if (ws) {
      ws.send(`SET_PARAM:${id}:${param}:${value}`);
    }
  }, [ws]);

  const applyPreset = (preset: Partial<SoundParams>) => {
    if (!selectedId) return;
    Object.entries(preset).forEach(([key, value]) => {
      if (key !== 'id') {
        updateParam(selectedId, key as keyof SoundParams, value);
      }
    });
    setShowGallery(false);
  };

  const handleTestTrigger = (id: string) => {
    if (ws) {
      ws.send(`TEST_TRIGGER:${id}`);
    }
  };

  useEffect(() => {
    if (!ws) return;

    ws.send('GET_KIT');

    const handleMessage = (event: MessageEvent) => {
      const data = event.data as string;
      if (data.startsWith('KIT: ')) {
        try {
          const kit = JSON.parse(data.replace('KIT: ', '')) as SoundParams[];
          setSounds(kit);
          if (kit.length > 0 && !selectedId) {
            setSelectedId(kit[0].id);
          }
        } catch (e) {
          console.error('Failed to parse kit:', e);
        }
      }
    };

    ws.addEventListener('message', handleMessage);
    return () => ws.removeEventListener('message', handleMessage);
  }, [ws, selectedId]);

  return (
    <div className="flex gap-8 h-[calc(100vh-12rem)] animate-in fade-in slide-in-from-right-4 duration-500 relative">
      {/* Sound List */}
      <div className="w-64 flex flex-col gap-2 overflow-y-auto pr-2">
        <h3 className="text-xs font-bold uppercase tracking-widest text-muted-foreground mb-2 px-4">Sounds</h3>
        {sounds.map(sound => (
          <button
            key={sound.id}
            onClick={() => setSelectedId(sound.id)}
            className={cn(
              "flex items-center justify-between px-4 py-3 rounded-xl text-sm font-medium transition-all group",
              selectedId === sound.id 
                ? "bg-primary text-primary-foreground shadow-lg shadow-primary/20" 
                : "bg-card/50 text-muted-foreground hover:bg-muted hover:text-foreground border border-border"
            )}
          >
            <span>{sound.name}</span>
            <button 
              onClick={(e) => { e.stopPropagation(); handleTestTrigger(sound.id); }}
              className={cn(
                "p-1.5 rounded-lg transition-colors",
                selectedId === sound.id ? "hover:bg-primary-foreground/20" : "hover:bg-background"
              )}
            >
              <Play weight="fill" size={14} />
            </button>
          </button>
        ))}
      </div>

      {/* Editor Area */}
      <div className="flex-1 bg-card/30 border border-border rounded-3xl p-8 flex flex-col gap-10 overflow-y-auto">
        {selectedSound ? (
          <>
            <header className="flex items-center justify-between">
              <div className="flex items-center gap-4">
                <div className="w-12 h-12 rounded-2xl bg-primary/10 flex items-center justify-center text-primary">
                  <Waveform size={28} weight="duotone" />
                </div>
                <div>
                  <h2 className="text-2xl font-bold tracking-tight">{selectedSound.name}</h2>
                  <p className="text-sm text-muted-foreground">FM Synthesis & Envelope</p>
                </div>
              </div>
              
              <div className="flex gap-3">
                <button 
                  onClick={() => setShowGallery(true)}
                  className="flex items-center gap-2 px-6 py-2 bg-secondary text-secondary-foreground rounded-full font-bold hover:bg-muted transition-all"
                >
                  <SquaresFour size={20} />
                  Presets
                </button>
                <button 
                  className="flex items-center gap-2 px-6 py-2 bg-primary text-primary-foreground rounded-full font-bold hover:scale-105 active:scale-95 transition-all shadow-lg shadow-primary/20"
                  onClick={() => ws?.send(`SAVE_KIT:${JSON.stringify(sounds)}`)}
                >
                  <FloppyDisk size={20} />
                  Save Kit
                </button>
              </div>
            </header>

            <div className="grid grid-cols-1 xl:grid-cols-2 gap-12">
              {/* FM Parameters */}
              <section className="space-y-6">
                <h4 className="flex items-center gap-2 text-sm font-bold uppercase tracking-wider text-muted-foreground">
                  <Sparkle size={16} />
                  FM Synthesis
                </h4>
                
                <div className="space-y-8">
                  <Slider 
                    label="Carrier Freq" 
                    value={selectedSound.freq} 
                    min={20} max={1000} step={1}
                    onChange={(v) => updateParam(selectedSound.id, 'freq', v)} 
                  />
                  <Slider 
                    label="Mod Ratio" 
                    value={selectedSound.mod_ratio} 
                    min={0.1} max={10.0} step={0.01}
                    onChange={(v) => updateParam(selectedSound.id, 'mod_ratio', v)} 
                  />
                  <Slider 
                    label="Mod Index" 
                    value={selectedSound.mod_index} 
                    min={0} max={50.0} step={0.1}
                    onChange={(v) => updateParam(selectedSound.id, 'mod_index', v)} 
                  />
                </div>
              </section>

              {/* Envelope Area */}
              <section className="space-y-6">
                <h4 className="flex items-center gap-2 text-sm font-bold uppercase tracking-wider text-muted-foreground">
                  <Waveform size={16} />
                  AD Envelope
                </h4>
                
                <div className="aspect-video bg-background/50 rounded-2xl border border-border relative overflow-hidden group">
                   <EnvelopeVisual 
                    attack={selectedSound.attack} 
                    decay={selectedSound.decay} 
                    onUpdate={(a, d) => {
                      updateParam(selectedSound.id, 'attack', a);
                      updateParam(selectedSound.id, 'decay', d);
                    }}
                   />
                </div>

                <div className="grid grid-cols-2 gap-4">
                  <Slider 
                    label="Attack" 
                    value={selectedSound.attack} 
                    min={0.001} max={0.5} step={0.001}
                    onChange={(v) => updateParam(selectedSound.id, 'attack', v)} 
                  />
                  <Slider 
                    label="Decay" 
                    value={selectedSound.decay} 
                    min={0.01} max={2.0} step={0.01}
                    onChange={(v) => updateParam(selectedSound.id, 'decay', v)} 
                  />
                </div>
              </section>
            </div>
          </>
        ) : (
          <div className="flex-1 flex flex-col items-center justify-center text-center space-y-4">
            <div className="w-20 h-20 rounded-full bg-muted flex items-center justify-center">
              <Waveform size={40} className="text-muted-foreground" />
            </div>
            <p className="text-muted-foreground">Select a sound to start editing</p>
          </div>
        )}
      </div>

      {/* Preset Gallery Overlay */}
      {showGallery && (
        <div className="absolute inset-0 z-50 flex items-center justify-center p-8 bg-background/80 backdrop-blur-md animate-in fade-in duration-300">
          <div className="bg-card border border-border w-full max-w-2xl rounded-3xl shadow-2xl overflow-hidden flex flex-col max-h-full animate-in zoom-in-95 duration-300">
            <header className="p-6 border-b border-border flex items-center justify-between">
              <div className="flex items-center gap-3 text-primary">
                <SquaresFour size={24} weight="bold" />
                <h3 className="text-xl font-bold">Sound Gallery</h3>
              </div>
              <button 
                onClick={() => setShowGallery(false)}
                className="p-2 hover:bg-muted rounded-full transition-colors"
              >
                <X size={20} />
              </button>
            </header>
            
            <div className="flex-1 overflow-y-auto p-6 grid grid-cols-1 sm:grid-cols-2 gap-4">
              {PRESETS.map((preset, i) => (
                <button
                  key={i}
                  onClick={() => applyPreset(preset)}
                  className="flex flex-col gap-1 p-4 rounded-2xl border border-border bg-background/50 hover:border-primary/50 hover:bg-primary/5 transition-all text-left group"
                >
                  <span className="font-bold group-hover:text-primary">{preset.name}</span>
                  <span className="text-[10px] uppercase tracking-widest text-muted-foreground">FM Preset</span>
                </button>
              ))}
            </div>
          </div>
        </div>
      )}
    </div>
  )
}

function Slider({ label, value, min, max, step, onChange }: { label: string, value: number, min: number, max: number, step: number, onChange: (v: number) => void }) {
  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <span className="text-sm font-medium text-muted-foreground">{label}</span>
        <span className="text-sm font-mono font-bold bg-muted px-2 py-0.5 rounded">{value.toFixed(3)}</span>
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

function EnvelopeVisual({ attack, decay, onUpdate }: { attack: number, decay: number, onUpdate: (a: number, d: number) => void }) {
  const svgRef = useRef<SVGSVGElement>(null);
  const [isDragging, setIsDragging] = useState(false);

  const maxAttack = 0.5;
  const maxDecay = 2.0;
  
  const x = (attack / maxAttack) * 40; 
  const dx = (decay / maxDecay) * 60; 
  const peakX = x;
  const endX = x + dx;

  const handleMouseDown = () => setIsDragging(true);

  useEffect(() => {
    const handleMouseMove = (e: MouseEvent) => {
      if (!isDragging || !svgRef.current) return;
      const rect = svgRef.current.getBoundingClientRect();
      const relativeX = ((e.clientX - rect.left) / rect.width) * 100;
      const newAttack = Math.min(Math.max((relativeX / 40) * maxAttack, 0.001), maxAttack);
      onUpdate(newAttack, decay);
    };

    const handleMouseUp = () => setIsDragging(false);

    if (isDragging) {
      window.addEventListener('mousemove', handleMouseMove);
      window.addEventListener('mouseup', handleMouseUp);
    }

    return () => {
      window.removeEventListener('mousemove', handleMouseMove);
      window.removeEventListener('mouseup', handleMouseUp);
    };
  }, [isDragging, decay, onUpdate]);

  return (
    <svg 
      ref={svgRef}
      className={cn(
        "w-full h-full p-8 select-none touch-none",
        isDragging ? "cursor-grabbing" : "cursor-grab"
      )} 
      viewBox="0 0 100 100" 
      preserveAspectRatio="none"
    >
      <defs>
        <linearGradient id="envelopeGradient" x1="0" y1="1" x2="0" y2="0">
          <stop offset="0%" stopColor="var(--primary)" stopOpacity="0" />
          <stop offset="100%" stopColor="var(--primary)" stopOpacity="0.2" />
        </linearGradient>
      </defs>
      
      <line x1="0" y1="0" x2="100" y2="0" stroke="var(--border)" strokeWidth="0.5" strokeDasharray="2 2" />
      <line x1="0" y1="50" x2="100" y2="50" stroke="var(--border)" strokeWidth="0.5" strokeDasharray="2 2" />
      <line x1="0" y1="100" x2="100" y2="100" stroke="var(--border)" strokeWidth="1" />
      
      <path 
        d={`M 0 100 L ${peakX} 10 L ${endX} 100`} 
        fill="url(#envelopeGradient)"
        stroke="var(--primary)" 
        strokeWidth="2"
        strokeLinejoin="round"
        className="transition-all duration-75 ease-out"
      />
      
      <circle 
        cx={peakX} cy="10" r="4" 
        fill="var(--primary)" 
        onMouseDown={handleMouseDown}
        className={cn(
          "transition-all duration-75 hover:r-6 cursor-grab active:cursor-grabbing shadow-xl",
          isDragging && "fill-emerald-500 r-6"
        )} 
      />
    </svg>
  )
}
