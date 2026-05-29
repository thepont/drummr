import { useState, useEffect, useRef } from 'react'
import { House, ListDashes, Faders, WifiHigh, WifiSlash, SpeakerHigh, Cpu, List as ListIcon, X, Pulse, Books, Clock, Sparkle, Warning } from "@phosphor-icons/react"
import { clsx, type ClassValue } from 'clsx'
import { twMerge } from 'tailwind-merge'

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

import MappingView from './views/MappingView'
import KitEditorView from './views/KitEditorView'
import LibraryView from './views/LibraryView'
import PerformanceView from './views/PerformanceView'
import { Card } from './components/ui'
import { MasterPeakMeter } from './components/MasterPeakMeter'
import LibrarySidebar from './components/LibrarySidebar'
import { PreviewKitButton } from './components/PreviewKitButton'

type View = 'performance' | 'dashboard' | 'mapping' | 'editor' | 'library';

export interface AnalysisResult {
  slot: number;
  peak: number;
  rms: number;
  clipped_samples: number;
  sustained_clip: boolean;
  silent: boolean;
  engine: string;
  decay_ms: number;
}

export default function App() {
  const [view, setView] = useState<View>('performance');
  const [status, setStatus] = useState<'Connecting' | 'Connected' | 'Disconnected'>('Connecting');
  const [ws, setWs] = useState<WebSocket | null>(null);
  const [midiPort, setMidiPort] = useState<string>('None');
  const [audioDevice, setAudioDevice] = useState<string>('None');
  const [isMobileMenuOpen, setIsMobileMenuOpen] = useState(false);
  const [isLibraryOpen, setIsLibraryOpen] = useState(true);
  
  const [bpm, setBpm] = useState<string>("0.0");
  const [isAutoSync, setIsAutoSync] = useState(false);
  const [syncStatus, setSyncStatus] = useState("Stopped");

  const [availableMidi, setAvailableMidi] = useState<string[]>([]);
  const [availableAudio, setAvailableAudio] = useState<string[]>([]);
  const [availableHosts, setAvailableHosts] = useState<string[]>([]);
  const [audioHost, setAudioHost] = useState<string>("");
  const [bufferSize, setBufferSize] = useState<number>(128);
  const [availableKits, setAvailableKits] = useState<string[]>([]);
  const [activeKitName, setActiveKitName] = useState<string>("");
  
  const [sounds, setSounds] = useState<any[]>([]);
  const [schemas, setSchemas] = useState<Record<string, any[]>>({});
  const [soundPresets, setSoundPresets] = useState<string[]>([]);
  const [mappingPresets, setMappingPresets] = useState<string[]>([]);
  const [selectedSoundId, setSelectedSoundId] = useState<any>(null);
  const [analysis, setAnalysis] = useState<Record<number, AnalysisResult>>({});

  const [lastMidi, setLastMidi] = useState<{note: number, vel: number} | null>(null);
  const [isMidiFlashing, setIsMidiFlashing] = useState(false);
  
  const [toasts, setToasts] = useState<Array<{ id: string; message: string; type: 'success' | 'error' | 'info' }>>([]);
  const [masterPeak, setMasterPeak] = useState<number>(0);
  const lastKitNameRef = useRef<string>("");

  const showToast = (message: string, type: 'success' | 'error' | 'info' = 'success') => {
    const id = Math.random().toString(36).substring(2, 9);
    setToasts(prev => [...prev, { id, message, type }]);
    setTimeout(() => {
      setToasts(prev => prev.filter(t => t.id !== id));
    }, 4000);
  };

  // MIDI flash timer ref: replaced on each new MIDI event so the previous
  // pending "flash off" never lands AFTER a fresh "flash on", and so we
  // don't leak timers on unmount during high-rate MIDI input (drum hits
  // can arrive faster than the 80ms flash window).
  const midiFlashTimerRef = useRef<number | null>(null);

  // Preview Kit: backend-curated list of CC-BY MIDI drum tracks that can be
  // played through the active kit to audition it in a real musical context.
  // Track names come from MIDI_TRACKS:<csv>; the currently-playing one is set
  // by MIDI_TRACK_PLAYING:<name> and cleared by MIDI_TRACK_STOPPED:<name?>.
  const [midiTracks, setMidiTracks] = useState<string[]>([]);
  const [playingTrack, setPlayingTrack] = useState<string | null>(null);

  useEffect(() => {
    let reconnectTimeout: number;

    const connect = () => {
      setStatus('Connecting');
      
      const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
      let wsUrl: string;

      if (window.location.port === '5173' || window.location.port === '3000') {
        // We are likely in a dev environment (Vite/CRA default ports)
        // Connect to the backend on 8080 regardless of hostname
        wsUrl = `${protocol}//${window.location.hostname}:8080`;
      } else {
        // We are likely in "production" (e.g. served via Nginx on port 80)
        // Use the /ws path which should be proxied
        wsUrl = `${protocol}//${window.location.host}/ws`;
      }


      console.log('[UI] Connecting to:', wsUrl);
      const socket = new WebSocket(wsUrl);
      let isCurrent = true;
      
      socket.onopen = () => {
        if (!isCurrent) return;
        setStatus('Connected');
        showToast('Connected to drummr engine', 'success');
        socket.send('LIST_HOSTS');
        socket.send('LIST_MIDI');
        socket.send('LIST_AUDIO');
        socket.send('LIST_KITS');
        socket.send('GET_SYNC_STATUS');
        socket.send('GET_KIT');
        socket.send('GET_MAPPING');
        socket.send('LIST_SOUND_PRESETS');
        socket.send('LIST_MAPPING_PRESETS');
        socket.send('LIST_MIDI_TRACKS');
        setWs(socket);
      };

      socket.onclose = () => {
        if (!isCurrent) return;
        setStatus('Disconnected');
        showToast('Connection lost. Reconnecting...', 'error');
        setWs(null);
        reconnectTimeout = window.setTimeout(connect, 2000);
      };

      socket.onerror = () => {
        if (!isCurrent) return;
        showToast('WebSocket error encountered', 'error');
        socket.close();
      };

      socket.onmessage = (event) => {
        if (!isCurrent) return;
        const data = event.data as string;
        
        if (data.startsWith('PORT: ')) {
          setMidiPort(data.replace('PORT: ', ''));
        } else if (data.startsWith('AUDIO_DEVICE: ')) {
          setAudioDevice(data.replace('AUDIO_DEVICE: ', ''));
        } else if (data.startsWith('AUDIO_HOST: ')) {
          setAudioHost(data.replace('AUDIO_HOST: ', ''));
        } else if (data.startsWith('BUFFER_SIZE: ')) {
          setBufferSize(parseInt(data.replace('BUFFER_SIZE: ', '')));
        } else if (data.startsWith('LIST_MIDI: ')) {
          setAvailableMidi(data.replace('LIST_MIDI: ', '').split(',').filter(Boolean));
        } else if (data.startsWith('LIST_AUDIO: ')) {
          setAvailableAudio(data.replace('LIST_AUDIO: ', '').split(',').filter(Boolean));
        } else if (data.startsWith('LIST_HOSTS: ')) {
          setAvailableHosts(data.replace('LIST_HOSTS: ', '').split(',').filter(Boolean));
        } else if (data.startsWith('KIT_LIST:')) {
          setAvailableKits(data.replace('KIT_LIST:', '').split(',').filter(Boolean));
        } else if (data.startsWith('ACTIVE_KIT:')) {
          const name = data.replace('ACTIVE_KIT:', '');
          setActiveKitName(name);
          if (lastKitNameRef.current && lastKitNameRef.current !== name) {
            showToast(`Kit active: "${name}"`, 'success');
          }
          lastKitNameRef.current = name;
        } else if (data.startsWith('KIT_ERROR:')) {
          const err = data.replace('KIT_ERROR:', '');
          showToast(`Failed to load kit: ${err}`, 'error');
        } else if (data.startsWith('KIT: ')) {
          try {
            const kit = JSON.parse(data.replace('KIT: ', ''));
            console.log('[UI] Setting sounds from KIT broadcast:', kit.length);
            setSounds(kit);
            if (Array.isArray(kit) && socket.readyState === WebSocket.OPEN) {
              kit.forEach((slot: any, i: number) => {
                if (slot) {
                  socket.send('GET_SCHEMA:' + i);
                  // Optimistic: backend may or may not implement ANALYZE_SLOT yet.
                  // If it doesn't, we'll never receive ANALYSIS: messages and
                  // the UI just won't show analysis dots/banners. No errors.
                  socket.send('ANALYZE_SLOT:' + i);
                }
              });
            }
          } catch (e) { console.error(e); }
        } else if (data.startsWith('ANALYSIS:')) {
          // Format: ANALYSIS:<slot>|<json>
          const pipeIdx = data.indexOf('|');
          if (pipeIdx > 'ANALYSIS:'.length) {
            const slot = parseInt(data.substring('ANALYSIS:'.length, pipeIdx));
            try {
              const result = JSON.parse(data.substring(pipeIdx + 1)) as AnalysisResult;
              if (!Number.isNaN(slot)) {
                setAnalysis(prev => ({ ...prev, [slot]: result }));
              }
            } catch (e) { console.error('Failed to parse ANALYSIS payload', e); }
          }
        } else if (data.startsWith('SOUND_PRESETS:')) {
          setSoundPresets(data.replace('SOUND_PRESETS:', '').split(',').filter(Boolean));
        } else if (data.startsWith('MAPPING_PRESETS:')) {
          setMappingPresets(data.replace('MAPPING_PRESETS:', '').split(',').filter(Boolean));
        } else if (data.startsWith('SCHEMA:')) {
          try {
            const firstColon = data.indexOf(':');
            const firstPipe = data.indexOf('|', firstColon + 1);
            const soundId = data.substring(firstColon + 1, firstPipe);
            const jsonStr = data.substring(firstPipe + 1);
            const schema = JSON.parse(jsonStr);
            setSchemas(prev => ({ ...prev, [soundId]: schema }));
          } catch (e) { console.error(e); }
        } else if (data.startsWith('BPM:')) {
          setBpm(data.replace('BPM:', '').trim());
        } else if (data.startsWith('SYNC_STATUS:')) {
          setSyncStatus(data.replace('SYNC_STATUS:', '').trim());
        } else if (data.startsWith('MIDI_TRACKS:')) {
          setMidiTracks(data.replace('MIDI_TRACKS:', '').split(',').filter(Boolean));
        } else if (data.startsWith('MIDI_TRACK_PLAYING:')) {
          setPlayingTrack(data.replace('MIDI_TRACK_PLAYING:', ''));
        } else if (data.startsWith('MIDI_TRACK_STOPPED')) {
          // MIDI_TRACK_STOPPED: (manual stop) or MIDI_TRACK_STOPPED:<name>
          // (natural end). Either way, reset to the idle state.
          setPlayingTrack(null);
        } else if (data.startsWith('MIDI_TRACK_ERROR:')) {
          // Backend couldn't load the requested track -- log and reset.
          console.warn('Preview Kit:', data);
          showToast('Failed to play preview MIDI track', 'error');
          setPlayingTrack(null);
        } else if (data.startsWith('PEAK:')) {
          const val = parseFloat(data.replace('PEAK:', ''));
          if (!isNaN(val)) {
            setMasterPeak(val);
          }
        } else if (data.startsWith('MIDI: ')) {
          const rawValues = data.replace('MIDI: ', '');
          const parts = rawValues.split(',');
          if (parts.length < 2) return;
          const note = parseInt(parts[0]);
          const vel = parseInt(parts[1]);
          if (isNaN(note) || isNaN(vel)) return;
          if (midiFlashTimerRef.current !== null) {
            window.clearTimeout(midiFlashTimerRef.current);
          }
          if (vel > 0) {
            setLastMidi({ note, vel });
            setIsMidiFlashing(true);
            midiFlashTimerRef.current = window.setTimeout(() => {
              midiFlashTimerRef.current = null;
              setIsMidiFlashing(false);
            }, 80);
          } else {
            setIsMidiFlashing(true);
            midiFlashTimerRef.current = window.setTimeout(() => {
              midiFlashTimerRef.current = null;
              setIsMidiFlashing(false);
            }, 40);
          }
        }
      };

      return () => {
        isCurrent = false;
        socket.close();
      };
    };

    const cleanup = connect();
    return () => {
      cleanup();
      clearTimeout(reconnectTimeout);
    };
  }, []);

  const closeMenu = () => setIsMobileMenuOpen(false);

  // Debounced per-slot ANALYZE_SLOT requests. Dragging a slider fires SET_PARAM
  // every animation frame; we collapse those into one analysis request 500ms
  // after the dragging settles so we don't spam the backend.
  const analyzeTimersRef = useRef<Record<number, number>>({});
  const requestAnalysis = (slot: number) => {
    if (!ws || Number.isNaN(slot)) return;
    const existing = analyzeTimersRef.current[slot];
    if (existing) window.clearTimeout(existing);
    analyzeTimersRef.current[slot] = window.setTimeout(() => {
      delete analyzeTimersRef.current[slot];
      if (ws.readyState === WebSocket.OPEN) {
        ws.send('ANALYZE_SLOT:' + slot);
      }
    }, 500);
  };

  // On unmount: cancel any in-flight debounce timers and the MIDI-flash timer.
  useEffect(() => {
    return () => {
      Object.values(analyzeTimersRef.current).forEach(id => window.clearTimeout(id));
      analyzeTimersRef.current = {};
      if (midiFlashTimerRef.current !== null) {
        window.clearTimeout(midiFlashTimerRef.current);
        midiFlashTimerRef.current = null;
      }
    };
  }, []);

  const toggleAutoSync = () => {
    const next = !isAutoSync;
    setIsAutoSync(next);
    ws?.send(`SET_AUTO_SYNC:${next}`);
  };

  const toggleSync = () => {
    if (syncStatus === "Running") {
      ws?.send("SYNC_STOP");
    } else {
      ws?.send("SYNC_START");
    }
  };

  return (
    <div className="flex h-screen w-full overflow-hidden bg-background text-foreground select-none">
      {/* Sidebar - Desktop */}
      <nav className="hidden lg:flex flex-col w-64 border-r border-border bg-card/50 backdrop-blur-xl shrink-0">
        <SidebarContent 
          view={view} 
          setView={setView} 
          status={status} 
          midiPort={midiPort} 
          audioDevice={audioDevice} 
          isMidiActive={isMidiFlashing}
        />
      </nav>

      {/* Main Content Area */}
      <div className="flex-1 flex flex-col min-w-0 overflow-hidden">
          <header className="h-16 border-b border-border flex items-center justify-between px-4 lg:px-8 bg-background/50 backdrop-blur-md z-10 shrink-0">
            <div className="flex items-center gap-4">
              <button
                onClick={() => setIsMobileMenuOpen(true)}
                aria-label="Open navigation menu"
                className="lg:hidden p-2 hover:bg-muted rounded-lg focus:outline-none focus-visible:ring-2 focus-visible:ring-primary"
              >
                <ListIcon size={24} />
              </button>
              <div className="flex flex-col">
                <h2 className="text-sm font-black uppercase tracking-widest text-foreground leading-tight">
                  {activeKitName || "Loading Kit..."}
                </h2>
                <span className="text-[10px] font-bold uppercase tracking-widest text-muted-foreground/60">
                  {view.replace('_', ' ')}
                </span>
              </div>

            </div>
            
            <div className="flex items-center gap-4">
               <div className="flex items-center gap-3 border-r border-border pr-6 h-8">
                  <div className={cn(
                    "w-2 h-2 rounded-full transition-all duration-75",
                    isMidiFlashing ? "bg-emerald-400 shadow-[0_0_10px_#34d399] scale-125" : "bg-zinc-800"
                  )} />
                  <span className="text-[10px] font-bold uppercase tracking-tighter text-muted-foreground">MIDI In</span>
               </div>

               <button
                onClick={() => setIsLibraryOpen(!isLibraryOpen)}
                aria-pressed={isLibraryOpen}
                aria-label={isLibraryOpen ? 'Hide library' : 'Show library'}
                className={cn(
                  "px-3 py-2 rounded-lg transition-colors border flex items-center gap-2 focus:outline-none focus-visible:ring-2 focus-visible:ring-primary",
                  isLibraryOpen ? "bg-primary/10 border-primary/50 text-primary" : "bg-muted/50 border-border text-muted-foreground hover:text-foreground hover:border-primary/30"
                )}
               >
                 <Books size={20} />
                 <span className="text-xs font-bold uppercase hidden sm:inline">Library</span>
               </button>
            </div>
          </header>

          {/* Transport Bar */}
          <div className="bg-card/40 border-b border-border h-16 flex items-center px-4 lg:px-8 gap-3 sm:gap-4 md:gap-6 lg:gap-8 backdrop-blur-lg shrink-0 relative z-20">
             <div className="flex flex-col justify-center pr-4 sm:pr-8 border-r border-white/5 h-full">
                <span className="text-[8px] font-black text-muted-foreground uppercase tracking-[0.2em] mb-1">Estimated BPM</span>
                <span className="text-2xl font-black text-primary tabular-nums tracking-tighter leading-none min-w-[5rem]">
                  {parseFloat(bpm) > 0 ? bpm : "---"}
                </span>
             </div>

             <div className="flex items-center gap-2 md:gap-3 shrink-0">
                <button
                  onClick={toggleAutoSync}
                  aria-pressed={isAutoSync}
                  className={cn(
                    "px-3 md:px-4 py-2.5 rounded-xl text-[10px] font-black uppercase tracking-wider transition-all border flex items-center gap-2 focus:outline-none focus-visible:ring-2 focus-visible:ring-primary shrink-0",
                    isAutoSync ? "bg-amber-500/20 border-amber-500 text-amber-500 shadow-[0_0_15px_rgba(245,158,11,0.2)]" : "bg-background/50 border-border text-muted-foreground hover:text-foreground hover:border-primary/30"
                  )}
                >
                  <Pulse size={14} weight={isAutoSync ? "fill" : "regular"} />
                  <span className="hidden sm:inline">Auto-Record</span>
                  <span className="sm:hidden">Auto</span>
                </button>

                <button
                  onClick={toggleSync}
                  aria-pressed={syncStatus === "Running"}
                  className={cn(
                    "px-4 md:px-6 py-2.5 rounded-xl text-[10px] font-black uppercase tracking-wider transition-all border flex items-center gap-2 focus:outline-none focus-visible:ring-2 focus-visible:ring-primary shrink-0",
                    syncStatus === "Running" ? "bg-emerald-500 border-emerald-500 text-white shadow-[0_0_25px_rgba(16,185,129,0.5)]" : "bg-background/50 border-border text-muted-foreground hover:text-foreground hover:border-primary/30"
                  )}
                >
                  <span className="hidden sm:inline">{syncStatus === "Running" ? "Master GO" : "Start Master Sync"}</span>
                  <span className="sm:hidden">{syncStatus === "Running" ? "GO" : "Sync"}</span>
                </button>

                <PreviewKitButton ws={ws} tracks={midiTracks} playingTrack={playingTrack} />
             </div>

             <div className="ml-auto flex items-center gap-3 sm:gap-6">
                <div className="hidden md:flex flex-col items-end">
                  <span className="text-[8px] font-black text-muted-foreground uppercase tracking-widest mb-1">Signal Status</span>
                  <div className="flex items-center gap-2">
                     <div className={cn(
                       "w-2.5 h-2.5 rounded-full transition-all duration-300",
                       syncStatus === "Running" ? "bg-emerald-500 animate-pulse shadow-[0_0_12px_rgba(16,185,129,1)]" : "bg-zinc-800"
                     )} />
                     <span className={cn(
                       "text-[10px] font-black uppercase tracking-widest",
                       syncStatus === "Running" ? "text-emerald-400" : "text-muted-foreground"
                     )}>
                       {syncStatus === "Running" ? "TRANSMITTING" : "STOPPED"}
                     </span>
                  </div>
                </div>
                <div className="hidden md:block w-px h-10 bg-border" />
                <MasterPeakMeter peak={masterPeak} />
             </div>
          </div>

          <div className="flex-1 overflow-auto bg-background/20">
            <div className="p-4 lg:p-8 max-w-7xl mx-auto pb-24 lg:pb-8">
              {view === 'performance' && (
                <PerformanceView
                  ws={ws}
                  activeKitName={activeKitName}
                  availableKits={availableKits}
                  bpm={bpm}
                  masterPeak={masterPeak}
                  isMidiActive={isMidiFlashing}
                  syncStatus={syncStatus}
                  toggleSync={toggleSync}
                />
              )}
              {view === 'dashboard' && (
                <DashboardView 
                  ws={ws} 
                  midiPort={midiPort} 
                  audioDevice={audioDevice} 
                  audioHost={audioHost}
                  availableHosts={availableHosts}
                  bufferSize={bufferSize}
                  availableMidi={availableMidi} 
                  availableAudio={availableAudio}
                  lastMidi={lastMidi} isMidiActive={isMidiFlashing}
                />
              )}
              {view === 'mapping' && (
                <MappingView 
                  ws={ws} 
                  sounds={sounds}
                  mappingPresets={mappingPresets}
                  selectedSoundId={selectedSoundId}
                  setSelectedSoundId={setSelectedSoundId}
                />
              )}
              {view === 'editor' && (
                <KitEditorView
                  ws={ws} sounds={sounds} setSounds={setSounds}
                  schemas={schemas} setSchemas={setSchemas}
                  selectedSoundId={selectedSoundId} setSelectedSoundId={setSelectedSoundId}
                  analysis={analysis} requestAnalysis={requestAnalysis}
                  bpm={bpm}
                  activeKitName={activeKitName}
                />
              )}
              {view === 'library' && (
                <LibraryView 
                  availableKits={availableKits}
                  activeKitName={activeKitName}
                  soundPresets={soundPresets}
                  ws={ws}
                  selectedSoundId={selectedSoundId}
                />
              )}
            </div>
          </div>
      </div>

      {/* Library Sidebar (Managed internally for mobile/desktop) */}
      <LibrarySidebar 
        availableKits={availableKits}
        activeKitName={activeKitName}
        soundPresets={soundPresets}
        ws={ws}
        selectedSoundId={selectedSoundId}
        isOpen={isLibraryOpen}
        onClose={() => setIsLibraryOpen(false)}
      />

      {/* Bottom Navigation - Mobile only */}
      <div className="lg:hidden fixed bottom-0 left-0 right-0 h-16 bg-card/80 backdrop-blur-xl border-t border-border flex items-center justify-around px-4 z-40">
        <BottomNavItem icon={<Sparkle size={20} />} label="Live" active={view === 'performance'} onClick={() => setView('performance')} />
        <BottomNavItem icon={<Books size={20} />} label="Library" active={view === 'library'} onClick={() => setView('library')} />
        <BottomNavItem icon={<Faders size={20} />} label="Editor" active={view === 'editor'} onClick={() => setView('editor')} />
        <BottomNavItem icon={<House size={20} />} label="System" active={view === 'dashboard'} onClick={() => setView('dashboard')} />
      </div>

      {isMobileMenuOpen && (
        <div className="fixed inset-0 z-50 lg:hidden flex">
          <div className="fixed inset-0 bg-background/80 backdrop-blur-sm" onClick={closeMenu} />
          <nav className="relative w-80 h-full bg-card border-r border-border flex flex-col animate-in slide-in-from-left duration-300">
            <div className="absolute top-4 right-4">
               <button
                 onClick={closeMenu}
                 aria-label="Close navigation menu"
                 className="p-2 hover:bg-muted rounded-full focus:outline-none focus-visible:ring-2 focus-visible:ring-primary"
               >
                 <X size={24} />
               </button>
            </div>
            <SidebarContent 
              view={view} setView={(v: View) => { setView(v); closeMenu(); }} 
              status={status} midiPort={midiPort} audioDevice={audioDevice} isMidiActive={isMidiFlashing}
            />
          </nav>
        </div>
      )}

      {/* Toast Container */}
      <div className="fixed bottom-4 right-4 z-[999] flex flex-col gap-2 max-w-sm w-full pointer-events-none">
        {toasts.map(toast => (
          <div
            key={toast.id}
            className={cn(
              "p-4 rounded-xl shadow-lg border backdrop-blur-md transition-all duration-300 animate-in slide-in-from-right-5 pointer-events-auto flex items-center gap-3",
              toast.type === 'success' ? "bg-emerald-500/10 border-emerald-500/30 text-emerald-300" :
              toast.type === 'error' ? "bg-rose-500/10 border-rose-500/30 text-rose-300" :
              "bg-primary/10 border-primary/30 text-primary-foreground"
            )}
          >
            {toast.type === 'success' && <Sparkle weight="fill" className="text-emerald-400 shrink-0" size={16} />}
            {toast.type === 'error' && <Warning weight="fill" className="text-rose-400 shrink-0" size={16} />}
            {toast.type === 'info' && <Pulse weight="fill" className="text-primary shrink-0" size={16} />}
            <span className="text-xs font-bold leading-normal">{toast.message}</span>
            <button
              onClick={() => setToasts(prev => prev.filter(t => t.id !== toast.id))}
              className="ml-auto p-1 hover:bg-white/10 rounded transition-colors text-current opacity-70 hover:opacity-100"
            >
              <X size={12} />
            </button>
          </div>
        ))}
      </div>
    </div>
  )
}

