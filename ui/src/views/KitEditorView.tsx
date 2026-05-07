import { useState, useEffect, useMemo } from 'react'
import { Play, FloppyDisk, Sparkle, Waves, Sliders as SlidersIcon, Clock } from "@phosphor-icons/react"
import { cn, Slider, Button, Card } from '../components/ui'

interface Sound {
  id: string;
  name: string;
  engine_type: string;
  freq: number;
  mod_ratio: number;
  mod_index: number;
  noise_level: number;
  brightness: number;
  dampening: number;
  attack: number;
  decay: number;
}

export default function KitEditorView({ ws }: { ws: WebSocket | null }) {
  const [sounds, setSounds] = useState<Sound[]>([]);
  const [selectedSoundId, setSelectedSoundId] = useState<string | null>(null);

  const selectedSound = useMemo(() => 
    sounds.find(s => s.id === selectedSoundId), 
  [sounds, selectedSoundId]);

  useEffect(() => {
    if (!ws) return;
    ws.send('GET_KIT');

    const handleMessage = (event: MessageEvent) => {
      const data = event.data as string;
      if (data.startsWith('KIT: ')) {
        try {
          const kit = JSON.parse(data.replace('KIT: ', '')) as Sound[];
          setSounds(kit);
          if (kit.length > 0 && !selectedSoundId) {
            setSelectedSoundId(kit[0].id);
          }
        } catch (e) {
          console.error('Failed to parse kit:', e);
        }
      }
    };

    ws.addEventListener('message', handleMessage);
    return () => ws.removeEventListener('message', handleMessage);
  }, [ws]);

  const updateParam = (param: keyof Sound, value: number) => {
    if (!selectedSoundId || !ws) return;
    
    setSounds(prev => prev.map(s => 
      s.id === selectedSoundId ? { ...s, [param]: value } : s
    ));

    ws.send(`SET_PARAM:${selectedSoundId}:${param}:${value}`);
  };

  const triggerPreview = () => {
    if (selectedSoundId && ws) {
      ws.send(`TEST_TRIGGER:${selectedSoundId}`);
    }
  };

  return (
    <div className="space-y-8 animate-in fade-in slide-in-from-bottom-4 duration-500 pb-20">
      <header className="flex flex-col md:flex-row md:items-center justify-between gap-4">
        <div>
          <h3 className="text-2xl font-bold tracking-tight">Kit Editor</h3>
          <p className="text-muted-foreground mt-1">Sculpt your sounds in real-time.</p>
        </div>
        
        <div className="flex gap-2">
           <Button 
            onClick={triggerPreview} 
            variant="primary" 
            icon={<Play weight="fill" />}
           >
            Preview Sound
           </Button>
        </div>
      </header>

      <div className="grid grid-cols-1 lg:grid-cols-12 gap-8">
        {/* Sound List */}
        <aside className="lg:col-span-3 space-y-2">
          <div className="text-[10px] font-bold text-muted-foreground uppercase tracking-widest px-2 mb-2">Sounds</div>
          {sounds.map(sound => (
            <button
              key={sound.id}
              onClick={() => setSelectedSoundId(sound.id)}
              className={cn(
                "w-full text-left px-4 py-3 rounded-xl transition-all flex items-center justify-between group",
                selectedSoundId === sound.id 
                  ? "bg-primary text-primary-foreground shadow-lg shadow-primary/20" 
                  : "bg-card/30 border border-border hover:border-primary/50"
              )}
            >
              <span className="font-medium text-sm">{sound.name}</span>
              <Sparkle 
                size={14} 
                weight={selectedSoundId === sound.id ? "fill" : "regular"}
                className={selectedSoundId === sound.id ? "text-primary-foreground" : "text-muted-foreground opacity-0 group-hover:opacity-100"} 
              />
            </button>
          ))}
        </aside>

        {/* Editor Area */}
        <main className="lg:col-span-9 space-y-8">
          {selectedSound ? (
            <>
              {/* Visualizer Row */}
              <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                <section className="bg-card/30 border border-border rounded-3xl p-6 flex flex-col gap-4">
                  <header className="flex items-center gap-2 text-sm font-bold text-muted-foreground uppercase tracking-wider">
                    <Waves size={18} />
                    Envelope
                  </header>
                  <div className="flex-1 min-h-[200px] bg-background/50 rounded-2xl relative overflow-hidden border border-border/50">
                    <InteractiveEnvelope 
                      attack={selectedSound.attack} 
                      decay={selectedSound.decay}
                      onChange={(a, d) => {
                        updateParam('attack', a);
                        updateParam('decay', d);
                      }}
                    />
                  </div>
                </section>

                <div className="grid grid-cols-1 gap-4">
                  <Card title="Oscillator" value={`${selectedSound.freq.toFixed(1)} Hz`} icon={<SlidersIcon />} />
                  <Card title="Modulation" value={`x${selectedSound.mod_ratio.toFixed(2)}`} icon={<Waves />} />
                  <Card title="Envelope" value={`${(selectedSound.attack + selectedSound.decay).toFixed(0)} ms`} icon={<Clock />} />
                </div>
              </div>

              {/* Controls */}
              <section className="bg-card/30 border border-border rounded-3xl p-8 space-y-10">
                <div className="flex items-center gap-4 mb-4">
                  <div className="text-[10px] font-black text-primary uppercase tracking-[0.2em]">Engine Type</div>
                  <div className="flex bg-background/50 p-1 rounded-xl border border-border/50">
                    {['fm', 'phys'].map(type => (
                      <button
                        key={type}
                        onClick={() => updateParam('engine_type' as any, type as any)}
                        className={cn(
                          "px-4 py-1.5 rounded-lg text-xs font-bold transition-all uppercase tracking-wider",
                          selectedSound.engine_type === type 
                            ? "bg-primary text-primary-foreground shadow-sm" 
                            : "text-muted-foreground hover:text-foreground"
                        )}
                      >
                        {type}
                      </button>
                    ))}
                  </div>
                </div>

                <div className="grid grid-cols-1 md:grid-cols-2 gap-x-12 gap-y-10">
                  <div className="space-y-8">
                    <div className="text-[10px] font-black text-primary uppercase tracking-[0.2em]">Core Settings</div>
                    <Slider 
                      label="Base Frequency" 
                      value={selectedSound.freq} 
                      min={20} max={2000} step={1}
                      format={v => `${v.toFixed(0)} Hz`}
                      onChange={v => updateParam('freq', v)} 
                    />
                  </div>
                  
                  <div className="space-y-8">
                    {selectedSound.engine_type === 'phys' ? (
                      <>
                        <div className="text-[10px] font-black text-primary uppercase tracking-[0.2em]">Physical Model</div>
                        <Slider 
                          label="Brightness (b)" 
                          value={selectedSound.brightness} 
                          min={0} max={1.0} step={0.01}
                          format={v => `${(v * 100).toFixed(0)}%`}
                          onChange={v => updateParam('brightness', v)} 
                        />
                        <Slider 
                          label="Dampening" 
                          value={selectedSound.dampening} 
                          min={0} max={1.0} step={0.01}
                          format={v => `${(v * 100).toFixed(0)}%`}
                          onChange={v => updateParam('dampening', v)} 
                        />
                      </>
                    ) : (
                      <>
                        <div className="text-[10px] font-black text-primary uppercase tracking-[0.2em]">FM Modulation</div>
                        <Slider 
                          label="Mod Ratio" 
                          value={selectedSound.mod_ratio} 
                          min={0.1} max={10.0} step={0.01}
                          format={v => `x${v.toFixed(2)}`}
                          onChange={v => updateParam('mod_ratio', v)} 
                        />
                        <Slider 
                          label="Mod Index" 
                          value={selectedSound.mod_index} 
                          min={0} max={50} step={0.1}
                          onChange={v => updateParam('mod_index', v)} 
                        />
                        <Slider 
                          label="Sizzle (Noise)" 
                          value={selectedSound.noise_level || 0} 
                          min={0} max={1.0} step={0.01}
                          format={v => `${(v * 100).toFixed(0)}%`}
                          onChange={v => updateParam('noise_level', v)} 
                        />
                      </>
                    )}
                  </div>
                </div>
              </section>
            </>
          ) : (
            <div className="h-[400px] flex items-center justify-center border-2 border-dashed border-border rounded-3xl text-muted-foreground italic">
              Select a sound to start editing
            </div>
          )}
        </main>
      </div>
    </div>
  )
}

