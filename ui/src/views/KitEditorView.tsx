import { useState, useEffect, useMemo } from 'react'
import { Play, Sparkle, Sliders as SlidersIcon, Clock, Cpu, ArrowsClockwise, FloppyDisk, Waveform, Warning, SpeakerSlash } from "@phosphor-icons/react"
import { cn, ParamController, Button, FrequencyVisualizer, PredictiveGraph, Slider } from '../components/ui'
import { smartFormat } from '../components/format'
import { EnvelopeEditor } from '../components/EnvelopeEditor'
import { ModulationPanel } from '../components/ModulationPanel'
import type { AnalysisResult } from '../App'

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
  analysis?: Record<number, AnalysisResult>;
  requestAnalysis?: (slot: number) => void;
  /** Live BPM (string from App.tsx, may be "0.0" before first sync).
   *  Used to render "~Xs" / "~X Hz" previews on the tempo-locked
   *  decay / LFO badges. Optional — falls back to 120 BPM. */
  bpm?: string;
}

// Mirrors `BeatDivision::to_seconds` in `src/dsp/timing.rs`. Used by the
// tempo-lock hint on the Shape -> Decay slider so the UI can preview the
// engine-overridden decay length without a backend round-trip.
const DIVISION_QUARTERS: Record<string, number> = {
  ThirtySecond: 0.125,
  SixteenthTriplet: 1.0 / 6.0,
  Sixteenth: 0.25,
  SixteenthDotted: 0.375,
  EighthTriplet: 1.0 / 3.0,
  Eighth: 0.5,
  EighthDotted: 0.75,
  QuarterTriplet: 2.0 / 3.0,
  Quarter: 1.0,
  QuarterDotted: 1.5,
  Half: 2.0,
  Bar: 4.0,
  TwoBars: 8.0,
  FourBars: 16.0,
};

function divisionToSeconds(name: string, bpm: number): number | null {
  const q = DIVISION_QUARTERS[name];
  if (q === undefined) return null;
  return (q * 60.0) / Math.max(bpm, 0.01);
}

function decayHint(division: string, bpm: number): string {
  const sec = divisionToSeconds(division, bpm);
  if (sec === null || !isFinite(sec)) return `Tempo-locked to ${division}`;
  return `Tempo-locked to ${division} @ ${bpm.toFixed(1)} BPM (~${sec.toFixed(2)}s)`;
}

/** Severity tier derived from an AnalysisResult. */
type AnalysisStatus = 'clipping' | 'silent' | 'healthy';

function statusFor(a: AnalysisResult | undefined): AnalysisStatus | null {
  if (!a) return null;
  // A single envelope-peak sample touching unity is normal and fine; only
  // sustained rail-locking (>= ~100 consecutive samples) is audible
  // distortion. The backend already computes that into `sustained_clip`.
  if (a.sustained_clip) return 'clipping';
  if (a.silent) return 'silent';
  return 'healthy';
}