function SidebarContent({ view, setView, status, midiPort, audioDevice, isMidiActive }: any) {
  return (
    <>
      <div className="p-6 flex items-center gap-3">
        <div className="w-8 h-8 rounded-lg bg-primary flex items-center justify-center">
          <Faders className="text-primary-foreground" size={20} weight="bold" />
        </div>
        <h1 className="font-bold text-xl tracking-tight">drummr</h1>
      </div>

      <div className="flex-1 px-3 space-y-1">
        <NavItem icon={<Sparkle size={20} />} label="Performance" active={view === 'performance'} onClick={() => setView('performance')} />
        <NavItem icon={<Books size={20} />} label="Kit Library" active={view === 'library'} onClick={() => setView('library')} />
        <NavItem icon={<ListDashes size={20} />} label="MIDI Mapping" active={view === 'mapping'} onClick={() => setView('mapping')} />
        <NavItem icon={<Faders size={20} />} label="Kit Editor" active={view === 'editor'} onClick={() => setView('editor')} />
        <NavItem icon={<House size={20} />} label="Dashboard" active={view === 'dashboard'} onClick={() => setView('dashboard')} />
      </div>

      <div className="p-4 border-t border-border space-y-3">
        <div className="flex items-center justify-between text-xs px-2">
          <span className={cn(
            "flex items-center gap-2 font-medium transition-colors",
            status === 'Connected' ? "text-emerald-500" : (status === 'Connecting' ? "text-amber-500" : "text-Disconnected")
          )}>
            {status === 'Connected' ? <WifiHigh weight="bold" /> : <WifiSlash weight="bold" />}
            {status}
          </span>
          {isMidiActive && <Pulse size={14} className="text-emerald-400 animate-pulse" />}
        </div>
        <div className="space-y-1">
           <div className="flex items-center gap-2 text-[10px] text-muted-foreground px-2">
              <Cpu size={12} /><span className="truncate">{midiPort}</span>
           </div>
           <div className="flex items-center gap-2 text-[10px] text-muted-foreground px-2">
              <SpeakerHigh size={12} /><span className="truncate">{audioDevice}</span>
           </div>
        </div>
      </div>
    </>
  )
}