function InteractiveEnvelope({ attack, decay, onChange }: { attack: number, decay: number, onChange: (a: number, d: number) => void }) {
  // Constants for visualization
  const width = 400;
  const height = 200;
  const padding = 20;
  
  // Scaling factors (visual to ms)
  const maxMs = 2000;
  
  const handleMouseDown = (e: React.MouseEvent<SVGSVGElement>) => {
    const svg = e.currentTarget;
    const updatePosition = (moveEvent: MouseEvent) => {
      const rect = svg.getBoundingClientRect();
      const x = Math.max(0, Math.min(width, moveEvent.clientX - rect.left));
      
      // We'll treat the peak point as the target
      // Attack determines X of peak. Decay is total length - attack.
      const totalMs = (x / width) * maxMs;
      const newAttack = Math.max(1, Math.min(totalMs, 1000)); // Cap attack at 1s for usability
      const newDecay = Math.max(1, totalMs - newAttack);
      
      onChange(newAttack, newDecay);
    };

    const handleMouseUp = () => {
      window.removeEventListener('mousemove', updatePosition);
      window.removeEventListener('mouseup', handleMouseUp);
    };

    window.addEventListener('mousemove', updatePosition);
    window.addEventListener('mouseup', handleMouseUp);
  };

  // Convert ms to visual coords
  const attackX = (attack / maxMs) * width;
  const decayX = ((attack + decay) / maxMs) * width;
  
  const points = `0,${height} ${attackX},${padding} ${decayX},${height}`;

  return (
    <div className="w-full h-full flex flex-col">
      <svg 
        viewBox={`0 0 ${width} ${height}`} 
        className="w-full h-full cursor-crosshair touch-none"
        onMouseDown={handleMouseDown}
      >
        <defs>
          <linearGradient id="envGradient" x1="0%" y1="0%" x2="0%" y2="100%">
            <stop offset="0%" stopColor="var(--color-primary)" stopOpacity="0.4" />
            <stop offset="100%" stopColor="var(--color-primary)" stopOpacity="0" />
          </linearGradient>
        </defs>
        
        {/* Grid lines */}
        <line x1="0" y1={height/2} x2={width} y2={height/2} stroke="var(--color-border)" strokeDasharray="4" />
        <line x1={width/2} y1="0" x2={width/2} y2={height} stroke="var(--color-border)" strokeDasharray="4" />

        {/* The Shape */}
        <polyline
          points={points}
          fill="url(#envGradient)"
          stroke="var(--color-primary)"
          strokeWidth="3"
          strokeLinejoin="round"
        />

        {/* Draggable handle at peak */}
        <circle 
          cx={attackX} 
          cy={padding} 
          r="6" 
          fill="var(--color-primary-foreground)" 
          stroke="var(--color-primary)" 
          strokeWidth="3"
          className="drop-shadow-lg"
        />
        
        {/* Labels */}
        <text x={attackX/2} y={height - 10} fontSize="10" fill="var(--color-muted-foreground)" textAnchor="middle">A: {attack.toFixed(0)}ms</text>
        <text x={attackX + decay/2} y={height - 10} fontSize="10" fill="var(--color-muted-foreground)" textAnchor="middle">D: {decay.toFixed(0)}ms</text>
      </svg>
    </div>
  );
}
