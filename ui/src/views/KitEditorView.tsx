import { useState, useEffect, useMemo } from 'react'
import { Play, FloppyDisk, Sparkle, Waves, Sliders as SlidersIcon, Clock } from "@phosphor-icons/react"
import { cn, ParamSlider, Button, Card } from '../components/ui'
import { EnvelopeEditor } from '../components/EnvelopeEditor'
import { ModulationPanel } from '../components/ModulationPanel'

interface ParamSchema {
  name: string;
  min: number;
  max: number;
  default: number;
  unit: string;
}

interface ModSlotData {
  source: string;
  depth: number;
}

interface ModEntry extends ModSlotData {
  param: string;
}

interface Sound {
  id: string;
  name: string;
  engine_type: string;
  mods?: ModEntry[];
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
  const [modStates, setModStates] = useState<number[][]>([]);

  const selectedSound = useMemo(() => 
    sounds.find(s => s.id === selectedSoundId), 
  [sounds, selectedSoundId]);

  const selectedSlotIndex = useMemo(() => 
    sounds.findIndex(s => s.id === selectedSoundId),
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
      } else if (data.startsWith('MOD_STATES:')) {
        try {
          const states = JSON.parse(data.replace('MOD_STATES:', ''));
          setModStates(states);
        } catch (e) {}
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

  const updateMod = (param: string, index: number, source: string, depth: number) => {
    if (!selectedSoundId || !ws) return;

    setSounds(prev => prev.map(s => {
      if (s.id !== selectedSoundId) return s;
      const mods = [...(s.mods || [])];
      
      // We need to find the specific mod for this param and index
      // For simplicity, we assume 2 slots per param. 
      // But the backend uses a Vec<ModEntry> which is more flexible.
      // Let's filter by param and take the index-th one.
      const paramMods = mods.filter(m => m.param === param);
      const modToUpdate = paramMods[index];

      if (modToUpdate) {
        modToUpdate.source = source;
        modToUpdate.depth = depth;
      } else {
        mods.push({ param, source, depth });
      }

      return { ...s, mods };
    }));

    ws.send(`SET_MOD:${selectedSoundId}:${param}:${source}:${depth}`);
  };

  const updateLfo = (index: number, freq: number) => {
    if (!selectedSoundId || !ws) return;

    setSounds(prev => prev.map(s => 
      s.id === selectedSoundId ? { ...s, [`lfo${index}_freq`]: freq } : s
    ));

    ws.send(`SET_LFO:${selectedSoundId}:${index}:${freq}`);
  };

  const triggerPreview = () => {
    if (selectedSoundId && ws) {
      ws.send(`TEST_TRIGGER:${selectedSoundId}`);
    }
  };

  const getModulatedValue = (paramName: string, baseValue: number) => {
    if (selectedSlotIndex === -1 || !modStates[selectedSlotIndex] || !selectedSound) return undefined;
    const currentMods = selectedSound.mods?.filter(m => m.param === paramName) || [];
    let totalMod = 0;
    currentMods.forEach(m => {
      const srcIdx = m.source === 'Envelope' ? 0 : 
                     m.source === 'Lfo1' ? 1 :
                     m.source === 'Lfo2' ? 2 :
                     m.source === 'Velocity' ? 3 : -1;
      if (srcIdx !== -1) {
        totalMod += modStates[selectedSlotIndex][srcIdx] * m.depth;
      }
    });
    // For many params, totalMod is just a linear offset.
    // For some like freq, it might be more complex, but we'll stick to linear for now.
    return baseValue + totalMod;
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
                    <EnvelopeEditor 
                      attack={selectedSound.attack} 
                      decay={selectedSound.decay}
                      onChange={(a, d) => {
                        updateParam('attack', a);
                        updateParam('decay', d);
                      }}
                    />
                  </div>
                </section>

                <ModulationPanel 
                  lfo1_freq={selectedSound.lfo1_freq || 1.0}
                  lfo2_freq={selectedSound.lfo2_freq || 1.0}
                  onChangeLfo={updateLfo}
                  modValues={selectedSlotIndex !== -1 ? modStates[selectedSlotIndex] : undefined}
                />
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
                    const paramMods = selectedSound.mods?.filter(m => m.param === param.name) || [];
                    const displayMods = [...paramMods];
                    while (displayMods.length < 1) { // Show at least 1 slot for now
                      displayMods.push({ param: param.name, source: 'None', depth: 0 });
                    }

                    return (
                      <div key={param.name} className="space-y-8">
                        <div className="text-[10px] font-black text-primary uppercase tracking-[0.2em]">
                          {param.name.replace('_', ' ')}
                        </div>
                        <ParamSlider 
                          label={param.name.charAt(0).toUpperCase() + param.name.slice(1).replace('_', ' ')} 
                          value={selectedSound[param.name] ?? param.default} 
                          min={param.min} 
                          max={param.max} 
                          step={param.max - param.min > 10 ? 1 : 0.01}
                          format={v => param.unit ? `${v.toFixed(param.unit === 'Hz' ? 0 : 2)} ${param.unit}` : v.toFixed(2)}
                          onChange={v => updateParam(param.name as any, v)} 
                          mods={displayMods}
                          onModChange={(idx, source, depth) => updateMod(param.name, idx, source, depth)}
                          modValue={getModulatedValue(param.name, selectedSound[param.name] ?? param.default)}
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