function NavItem({ icon, label, active, onClick }: { icon: React.ReactNode, label: string, active: boolean, onClick: () => void }) {
  return (
    <button onClick={onClick} className={cn(
      "flex items-center gap-3 w-full px-4 py-2.5 rounded-lg text-sm font-medium transition-all duration-200 outline-none",
      active ? "bg-primary text-primary-foreground shadow-lg shadow-primary/10" : "text-muted-foreground hover:bg-muted hover:text-foreground"
    )}>
      {icon}
      {label}
    </button>
  )
}

function BottomNavItem({ icon, label, active, onClick }: { icon: React.ReactNode, label: string, active: boolean, onClick: () => void }) {
  return (
    <button onClick={onClick} className={cn(
      "flex flex-col items-center justify-center gap-1 flex-1 h-full transition-colors outline-none",
      active ? "text-primary" : "text-muted-foreground hover:text-foreground"
    )}>
      {icon}
      <span className="text-[10px] font-black uppercase tracking-widest">{label}</span>
      {active && <div className="w-1 h-1 rounded-full bg-primary absolute bottom-1" />}
    </button>
  )
}

function DashboardView({ 
  ws, midiPort, audioDevice, audioHost, availableHosts, bufferSize, 
  availableMidi, availableAudio, lastMidi, isMidiActive 
}: any) {
  return (
    <div className="space-y-10">
      <div className="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-5 gap-6">
        <Card title="MIDI Input" value={midiPort} icon={<Cpu size={20} className={cn("transition-colors", isMidiActive && "text-emerald-400")} />} />
        <Card title="Audio Output" value={audioDevice} icon={<SpeakerHigh size={20} />} />
        <Card title="Backend" value={audioHost || "Default"} icon={<Faders size={20} />} />
        <Card title="Buffer" value={`${bufferSize}`} icon={<Clock size={20} />} />
        <Card title="Last Note" value={lastMidi ? `${lastMidi.note} (${lastMidi.vel})` : "-"} icon={<Pulse size={20} className={cn("transition-colors", isMidiActive && "text-emerald-400")} />} />
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-8">
        <section className="bg-card/30 border border-border rounded-3xl overflow-hidden">
          <header className="p-6 border-b border-border flex items-center justify-between">
            <h3 className="font-bold flex items-center gap-2"><Faders size={20} className="text-muted-foreground" />Audio Backend</h3>
            <button onClick={() => ws?.send('LIST_HOSTS')} className="text-xs text-primary hover:underline">Refresh</button>
          </header>
          <div className="divide-y divide-border">
            {availableHosts.map((name: string, i: number) => (
              <button key={i} onClick={() => ws?.send(`SELECT_HOST:${name}`)} className={cn("w-full text-left p-4 text-sm transition-colors flex items-center justify-between group", audioHost === name ? "bg-primary/5 text-primary" : "hover:bg-muted")}>
                <span>{name}</span>
                {audioHost === name && <div className="w-2 h-2 rounded-full bg-primary" />}
              </button>
            ))}
          </div>
          <div className="p-6 border-t border-border bg-card/20">
             <label className="text-[10px] font-black uppercase tracking-widest text-muted-foreground mb-3 block">Latency (Buffer Size)</label>
             <div className="grid grid-cols-4 gap-2">
                {[32, 64, 128, 256, 512].map(size => (
                  <button 
                    key={size}
                    onClick={() => ws?.send(`SET_BUFFER:${size}`)}
                    className={cn(
                      "py-2 rounded-lg text-xs font-bold transition-all border",
                      bufferSize === size ? "bg-primary border-primary text-primary-foreground shadow-lg shadow-primary/20" : "bg-background/50 border-border text-muted-foreground hover:border-primary/40"
                    )}
                  >
                    {size}
                  </button>
                ))}
             </div>
             <p className="text-[10px] text-muted-foreground mt-4 italic leading-relaxed">
               Note: Buffer changes take effect when you select an audio device or restart the engine.
             </p>
          </div>
        </section>

        <section className="bg-card/30 border border-border rounded-3xl overflow-hidden">
          <header className="p-6 border-b border-border flex items-center justify-between">
            <h3 className="font-bold flex items-center gap-2"><Cpu size={20} className="text-muted-foreground" />MIDI Inputs</h3>
            <button onClick={() => ws?.send('LIST_MIDI')} className="text-xs text-primary hover:underline">Refresh</button>
          </header>
          <div className="divide-y divide-border">
            {availableMidi.map((name: string, i: number) => (
              <button key={i} onClick={() => ws?.send(`SELECT_MIDI:${i}`)} className={cn("w-full text-left p-4 text-sm transition-colors flex items-center justify-between group", midiPort === name ? "bg-primary/5 text-primary" : "hover:bg-muted")}>
                <span>{name}</span>
                {midiPort === name && <div className="w-2 h-2 rounded-full bg-primary" />}
              </button>
            ))}
            {availableMidi.length === 0 && <p className="p-8 text-center text-sm text-muted-foreground italic">No MIDI devices detected</p>}
          </div>
        </section>

        <section className="bg-card/30 border border-border rounded-3xl overflow-hidden">
          <header className="p-6 border-b border-border flex items-center justify-between">
            <h3 className="font-bold flex items-center gap-2"><SpeakerHigh size={20} className="text-muted-foreground" />Audio Outputs</h3>
            <button onClick={() => ws?.send('LIST_AUDIO')} className="text-xs text-primary hover:underline">Refresh</button>
          </header>
          <div className="divide-y divide-border">
            {availableAudio.map((name: string, i: number) => (
              <button key={i} onClick={() => ws?.send(`SELECT_AUDIO:${i}`)} className={cn("w-full text-left p-4 text-sm transition-colors flex items-center justify-between group", audioDevice === name ? "bg-primary/5 text-primary" : "hover:bg-muted")}>
                <span>{name}</span>
                {audioDevice === name && <div className="w-2 h-2 rounded-full bg-primary" />}
              </button>
            ))}
            {availableAudio.length === 0 && <p className="p-8 text-center text-sm text-muted-foreground italic">No audio devices detected</p>}
          </div>
        </section>
      </div>
    </div>
  )
}
