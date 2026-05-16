import { useState, useEffect, useMemo } from 'react'
import { Play, Sparkle, Sliders as SlidersIcon, Clock, Cpu, ArrowsClockwise, FloppyDisk, Waveform } from "@phosphor-icons/react"
import { cn, ParamController, Button, FrequencyVisualizer, PredictiveGraph, Slider } from '../components/ui'
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
  id: any;
  name: string;
  engine_type: string;
  mods?: ModEntry[];
  [key: string]: any;
}

interface KitEditorProps {
  ws: WebSocket | null;
  sounds: Sound[];
  setSounds: React.Dispatch<React.SetStateAction<Sound[]>>;
  schemas: Record<string, ParamSchema[]>;
  setSchemas: React.Dispatch<React.SetStateAction<Record<string, ParamSchema[]>>>;
  selectedSoundId: any;
  setSelectedSoundId: (id: any) => void;
}

export default function KitEditorView({ 
  ws, sounds, setSounds, schemas, 
  selectedSoundId, setSelectedSoundId 
}: KitEditorProps) {
  const [newKitName, setNewKitName] = useState("");
  const [isSaveKitModalOpen, setIsSaveKitModalOpen] = useState(false);
  const [modStates, setModStates] = useState<number[][]>([]);

  const selectedSound = useMemo(() => 
    sounds.find(s => String(s.id) === String(selectedSoundId)), 
  [sounds, selectedSoundId]);

  const selectedSlotIndex = useMemo(() => 
    sounds.findIndex(s => String(s.id) === String(selectedSoundId)),
  [sounds, selectedSoundId]);

  useEffect(() => {
    if (sounds.length > 0 && selectedSoundId === null) {
      setSelectedSoundId(sounds[0].id);
    }
  }, [sounds, selectedSoundId, setSelectedSoundId]);

  useEffect(() => {
    if (selectedSoundId !== null && ws) {
      ws.send(`GET_SCHEMA:${selectedSoundId}`);
    }
  }, [selectedSoundId, ws, selectedSound?.engine_type]);

  useEffect(() => {
    if (!ws) return;
    
    const handleMessage = (event: MessageEvent) => {
      const data = event.data as string;
      if (data.startsWith('MOD_STATES:')) {
        try {
          const states = JSON.parse(data.replace('MOD_STATES:', ''));
          setModStates(states);
        } catch (e) {}
      }
    };

    ws.addEventListener('message', handleMessage);
    return () => ws.removeEventListener('message', handleMessage);
  }, [ws]);

  const updateParam = (param: keyof Sound, value: number) => {
    if (selectedSoundId === null || !ws) return;
    
    setSounds(prev => prev.map(s => 
      String(s.id) === String(selectedSoundId) ? { ...s, [param]: value } : s
    ));

    ws.send(`SET_PARAM:${selectedSoundId}:${String(param)}:${value}`);
  };

  const updateMod = (param: string, index: number, source: string, depth: number) => {
    if (selectedSoundId === null || !ws) return;

    setSounds(prev => prev.map(s => {
      if (String(s.id) !== String(selectedSoundId)) return s;
      const mods = [...(s.mods || [])];
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
    if (selectedSoundId === null || !ws) return;

    setSounds(prev => prev.map(s => 
      String(s.id) === String(selectedSoundId) ? { ...s, [`lfo${index}_freq`]: freq } : s
    ));

    ws.send(`SET_LFO:${selectedSoundId}:${index}:${freq}`);
  };

  const triggerPreview = () => {
    if (selectedSoundId !== null && ws) {
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
            onClick={() => ws?.send('GET_KIT')} 
            variant="secondary" 
            icon={<ArrowsClockwise />}
           >
            Reload
           </Button>
           <Button 
            onClick={() => setIsSaveKitModalOpen(true)} 
            variant="secondary" 
            icon={<FloppyDisk />}
           >
            Save Kit As
           </Button>
           <Button 
            onClick={triggerPreview} 
            variant="primary" 
            icon={<Play weight="fill" />}
           >
            Preview
           </Button>
        </div>
      </header>

      {isSaveKitModalOpen && (
        <div className="fixed inset-0 z-[100] flex items-center justify-center p-4 bg-background/80 backdrop-blur-sm">
          <div className="bg-card border border-border rounded-3xl p-8 w-full max-w-md shadow-2xl animate-in zoom-in-95 duration-200">
            <h4 className="text-xl font-bold mb-4">Save Kit As</h4>
            <input 
              autoFocus
              type="text" 
              placeholder="Enter kit name..." 
              className="w-full bg-muted border border-border rounded-xl px-4 py-3 text-sm outline-none focus:border-primary transition-colors mb-6"
              value={newKitName}
              onChange={e => setNewKitName(e.target.value)}
              onKeyDown={e => {
                if (e.key === 'Enter' && newKitName) {
                  ws?.send(`SAVE_KIT_AS:${newKitName}`);
                  setIsSaveKitModalOpen(false);
                  setNewKitName("");
                }
                if (e.key === 'Escape') setIsSaveKitModalOpen(false);
              }}
            />
            <div className="flex gap-3 justify-end">
              <Button variant="secondary" onClick={() => setIsSaveKitModalOpen(false)}>Cancel</Button>
              <Button 
                variant="primary" 
                disabled={!newKitName}
                onClick={() => {
                  ws?.send(`SAVE_KIT_AS:${newKitName}`);
                  setIsSaveKitModalOpen(false);
                  setNewKitName("");
                }}
              >
                Save Kit
              </Button>
            </div>
          </div>
        </div>
      )}

      <div className="flex flex-wrap gap-2 pb-2">
        {sounds.map(sound => (
          <button
            key={sound.id}
            onClick={() => setSelectedSoundId(sound.id)}
            aria-pressed={String(selectedSoundId) === String(sound.id)}
            className={cn(
              "flex-shrink-0 px-4 py-2 rounded-xl transition-all border flex items-center gap-2 focus:outline-none focus-visible:ring-2 focus-visible:ring-primary",
              String(selectedSoundId) === String(sound.id)
                ? "bg-primary text-primary-foreground shadow-lg border-primary"
                : "bg-card/30 border-border hover:border-primary/50"
            )}
          >
            <span className="font-bold text-xs uppercase tracking-widest">{sound.name}</span>
            {String(selectedSoundId) === String(sound.id) && <Sparkle size={12} weight="fill" />}
          </button>
        ))}
      </div>

      {selectedSound ? (() => {
        const safeAttack = selectedSound.attack ?? 0;
        const safeDecay = selectedSound.decay ?? 0;
        const safeFreq = selectedSound.freq ?? 440;
        const safeLfo1 = selectedSound.lfo1_freq ?? 1.0;
        const safeLfo2 = selectedSound.lfo2_freq ?? 1.0;
        const safeBits = selectedSound.bits ?? 16;
        const safeRate = selectedSound.rate ?? 1;
        return (
        <div className="grid grid-cols-1 lg:grid-cols-2 xl:grid-cols-3 2xl:grid-cols-5 gap-4 items-stretch">
          <section className="bg-card/30 border border-border rounded-3xl p-5 space-y-5 flex flex-col">
            <header className="flex items-center gap-2 text-xs font-black text-primary uppercase tracking-[0.18em]">
              <Cpu size={14} />
              1. Source
            </header>
            
            <div className="space-y-4">
              <div className="grid grid-cols-3 gap-2">
                {['fm', 'phys', 'granular', 'hybrid', 'modal', 'noise'].map(type => (
                  <button
                    key={type}
                    onClick={() => updateParam('engine_type' as any, type as any)}
                    aria-pressed={selectedSound.engine_type === type}
                    className={cn(
                      "px-2 py-2 rounded-lg text-[11px] font-black transition-all uppercase tracking-wider border focus:outline-none focus-visible:ring-2 focus-visible:ring-primary",
                      selectedSound.engine_type === type
                        ? "bg-primary border-primary text-primary-foreground"
                        : "bg-background/50 border-border text-muted-foreground hover:text-foreground hover:border-primary/40"
                    )}
                  >
                    {type}
                  </button>
                ))}
              </div>

              <div className="pt-2">
                <FrequencyVisualizer
                  value={safeFreq}
                  min={20}
                  max={2000}
                  onChange={v => updateParam('freq', v)}
                  modValue={getModulatedValue('freq', safeFreq)}
                />
                <PredictiveGraph
                  base={safeFreq}
                  min={20}
                  max={2000}
                  mods={selectedSound.mods?.filter(m => m.param === 'freq') || []}
                  attack={safeAttack}
                  decay={safeDecay}
                  lfo1_freq={safeLfo1}
                  lfo2_freq={safeLfo2}
                  className="w-full mt-3 h-12"
                />
              </div>
            </div>

            <div className="mt-auto pt-4 border-t border-border/50">
               <div className="text-[10px] font-medium text-muted-foreground italic">
                 The raw sound generation engine. Choose the core synthesis method.
               </div>
            </div>
          </section>

          <section className="bg-card/30 border border-border rounded-3xl p-5 space-y-5 flex flex-col">
            <header className="flex items-center gap-2 text-xs font-black text-primary uppercase tracking-[0.18em]">
              <Clock size={14} />
              2. Shape
            </header>
            
            <div className="flex-1 min-h-[200px] bg-background/50 rounded-2xl relative overflow-hidden border border-border/50">
              <EnvelopeEditor
                attack={safeAttack}
                decay={safeDecay}
                onChange={(a, d) => {
                  updateParam('attack', a);
                  updateParam('decay', d);
                }}
              />
            </div>

            <div className="grid grid-cols-2 gap-3">
               <div className="p-3 bg-background/30 rounded-xl border border-border/50">
                 <div className="text-[10px] font-black text-muted-foreground uppercase tracking-wider mb-1">Attack</div>
                 <div className="text-sm font-mono font-bold">{safeAttack.toFixed(0)} <span className="text-muted-foreground font-normal text-xs">ms</span></div>
               </div>
               <div className="p-3 bg-background/30 rounded-xl border border-border/50">
                 <div className="text-[10px] font-black text-muted-foreground uppercase tracking-wider mb-1">Decay</div>
                 <div className="text-sm font-mono font-bold">{safeDecay.toFixed(0)} <span className="text-muted-foreground font-normal text-xs">ms</span></div>
               </div>
            </div>
          </section>

          <section className="bg-card/30 border border-border rounded-3xl p-5 space-y-5 flex flex-col">
            <header className="flex items-center gap-2 text-xs font-black text-primary uppercase tracking-[0.18em]">
              <SlidersIcon size={14} />
              3. Timbre
            </header>
            
            <div className="space-y-6 max-h-[480px] overflow-y-auto pr-2 custom-scrollbar">
              {schemas[selectedSoundId]?.filter(p => !['freq', 'attack', 'decay', 'bits', 'rate'].includes(p.name)).map((param) => {
                const paramMods = selectedSound.mods?.filter(m => m.param === param.name) || [];
                // Render every existing mod plus one trailing empty "add new" row.
                const displayMods = [
                  ...paramMods,
                  { param: param.name, source: 'None', depth: 0 },
                ];

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
                      attack={safeAttack}
                      decay={safeDecay}
                      lfo1_freq={safeLfo1}
                      lfo2_freq={safeLfo2}
                    />
                  </div>
                );
              })}
            </div>
          </section>

          <ModulationPanel
            lfo1_freq={safeLfo1}
            lfo2_freq={safeLfo2}
            onChangeLfo={updateLfo}
            modValues={selectedSlotIndex !== -1 ? modStates[selectedSlotIndex] : undefined}
          />

          <section className="bg-card/30 border border-border rounded-3xl p-5 space-y-5 flex flex-col">
            <header className="flex items-center justify-between gap-2 text-xs font-black text-primary uppercase tracking-[0.18em]">
              <span className="flex items-center gap-2">
                <Waveform size={14} />
                5. FX
              </span>
              {safeBits >= 16 && safeRate <= 1 && (
                <span className="text-[9px] font-bold text-muted-foreground bg-muted/50 px-2 py-0.5 rounded-full normal-case tracking-normal">
                  idle
                </span>
              )}
            </header>

            <div className="space-y-3">
              <div className="text-[10px] font-black text-muted-foreground uppercase tracking-widest">Bitcrusher</div>
              <div className="p-4 bg-background/30 rounded-xl border border-border/50 space-y-5">
                <Slider
                  label="Bit depth"
                  value={safeBits}
                  min={1}
                  max={16}
                  step={1}
                  onChange={v => updateParam('bits', v)}
                  format={v => `${v.toFixed(0)} bits`}
                />
                <Slider
                  label="Rate divisor"
                  value={safeRate}
                  min={1}
                  max={32}
                  step={1}
                  onChange={v => updateParam('rate', v)}
                  format={v => `${v.toFixed(0)}x`}
                />
              </div>
            </div>

            <div className="mt-auto pt-4 border-t border-border/50">
               <div className="text-[10px] font-medium text-muted-foreground italic">
                 Post-FX shaping. Crush bits or downsample for lo-fi grit.
               </div>
            </div>
          </section>

        </div>
        );
      })() : (
        <div className="h-[400px] flex items-center justify-center border-2 border-dashed border-border rounded-3xl text-muted-foreground italic">
          Select a sound to start designing
        </div>
      )}
    </div>
  )
}
