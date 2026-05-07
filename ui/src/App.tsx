import { useState, useEffect } from 'react'
import { House, ListDashes, Faders, WifiHigh, WifiSlash, HardDrive, SpeakerHigh, CircuitBoard, List as ListIcon, X } from "@phosphor-icons/react"
import { clsx, type ClassValue } from 'clsx'
import { twMerge } from 'tailwind-merge'

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

import MappingView from './views/MappingView'
import KitEditorView from './views/KitEditorView'

type View = 'dashboard' | 'mapping' | 'editor';

export default function App() {
  const [view, setView] = useState<View>('dashboard');
  const [status, setStatus] = useState<'Connecting' | 'Connected' | 'Disconnected'>('Connecting');
  const [ws, setWs] = useState<WebSocket | null>(null);
  const [midiPort, setMidiPort] = useState<string>('None');
  const [audioDevice, setAudioDevice] = useState<string>('None');
  const [isMobileMenuOpen, setIsMobileMenuOpen] = useState(false);
  
  const [availableMidi, setAvailableMidi] = useState<string[]>([]);
  const [availableAudio, setAvailableAudio] = useState<string[]>([]);

  useEffect(() => {
    const socket = new WebSocket(`ws://${window.location.hostname}:8080`);
    
    socket.onopen = () => {
      setStatus('Connected');
      socket.send('LIST_MIDI');
      socket.send('LIST_AUDIO');
    };
    socket.onclose = () => setStatus('Disconnected');
    socket.onmessage = (event) => {
      const data = event.data as string;
      if (data.startsWith('PORT: ')) {
        setMidiPort(data.replace('PORT: ', ''));
      } else if (data.startsWith('AUDIO_DEVICE: ')) {
        setAudioDevice(data.replace('AUDIO_DEVICE: ', ''));
      } else if (data.startsWith('LIST_MIDI: ')) {
        setAvailableMidi(data.replace('LIST_MIDI: ', '').split(',').filter(Boolean));
      } else if (data.startsWith('LIST_AUDIO: ')) {
        setAvailableAudio(data.replace('LIST_AUDIO: ', '').split(',').filter(Boolean));
      }
    };

    setWs(socket);
    return () => socket.close();
  }, []);

  const closeMenu = () => setIsMobileMenuOpen(false);

  return (
    <div className="flex h-screen w-full overflow-hidden bg-background text-foreground select-none">
      {/* Sidebar - Desktop */}
      <nav className="hidden lg:flex flex-col w-64 border-r border-border bg-card/50 backdrop-blur-xl">
        <SidebarContent 
          view={view} 
          setView={setView} 
          status={status} 
          midiPort={midiPort} 
          audioDevice={audioDevice} 
        />
      </nav>

      {/* Sidebar - Mobile Overlay */}
      {isMobileMenuOpen && (
        <div className="fixed inset-0 z-50 lg:hidden flex">
          <div className="fixed inset-0 bg-background/80 backdrop-blur-sm" onClick={closeMenu} />
          <nav className="relative w-80 h-full bg-card border-r border-border flex flex-col animate-in slide-in-from-left duration-300">
            <div className="absolute top-4 right-4">
               <button onClick={closeMenu} className="p-2 hover:bg-muted rounded-full">
                  <X size={24} />
               </button>
            </div>
            <SidebarContent 
              view={view} 
              setView={(v) => { setView(v); closeMenu(); }} 
              status={status} 
              midiPort={midiPort} 
              audioDevice={audioDevice} 
            />
          </nav>
        </div>
      )}

      {/* Main Content */}
      <main className="flex-1 overflow-auto relative">
        <header className="h-16 border-b border-border flex items-center justify-between px-4 lg:px-8 bg-background/50 backdrop-blur-md sticky top-0 z-10">
          <div className="flex items-center gap-4">
            <button 
              onClick={() => setIsMobileMenuOpen(true)}
              className="lg:hidden p-2 hover:bg-muted rounded-lg"
            >
              <ListIcon size={24} />
            </button>
            <h2 className="text-sm font-medium uppercase tracking-widest text-muted-foreground">
              {view.replace('_', ' ')}
            </h2>
          </div>
        </header>

        <div className="p-4 lg:p-8 max-w-7xl mx-auto">
          {view === 'dashboard' && (
            <DashboardView 
              ws={ws} 
              midiPort={midiPort} 
              audioDevice={audioDevice}
              availableMidi={availableMidi}
              availableAudio={availableAudio}
            />
          )}
          {view === 'mapping' && <MappingView ws={ws} />}
          {view === 'editor' && <KitEditorView ws={ws} />}
        </div>
      </main>
    </div>
  )
}