export default function KitEditorView({
  ws, sounds, setSounds, schemas,
  selectedSoundId, setSelectedSoundId,
  analysis = {}, requestAnalysis,
  bpm: bpmString,
}: KitEditorProps) {
  // The App-level bpm state is a string ("0.0" pre-sync); coerce to a
  // sensible number for the tempo-lock previews so we never divide by
  // zero / NaN downstream.
  const bpmNum = (() => {
    const n = parseFloat(bpmString ?? "");
    return isFinite(n) && n > 0 ? n : 120.0;
  })();
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
    requestAnalysis?.(Number(selectedSoundId));
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
    requestAnalysis?.(Number(selectedSoundId));
  };

  const updateLfo = (index: number, freq: number) => {
    if (selectedSoundId === null || !ws) return;

    setSounds(prev => prev.map(s =>
      String(s.id) === String(selectedSoundId) ? { ...s, [`lfo${index}_freq`]: freq } : s
    ));

    ws.send(`SET_LFO:${selectedSoundId}:${index}:${freq}`);
    requestAnalysis?.(Number(selectedSoundId));
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
        {sounds.map((sound, idx) => {
          const status = statusFor(analysis[idx]);
          const isSelected = String(selectedSoundId) === String(sound.id);
          const dotClasses =
            status === 'clipping' ? "bg-rose-500 shadow-[0_0_8px_rgb(244_63_94_/_0.8)]" :
            status === 'silent'   ? "bg-amber-400 shadow-[0_0_8px_rgb(251_191_36_/_0.7)]" :
            status === 'healthy'  ? "bg-emerald-500 shadow-[0_0_6px_rgb(16_185_129_/_0.6)]" :
                                    null;
          const dotTitle =
            status === 'clipping' ? "Voice clips on trigger" :
            status === 'silent'   ? "Voice is very quiet" :
            status === 'healthy'  ? "Voice level looks healthy" :
                                    undefined;
          return (
            <button
              key={sound.id}
              onClick={() => setSelectedSoundId(sound.id)}
              aria-pressed={isSelected}
              className={cn(
                "flex-shrink-0 px-4 py-2 rounded-xl transition-all border flex items-center gap-2 focus:outline-none focus-visible:ring-2 focus-visible:ring-primary",
                isSelected
                  ? "bg-primary text-primary-foreground shadow-lg border-primary"
                  : "bg-card/30 border-border hover:border-primary/50"
              )}
            >
              <span className="font-bold text-xs uppercase tracking-widest">{sound.name}</span>
              {dotClasses && (
                <span
                  role="img"
                  aria-label={dotTitle}
                  title={dotTitle}
                  className={cn("w-1.5 h-1.5 rounded-full transition-all", dotClasses)}
                />
              )}
              {isSelected && <Sparkle size={12} weight="fill" />}
            </button>
          );
        })}
      </div>

      {selectedSound ? (() => {
        const safeAttack = selectedSound.attack ?? 0;
        const safeDecay = selectedSound.decay ?? 0;
        const safeFreq = selectedSound.freq ?? 440;
        const safeLfo1 = selectedSound.lfo1_freq ?? 1.0;
        const safeLfo2 = selectedSound.lfo2_freq ?? 1.0;
        const safeBits = selectedSound.bits ?? 16;
        const safeRate = selectedSound.rate ?? 1;
        // Clock-aware effect fields. Defaults mirror the backend
        // (kit_to_json / GenerativeSettings::default) so a kit without
        // overrides shows non-firing ghosts and always-on triggers, which
        // matches what the audio engine does at runtime.
        const safeTrigProb = selectedSound.trigger_probability ?? 1.0;
        const safeGhostProb = selectedSound.ghost_probability ?? 0.0;
        const safeGhostOffset = selectedSound.ghost_offset_ms ?? 60.0;
        const safeGhostVel = selectedSound.ghost_velocity_factor ?? 0.3;
        const lfo1Div: string | null = selectedSound.lfo1_division ?? null;
        const lfo2Div: string | null = selectedSound.lfo2_division ?? null;
        const decayDiv: string | null = selectedSound.decay_division ?? null;
        const subHits: Array<{ offset_ms: number; velocity_factor: number }> =
          selectedSound.sub_hits ?? [];
        const patternSteps: Array<{ division: string; velocity_factor: number; multiplier: number }> =
          selectedSound.pattern ?? [];
        const modeList: Array<{ freq: number; q: number; gain: number }> =
          selectedSound.mode_list ?? [];
        // A slot is "clock-aware" if any of the new fields is non-default.
        // We use this to gate rendering the Clock section so kits without
        // any of these features don't grow a new always-empty subsection.
        const hasClockFeatures =
          lfo1Div !== null ||
          lfo2Div !== null ||
          decayDiv !== null ||
          subHits.length > 0 ||
          patternSteps.length > 0 ||
          modeList.length > 0;
        const timbreParams = schemas[selectedSoundId]?.filter(p => !['freq', 'attack', 'decay', 'bits', 'rate'].includes(p.name)) ?? [];
        const selectedAnalysis = selectedSlotIndex !== -1 ? analysis[selectedSlotIndex] : undefined;
        const selectedStatus = statusFor(selectedAnalysis);
        return (
        <div className="flex flex-col gap-4">
          {selectedStatus === 'clipping' && (
            <div
              role="alert"
              className="flex items-start gap-3 px-4 py-3 rounded-2xl border border-rose-500/40 bg-rose-500/10 text-rose-200"
            >
              <Warning size={18} weight="fill" className="text-rose-400 mt-0.5 shrink-0" />
              <div className="text-xs leading-relaxed">
                <span className="font-bold uppercase tracking-wider text-rose-300">Clipping</span>
                {selectedAnalysis && (
                  <span className="ml-2 font-mono text-[10px] text-rose-300/80">
                    peak {selectedAnalysis.peak.toFixed(3)} / RMS {selectedAnalysis.rms.toFixed(3)}
                  </span>
                )}
                <div className="mt-0.5 text-rose-100/80">
                  This voice will clip on trigger. Try lowering density, metallic, brightness, or any high-depth modulations.
                </div>
              </div>
            </div>
          )}
          {selectedStatus === 'silent' && (
            <div
              role="alert"
              className="flex items-start gap-3 px-4 py-3 rounded-2xl border border-amber-500/40 bg-amber-500/10 text-amber-200"
            >
              <SpeakerSlash size={18} weight="fill" className="text-amber-400 mt-0.5 shrink-0" />
              <div className="text-xs leading-relaxed">
                <span className="font-bold uppercase tracking-wider text-amber-300">Too quiet</span>
                {selectedAnalysis && (
                  <span className="ml-2 font-mono text-[10px] text-amber-300/80">
                    peak {selectedAnalysis.peak.toFixed(3)}
                  </span>
                )}
                <div className="mt-0.5 text-amber-100/80">
                  This voice is too quiet to hear. Try raising mod_index, brightness, or the envelope depth.
                </div>
              </div>
            </div>
          )}
          {/* 1. SOURCE */}
          <section className="bg-card/30 border border-border rounded-3xl p-5 flex flex-col gap-5">
            <header className="flex items-center justify-between gap-2 text-xs font-black text-primary uppercase tracking-[0.18em]">
              <span className="flex items-center gap-2">
                <Cpu size={14} />
                1. Source
              </span>
              <span className="text-[10px] font-medium text-muted-foreground italic normal-case tracking-normal hidden md:inline">
                Raw synthesis engine and base pitch
              </span>
            </header>

            <div className="grid grid-cols-1 lg:grid-cols-[minmax(0,1fr)_minmax(0,2fr)] gap-5 items-start">
              <div className="grid grid-cols-3 gap-2">
                {['fm', 'phys', 'granular', 'hybrid', 'modal', 'noise'].map(type => (
                  <button
                    key={type}
                    onClick={() => updateParam('engine_type' as any, type as any)}
                    aria-pressed={selectedSound.engine_type === type}
                    aria-label={`Set engine to ${type}`}
                    className={cn(
                      "px-3 py-2.5 rounded-lg text-[11px] font-black transition-all uppercase tracking-wider border focus:outline-none focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-offset-2 focus-visible:ring-offset-background",
                      selectedSound.engine_type === type
                        ? "bg-primary border-primary text-primary-foreground shadow-lg shadow-primary/20"
                        : "bg-background/50 border-border text-muted-foreground hover:text-foreground hover:border-primary/40 hover:bg-background"
                    )}
                  >
                    {type}
                  </button>
                ))}
              </div>

              <div>
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
          </section>

          {/* 2. SHAPE */}
          <section className="bg-card/30 border border-border rounded-3xl p-5 flex flex-col gap-5">
            <header className="flex items-center justify-between gap-2 text-xs font-black text-primary uppercase tracking-[0.18em]">
              <span className="flex items-center gap-2">
                <Clock size={14} />
                2. Shape
              </span>
              <span className="text-[10px] font-medium text-muted-foreground italic normal-case tracking-normal hidden md:inline">
                Drag the curve to shape attack and decay
              </span>
            </header>

            <div className="grid grid-cols-1 lg:grid-cols-[minmax(0,2fr)_minmax(0,1fr)] gap-5 items-stretch">
              <div className="min-h-[200px] bg-background/50 rounded-2xl relative overflow-hidden border border-border/50">
                <EnvelopeEditor
                  attack={safeAttack}
                  decay={safeDecay}
                  onChange={(a, d) => {
                    updateParam('attack', a);
                    updateParam('decay', d);
                  }}
                />
              </div>

              <div className="grid grid-cols-2 lg:grid-cols-1 gap-3 content-start">
                <div className="p-3 bg-background/30 rounded-xl border border-border/50">
                  <div className="text-[10px] font-black text-muted-foreground uppercase tracking-wider mb-1">Attack</div>
                  <div className="text-sm font-mono font-bold">{safeAttack.toFixed(0)} <span className="text-muted-foreground font-normal text-xs">ms</span></div>
                </div>
                <div className="p-3 bg-background/30 rounded-xl border border-border/50">
                  <div className="flex items-center gap-2 mb-2">
                    <div className="text-[10px] font-black text-muted-foreground uppercase tracking-wider">Decay</div>
                    {decayDiv && (
                      <span
                        title={decayHint(decayDiv, bpmNum)}
                        className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded-full bg-amber-500/15 border border-amber-500/40 text-amber-300 text-[9px] font-bold uppercase tracking-wider"
                      >
                        <Clock size={9} weight="fill" />
                        Tempo-locked
                      </span>
                    )}
                  </div>
                  <Slider
                    label={decayDiv ? "Decay (tempo-locked)" : "Decay"}
                    value={safeDecay}
                    min={1}
                    max={5000}
                    step={1}
                    format={v => `${v.toFixed(0)} ms`}
                    onChange={v => updateParam('decay', v)}
                    disabled={!!decayDiv}
                    disabledHint={decayDiv ? decayHint(decayDiv, bpmNum) : undefined}
                  />
                </div>
              </div>
            </div>
          </section>

          {/* 3. TIMBRE */}
          <section className="bg-card/30 border border-border rounded-3xl p-5 flex flex-col gap-5">
            <header className="flex items-center justify-between gap-2 text-xs font-black text-primary uppercase tracking-[0.18em]">
              <span className="flex items-center gap-2">
                <SlidersIcon size={14} />
                3. Timbre
              </span>
              <span className="text-[10px] font-medium text-muted-foreground italic normal-case tracking-normal hidden md:inline">
                Engine-specific parameters. Add modulation slots per knob.
              </span>
            </header>

            {timbreParams.length === 0 ? (
              <div className="text-[10px] text-muted-foreground italic py-8 text-center">
                No timbre parameters for this engine.
              </div>
            ) : (
              <div className="grid grid-cols-1 xl:grid-cols-2 gap-x-8 gap-y-6">
                {timbreParams.map((param) => {
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
                        format={v => smartFormat(v, param.unit)}
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
            )}
          </section>

          {/* 4. MODULATION */}
          <ModulationPanel
            lfo1_freq={safeLfo1}
            lfo2_freq={safeLfo2}
            onChangeLfo={updateLfo}
            modValues={selectedSlotIndex !== -1 ? modStates[selectedSlotIndex] : undefined}
            lfo1_division={lfo1Div}
            lfo2_division={lfo2Div}
            bpm={bpmNum}
          />

          {/* 5. FX */}
          <section className="bg-card/30 border border-border rounded-3xl p-5 flex flex-col gap-5">
            <header className="flex items-center justify-between gap-2 text-xs font-black text-primary uppercase tracking-[0.18em]">
              <span className="flex items-center gap-2">
                <Waveform size={14} />
                5. FX
              </span>
              <span className="flex items-center gap-3">
                {safeBits >= 16 && safeRate <= 1 && (
                  <span className="text-[9px] font-bold text-muted-foreground bg-muted/50 px-2 py-0.5 rounded-full normal-case tracking-normal">
                    idle
                  </span>
                )}
                <span className="text-[10px] font-medium text-muted-foreground italic normal-case tracking-normal hidden md:inline">
                  Post-FX lo-fi grit
                </span>
              </span>
            </header>

            <div className="max-w-3xl w-full">
              <div className="text-[10px] font-black text-muted-foreground uppercase tracking-widest mb-3">Bitcrusher</div>
              <div className="p-4 bg-background/30 rounded-xl border border-border/50 grid grid-cols-1 md:grid-cols-2 gap-6">
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

            {/*
              Generative subsection: probability + ghost-note controls.
              Wired through the existing SET_PARAM dispatch (see Phase A
              extension to SET_PARAM in commands.rs). Ghost-related
              sliders are hidden when ghost_probability == 0 since they
              have no audible effect — keeps the panel clean for kits
              that don't use ghosting.
            */}
            <div className="max-w-3xl w-full">
              <div className="text-[10px] font-black text-muted-foreground uppercase tracking-widest mb-3">Generative</div>
              <div className="p-4 bg-background/30 rounded-xl border border-border/50 grid grid-cols-1 md:grid-cols-2 gap-6">
                <Slider
                  label="Trigger %"
                  value={safeTrigProb}
                  min={0}
                  max={1}
                  step={0.01}
                  onChange={v => updateParam('trigger_probability' as any, v)}
                  format={v => `${Math.round(v * 100)}%`}
                />
                <Slider
                  label="Ghost %"
                  value={safeGhostProb}
                  min={0}
                  max={1}
                  step={0.01}
                  onChange={v => updateParam('ghost_probability' as any, v)}
                  format={v => `${Math.round(v * 100)}%`}
                />
                {safeGhostProb > 0 && (
                  <>
                    <Slider
                      label="Ghost offset"
                      value={safeGhostOffset}
                      min={1}
                      max={500}
                      step={1}
                      onChange={v => updateParam('ghost_offset_ms' as any, v)}
                      format={v => `${v.toFixed(0)} ms`}
                    />
                    <Slider
                      label="Ghost velocity"
                      value={safeGhostVel}
                      min={0}
                      max={1}
                      step={0.01}
                      onChange={v => updateParam('ghost_velocity_factor' as any, v)}
                      format={v => v.toFixed(2)}
                    />
                  </>
                )}
              </div>
            </div>
          </section>

          {/*
            6. Clock-aware indicators. Read-only display of the tempo-locked
            and compound clock-aware fields on this slot. Editing
            sub_hits / pattern / mode_list / divisions requires a richer
            UI than a single slider, so we surface them as informational
            badges -- enough to address "this kit has hidden features I
            can't see" (HIGH bug #6) without committing to a full editor
            in this pass. The decay-division / lfo*_division warnings on
            their respective sliders (Shape -> Decay, Modulation -> LFO
            Rate) are the primary user-facing indicator now; this Clock
            section remains the holistic summary plus the only surface
            for sub_hits / pattern / mode_list.
          */}
          {hasClockFeatures && (
            <section className="bg-card/30 border border-border rounded-3xl p-5 flex flex-col gap-3">
              <header className="flex items-center justify-between gap-2 text-xs font-black text-primary uppercase tracking-[0.18em]">
                <span className="flex items-center gap-2">
                  <Clock size={14} />
                  6. Clock
                </span>
                <span className="text-[10px] font-medium text-muted-foreground italic normal-case tracking-normal hidden md:inline">
                  Tempo-locked overrides &amp; generative recipes (read-only)
                </span>
              </header>

              <div className="flex flex-wrap gap-2 text-[11px]">
                {lfo1Div && (
                  <span className="px-2.5 py-1 rounded-full bg-primary/10 border border-primary/30 text-primary font-mono">
                    LFO1 ⏱ {lfo1Div}
                  </span>
                )}
                {lfo2Div && (
                  <span className="px-2.5 py-1 rounded-full bg-primary/10 border border-primary/30 text-primary font-mono">
                    LFO2 ⏱ {lfo2Div}
                  </span>
                )}
                {decayDiv && (
                  <span
                    title="Decay slider has no effect while this division is set"
                    className="px-2.5 py-1 rounded-full bg-amber-500/15 border border-amber-500/40 text-amber-300 font-mono"
                  >
                    ⏱ Decay locked to {decayDiv} (slider has no effect)
                  </span>
                )}
                {subHits.length > 0 && (
                  <details className="px-2.5 py-1 rounded-full bg-card border border-border text-muted-foreground font-mono cursor-pointer">
                    <summary>⚡ {subHits.length} sub-hits</summary>
                    <div className="mt-2 text-[10px] font-mono space-y-0.5">
                      {subHits.map((s, i) => (
                        <div key={i}>
                          +{s.offset_ms.toFixed(1)} ms × {s.velocity_factor.toFixed(2)}
                        </div>
                      ))}
                    </div>
                  </details>
                )}
                {patternSteps.length > 0 && (
                  <details className="px-2.5 py-1 rounded-full bg-card border border-border text-muted-foreground font-mono cursor-pointer">
                    <summary>⚡ {patternSteps.length}-step pattern</summary>
                    <div className="mt-2 text-[10px] font-mono space-y-0.5">
                      {patternSteps.map((p, i) => (
                        <div key={i}>
                          {p.division} ×{p.multiplier.toFixed(1)} @ {p.velocity_factor.toFixed(2)}
                        </div>
                      ))}
                    </div>
                  </details>
                )}
                {modeList.length > 0 && (
                  <details className="px-2.5 py-1 rounded-full bg-card border border-border text-muted-foreground font-mono cursor-pointer">
                    <summary>🔔 {modeList.length} explicit modes</summary>
                    <div className="mt-2 text-[10px] font-mono space-y-0.5">
                      {modeList.map((m, i) => (
                        <div key={i}>
                          {m.freq.toFixed(1)} Hz · Q={m.q.toFixed(1)} · g={m.gain.toFixed(2)}
                        </div>
                      ))}
                    </div>
                  </details>
                )}
              </div>
            </section>
          )}

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
