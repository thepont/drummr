import { useState, useEffect, useMemo } from 'react'
import { Play, FloppyDisk, Sparkle, Waves, Sliders as SlidersIcon, Clock, Cpu } from "@phosphor-icons/react"
import { cn, ParamController, Button, Card, FrequencyVisualizer } from '../components/ui'
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
    <div className="space-y-6 animate-in fade-in slide-in-from-bottom-4 duration-500 pb-20">
      <header className="flex flex-col md:flex-row md:items-center justify-between gap-4">
        <div>
          <h3 className="text-2xl font-bold tracking-tight">Sound Designer</h3>
          <p className="text-muted-foreground mt-1">Horizontal signal flow: Source → Shape → Timbre → Mod</p>
        </div>
        
        <div className="flex gap-2">
           <Button 
            onClick={triggerPreview} 
            variant="primary" 
            icon={<Play weight="fill" />}
           >
            Preview
           </Button>
        </div>
      </header>

      {/* Sound Selector - Horizontal Strip */}
      <div className="flex gap-2 overflow-x-auto pb-4 no-scrollbar">
        {sounds.map(sound => (
          <button
            key={sound.id}
            onClick={() => setSelectedSoundId(sound.id)}
            className={cn(
              "flex-shrink-0 px-6 py-3 rounded-2xl transition-all border flex items-center gap-3",
              selectedSoundId === sound.id 
                ? "bg-primary text-primary-foreground shadow-lg border-primary" 
                : "bg-card/30 border-border hover:border-primary/50"
            )}
          >
            <span className="font-bold text-xs uppercase tracking-widest">{sound.name}</span>
            {selectedSoundId === sound.id && <Sparkle size={14} weight="fill" />}
          </button>
        ))}
      </div>

      {selectedSound ? (
        <div className="grid grid-cols-1 xl:grid-cols-4 gap-6 items-stretch">
          
          {/* Module 1: Source */}
          <section className="bg-card/30 border border-border rounded-3xl p-6 space-y-6 flex flex-col">
            <header className="flex items-center gap-2 text-[10px] font-black text-primary uppercase tracking-[0.2em]">
              <Cpu size={16} />
              1. Source
            </header>
            
            <div className="space-y-4">
              <div className="grid grid-cols-2 gap-2">
                {['fm', 'phys', 'granular', 'hybrid'].map(type => (
                  <button
                    key={type}
                    onClick={() => updateParam('engine_type' as any, type as any)}
                    className={cn(
                      "px-3 py-2 rounded-xl text-[10px] font-black transition-all uppercase tracking-widest border",
                      selectedSound.engine_type === type 
                        ? "bg-primary border-primary text-primary-foreground" 
                        : "bg-background/50 border-border text-muted-foreground hover:text-foreground"
                    )}
                  >
                    {type}
                  </button>
                ))}
              </div>

              <div className="pt-4">
                <FrequencyVisualizer 
                  value={selectedSound.freq} 
                  min={20} 
                  max={2000} 
                  onChange={v => updateParam('freq', v)} 
                  modValue={getModulatedValue('freq', selectedSound.freq)}
                />
              </div>
            </div>

            <div className="mt-auto pt-6 border-t border-border/50">
               <div className="text-[9px] font-bold text-muted-foreground italic">
                 The raw sound generation engine. Choose the core synthesis method.
               </div>
            </div>
          </section>

          {/* Module 2: Shape */}
          <section className="bg-card/30 border border-border rounded-3xl p-6 space-y-6 flex flex-col xl:col-span-1">
            <header className="flex items-center gap-2 text-[10px] font-black text-primary uppercase tracking-[0.2em]">
              <Clock size={16} />
              2. Shape
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

            <div className="grid grid-cols-2 gap-4">
               <div className="p-3 bg-background/30 rounded-xl border border-border/50">
                 <div className="text-[8px] font-black text-muted-foreground uppercase mb-1">Attack</div>
                 <div className="text-xs font-bold">{selectedSound.attack.toFixed(0)}ms</div>
               </div>
               <div className="p-3 bg-background/30 rounded-xl border border-border/50">
                 <div className="text-[8px] font-black text-muted-foreground uppercase mb-1">Decay</div>
                 <div className="text-xs font-bold">{selectedSound.decay.toFixed(0)}ms</div>
               </div>
            </div>
          </section>

          {/* Module 3: Timbre */}
          <section className="bg-card/30 border border-border rounded-3xl p-6 space-y-6 flex flex-col xl:col-span-1">
            <header className="flex items-center gap-2 text-[10px] font-black text-primary uppercase tracking-[0.2em]">
              <SlidersIcon size={16} />
              3. Timbre
            </header>
            
            <div className="space-y-8 max-h-[400px] overflow-y-auto pr-2 custom-scrollbar">
              {schemas[selectedSound.id]?.filter(p => !['freq', 'attack', 'decay'].includes(p.name)).map((param) => {
                const paramMods = selectedSound.mods?.filter(m => m.param === param.name) || [];
                const displayMods = [...paramMods];
                while (displayMods.length < 1) {
                  displayMods.push({ param: param.name, source: 'None', depth: 0 });
                }

                return (
                  <div key={param.name}>
                    <ParamController 
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

          {/* Module 4: Modulation */}
          <section className="xl:col-span-1">
             <ModulationPanel 
                lfo1_freq={selectedSound.lfo1_freq || 1.0}
                lfo2_freq={selectedSound.lfo2_freq || 1.0}
                onChangeLfo={updateLfo}
                modValues={selectedSlotIndex !== -1 ? modStates[selectedSlotIndex] : undefined}
              />
          </section>

        </div>
      ) : (
        <div className="h-[400px] flex items-center justify-center border-2 border-dashed border-border rounded-3xl text-muted-foreground italic">
          Select a sound to start designing
        </div>
      )}

      {/* Preset Footer */}
      <footer className="fixed bottom-0 left-0 lg:left-64 right-0 bg-background/80 backdrop-blur-xl border-t border-border p-4 px-8 flex items-center justify-between z-20">
         <div className="flex items-center gap-6 overflow-x-auto no-scrollbar max-w-[60%]">
            <div className="text-[10px] font-black text-muted-foreground uppercase tracking-widest whitespace-nowrap">Presets</div>
            {soundPresets.map(preset => (
              <button
                key={preset}
                onClick={() => {
                  if (selectedSoundId && ws) {
                    ws.send(`LOAD_SOUND_PRESET:${preset}:${selectedSoundId}`);
                  }
                }}
                className="text-[10px] font-bold text-muted-foreground hover:text-primary transition-colors whitespace-nowrap"
              >
                {preset}
              </button>
            ))}
         </div>

         <div className="flex gap-2">
            <input 
              type="text" 
              placeholder="Save as..." 
              className="bg-muted/50 border border-border rounded-lg px-3 py-1.5 text-xs outline-none focus:border-primary/50 transition-colors w-32"
              value={newPresetName}
              onChange={e => setNewPresetName(e.target.value)}
            />
            <Button 
              variant="secondary" 
              className="h-8 px-4 text-[10px]"
              onClick={() => {
                if (newPresetName && selectedSoundId && ws) {
                  ws.send(`SAVE_SOUND_PRESET:${newPresetName}:${selectedSoundId}`);
                  setNewPresetName("");
                }
              }}
            >
              Save
            </Button>
         </div>
      </footer>
    </div>
  )
}