function SidebarContent({ view, setView, status, midiPort, audioDevice }: any) {
  return (
    <>
      <div className="p-6 flex items-center gap-3">
        <div className="w-8 h-8 rounded-lg bg-primary flex items-center justify-center">
          <Faders className="text-primary-foreground" size={20} weight="bold" />
        </div>
        <h1 className="font-bold text-xl tracking-tight">drummr</h1>
      </div>

      <div className="flex-1 px-3 space-y-1">
        <NavItem 
          icon={<House size={20} />} 
          label="Dashboard" 
          active={view === 'dashboard'} 
          onClick={() => setView('dashboard')} 
        />
        <NavItem 
          icon={<ListDashes size={20} />} 
          label="MIDI Mapping" 
          active={view === 'mapping'} 
          onClick={() => setView('mapping')} 
        />
        <NavItem 
          icon={<Faders size={20} />} 
          label="Kit Editor" 
          active={view === 'editor'} 
          onClick={() => setView('editor')} 
        />
      </div>

      <div className="p-4 border-t border-border space-y-3">
        <div className="flex items-center justify-between text-xs px-2">
          <span className={cn(
            "flex items-center gap-2 font-medium transition-colors",
            status === 'Connected' ? "text-emerald-500" : "text-destructive"
          )}>
            {status === 'Connected' ? <WifiHigh weight="bold" /> : <WifiSlash weight="bold" />}
            {status}
          </span>
        </div>
        <div className="space-y-1">
           <div className="flex items-center gap-2 text-[10px] text-muted-foreground px-2">
              <CircuitBoard size={12} />
              <span className="truncate">{midiPort}</span>
           </div>
           <div className="flex items-center gap-2 text-[10px] text-muted-foreground px-2">
              <SpeakerHigh size={12} />
              <span className="truncate">{audioDevice}</span>
           </div>
        </div>
      </div>
    </>
  )
}

function NavItem({ icon, label, active, onClick }: { icon: React.ReactNode, label: string, active: boolean, onClick: () => void }) {
  return (
    <button 
      onClick={onClick}
      className={cn(
        "flex items-center gap-3 w-full px-4 py-2.5 rounded-lg text-sm font-medium transition-all duration-200 outline-none",
        active 
          ? "bg-primary text-primary-foreground shadow-lg shadow-primary/10" 
          : "text-muted-foreground hover:bg-muted hover:text-foreground"
      )}
    >
      {icon}
      {label}
    </button>
  )
}

function DashboardView({ ws, midiPort, audioDevice, availableMidi, availableAudio }: any) {
  return (
    <div className="space-y-10">
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
        <Card title="MIDI Device" value={midiPort} icon={<CircuitBoard size={20} />} />
        <Card title="Audio Output" value={audioDevice} icon={<SpeakerHigh size={20} />} />
        <Card title="System" value="OK" icon={<WifiHigh size={20} />} />
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-8">
        <section className="bg-card/30 border border-border rounded-3xl overflow-hidden">
          <header className="p-6 border-b border-border flex items-center justify-between">
            <h3 className="font-bold flex items-center gap-2">
              <CircuitBoard size={20} className="text-muted-foreground" />
              MIDI Inputs
            </h3>
            <button onClick={() => ws?.send('LIST_MIDI')} className="text-xs text-primary hover:underline">Refresh</button>
          </header>
          <div className="divide-y divide-border">
            {availableMidi.map((name, i) => (
              <button
                key={i}
                onClick={() => ws?.send(`SELECT_MIDI:${i}`)}
                className={cn(
                  "w-full text-left p-4 text-sm transition-colors flex items-center justify-between group",
                  midiPort === name ? "bg-primary/5 text-primary" : "hover:bg-muted"
                )}
              >
                <span>{name}</span>
                {midiPort === name && <div className="w-2 h-2 rounded-full bg-primary" />}
              </button>
            ))}
            {availableMidi.length === 0 && <p className="p-8 text-center text-sm text-muted-foreground italic">No MIDI devices detected</p>}
          </div>
        </section>

        <section className="bg-card/30 border border-border rounded-3xl overflow-hidden">
          <header className="p-6 border-b border-border flex items-center justify-between">
            <h3 className="font-bold flex items-center gap-2">
              <SpeakerHigh size={20} className="text-muted-foreground" />
              Audio Outputs
            </h3>
            <button onClick={() => ws?.send('LIST_AUDIO')} className="text-xs text-primary hover:underline">Refresh</button>
          </header>
          <div className="divide-y divide-border">
            {availableAudio.map((name, i) => (
              <button
                key={i}
                onClick={() => ws?.send(`SELECT_AUDIO:${i}`)}
                className={cn(
                  "w-full text-left p-4 text-sm transition-colors flex items-center justify-between group",
                  audioDevice === name ? "bg-primary/5 text-primary" : "hover:bg-muted"
                )}
              >
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

function Card({ title, value, icon }: any) {
  return (
    <div className="bg-card border border-border p-6 rounded-2xl flex items-start gap-4">
      <div className="w-10 h-10 rounded-xl bg-muted flex items-center justify-center text-muted-foreground">
        {icon}
      </div>
      <div className="space-y-1">
        <span className="text-xs font-semibold text-muted-foreground uppercase tracking-wider">{title}</span>
        <p className="text-lg font-bold truncate max-w-[180px]">{value}</p>
      </div>
    </div>
  )
}
