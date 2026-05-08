import { useState, useEffect, useMemo } from 'react'
import { Play, FloppyDisk, Sparkle, Waves, Sliders as SlidersIcon, Clock } from "@phosphor-icons/react"
import { cn, Slider, Button, Card } from '../components/ui'

interface ParamSchema {
  name: string;
  min: number;
  max: number;
  default: number;
  unit: string;
}

interface Sound {
  id: string;
  name: string;
  engine_type: string;
  [key: string]: any; // Allow dynamic parameters
}

export default function KitEditorView({ ws }: { ws: WebSocket | null }) {
  const [sounds, setSounds] = useState<Sound[]>([]);
  const [selectedSoundId, setSelectedSoundId] = useState<string | null>(null);
  const [schemas, setSchemas] = useState<Record<string, ParamSchema[]>>({});
  const [soundPresets, setSoundPresets] = useState<string[]>([]);
  const [newPresetName, setNewPresetName] = useState("");
  const [kitList, setKitList] = useState<string[]>([]);
  const [newKitName, setNewKitName] = useState("");

  const selectedSound = useMemo(() => 
    sounds.find(s => s.id === selectedSoundId), 
  [sounds, selectedSoundId]);

  useEffect(() => {
    if (!selectedSoundId || !ws) return;
    ws.send(`GET_SCHEMA:${selectedSoundId}`);
  }, [selectedSoundId, ws, selectedSound?.engine_type]);

  useEffect(() => {
    if (!ws) return;
    ws.send('GET_KIT');
    ws.send('LIST_SOUND_PRESETS');
    ws.send('LIST_KITS');

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
      } else if (data.startsWith('SOUND_PRESETS:')) {
        const list = data.replace('SOUND_PRESETS:', '');
        setSoundPresets(list ? list.split(',') : []);
      } else if (data.startsWith('KIT_LIST:')) {
        const list = data.replace('KIT_LIST:', '');
        setKitList(list ? list.split(',') : []);
      } else if (data.startsWith('SCHEMA:')) {
        const parts = data.split(':');
        const soundId = parts[1];
        // The JSON starts at the first '[' which is after the soundId
        const jsonStr = data.substring(data.indexOf('[', data.indexOf(soundId)));
        try {
          const schema = JSON.parse(jsonStr) as ParamSchema[];
          setSchemas(prev => ({ ...prev, [soundId]: schema }));
        } catch (e) {
          console.error('Failed to parse schema:', e);
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
        <aside className="lg:col-span-3 space-y-6">
          <div className="space-y-2">
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
          </div>

          <div className="space-y-4 pt-4 border-t border-border/50">
            <div className="text-[10px] font-bold text-muted-foreground uppercase tracking-widest px-2">Sound Library</div>
            
            <div className="px-2 space-y-2">
               <input 
                type="text" 
                placeholder="Preset Name..." 
                className="w-full bg-background/50 border border-border rounded-lg px-3 py-2 text-xs"
                value={newPresetName}
                onChange={e => setNewPresetName(e.target.value)}
              />
              <Button 
                variant="secondary" 
                className="w-full text-[10px] h-8"
                onClick={() => {
                  if (newPresetName && selectedSoundId && ws) {
                    ws.send(`SAVE_SOUND_PRESET:${newPresetName}:${selectedSoundId}`);
                    setNewPresetName("");
                  }
                }}
              >
                Save as Preset
              </Button>
            </div>

            <div className="space-y-1">
              {soundPresets.map(preset => (
                <button
                  key={preset}
                  onClick={() => {
                    if (selectedSoundId && ws) {
                      ws.send(`LOAD_SOUND_PRESET:${preset}:${selectedSoundId}`);
                    }
                  }}
                  className="w-full text-left px-4 py-2 text-xs text-muted-foreground hover:text-foreground hover:bg-card/50 rounded-lg transition-colors"
                >
                  {preset}
                </button>
              ))}
              {soundPresets.length === 0 && (
                <div className="px-4 py-2 text-[10px] text-muted-foreground italic">No presets saved yet</div>
              )}
            </div>
          </div>

          <div className="space-y-4 pt-4 border-t border-border/50">
            <div className="text-[10px] font-bold text-muted-foreground uppercase tracking-widest px-2">Kit Library</div>
            
            <div className="px-2 space-y-2">
               <input 
                type="text" 
                placeholder="New Kit Name..." 
                className="w-full bg-background/50 border border-border rounded-lg px-3 py-2 text-xs"
                value={newKitName}
                onChange={e => setNewKitName(e.target.value)}
              />
              <Button 
                variant="secondary" 
                className="w-full text-[10px] h-8"
                onClick={() => {
                  if (newKitName && ws) {
                    ws.send(`SAVE_KIT_AS:${newKitName}`);
                    setNewKitName("");
                  }
                }}
              >
                Save Kit As
              </Button>
            </div>

            <div className="space-y-1">
              {kitList.map(kit => (
                <button
                  key={kit}
                  onClick={() => {
                    if (ws) {
                      ws.send(`LOAD_KIT:${kit}`);
                    }
                  }}
                  className="w-full text-left px-4 py-2 text-xs text-muted-foreground hover:text-foreground hover:bg-card/50 rounded-lg transition-colors"
                >
                  {kit}
                </button>
              ))}
              {kitList.length === 0 && (
                <div className="px-4 py-2 text-[10px] text-muted-foreground italic">No kits saved yet</div>
              )}
            </div>
          </div>
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
                    {['fm', 'phys', 'granular', 'hybrid'].map(type => (
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
                  {/* Schema-Driven Dynamic Sliders */}
                  {schemas[selectedSound.id]?.map((param, idx) => {
                    // Split parameters into two columns roughly
                    const isLeftColumn = idx < Math.ceil(schemas[selectedSound.id].length / 2);
                    return (
                      <div key={param.name} className="space-y-8">
                        <div className="text-[10px] font-black text-primary uppercase tracking-[0.2em]">
                          {param.name.replace('_', ' ')}
                        </div>
                        <Slider 
                          label={param.name.charAt(0).toUpperCase() + param.name.slice(1).replace('_', ' ')} 
                          value={selectedSound[param.name] ?? param.default} 
                          min={param.min} 
                          max={param.max} 
                          step={param.max - param.min > 10 ? 1 : 0.01}
                          format={v => param.unit ? `${v.toFixed(param.unit === 'Hz' ? 0 : 2)} ${param.unit}` : v.toFixed(2)}
                          onChange={v => updateParam(param.name as any, v)} 
                        />
                      </div>
                    );
                  })}
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
