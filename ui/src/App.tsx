import { useState, useEffect } from 'react'
import { House, ListDashes, Knobs, WifiHigh, WifiSlash, HardDrive } from "@phosphor-icons/react"
import { clsx, type ClassValue } from 'clsx'
import { twMerge } from 'tailwind-merge'

// Utility for tailwind classes
function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

type View = 'dashboard' | 'mapping' | 'editor';

export default function App() {
  const [view, setView] = useState<View>('dashboard');
  const [status, setStatus] = useState<'Connecting' | 'Connected' | 'Disconnected'>('Connecting');
  const [ws, setWs] = useState<WebSocket | null>(null);
  const [midiPort, setMidiPort] = useState<string>('None');

  useEffect(() => {
    const socket = new WebSocket(`ws://${window.location.hostname}:8080`);
    
    socket.onopen = () => setStatus('Connected');
    socket.onclose = () => setStatus('Disconnected');
    socket.onmessage = (event) => {
      const data = event.data as string;
      if (data.startsWith('PORT: ')) {
        setMidiPort(data.replace('PORT: ', ''));
      }
    };

    setWs(socket);
    return () => socket.close();
  }, []);

  return (
    <div className="flex h-screen w-full overflow-hidden bg-background text-foreground select-none">
      {/* Sidebar */}
      <nav className="flex flex-col w-64 border-r border-border bg-card/50 backdrop-blur-xl">
        <div className="p-6 flex items-center gap-3">
          <div className="w-8 h-8 rounded-lg bg-primary flex items-center justify-center">
            <Knobs className="text-primary-foreground" size={20} weight="bold" />
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
            icon={<Knobs size={20} />} 
            label="Kit Editor" 
            active={view === 'editor'} 
            onClick={() => setView('editor')} 
          />
        </div>

        {/* Status Area */}
        <div className="p-4 border-t border-border space-y-3">
          <div className="flex items-center justify-between text-xs px-2">
            <span className="text-muted-foreground flex items-center gap-2">
              {status === 'Connected' ? <WifiHigh className="text-emerald-500" /> : <WifiSlash className="text-destructive" />}
              {status}
            </span>
            <span className="text-muted-foreground flex items-center gap-2">
              <HardDrive />
              {midiPort}
            </span>
          </div>
        </div>
      </nav>

      {/* Main Content */}
      <main className="flex-1 overflow-auto relative">
        <header className="h-16 border-b border-border flex items-center px-8 bg-background/50 backdrop-blur-md sticky top-0 z-10">
          <h2 className="text-sm font-medium uppercase tracking-widest text-muted-foreground">
            {view.replace('_', ' ')}
          </h2>
        </header>

        <div className="p-8 max-w-7xl mx-auto">
          {view === 'dashboard' && <DashboardView ws={ws} midiPort={midiPort} />}
          {view === 'mapping' && <div className="text-center py-20 text-muted-foreground italic">MIDI Mapping View Coming Soon</div>}
          {view === 'editor' && <div className="text-center py-20 text-muted-foreground italic">Kit Editor View Coming Soon</div>}
        </div>
      </main>
    </div>
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

function DashboardView({ ws, midiPort }: { ws: WebSocket | null, midiPort: string }) {
  return (
    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
      <Card title="MIDI Device" value={midiPort} />
      <Card title="System Load" value="2.4%" />
      <Card title="Audio Buffer" value="128 Samples" />
      
      <div className="col-span-full mt-10 p-12 border-2 border-dashed border-border rounded-3xl flex flex-col items-center justify-center text-center space-y-4">
        <div className="w-16 h-16 rounded-full bg-muted flex items-center justify-center">
          <Knobs size={32} className="text-muted-foreground" />
        </div>
        <div>
          <h3 className="text-xl font-semibold">Welcome to Drummr</h3>
          <p className="text-muted-foreground max-w-md mt-2">
            Your low-latency MIDI drum engine is ready. Use the sidebar to map your pads or edit your kit sounds.
          </p>
        </div>
        <button 
          onClick={() => (ws?.send('LIST_MIDI'))}
          className="bg-primary text-primary-foreground px-6 py-2 rounded-full font-medium hover:scale-105 active:scale-95 transition-all"
        >
          Refresh MIDI List
        </button>
      </div>
    </div>
  )
}

function Card({ title, value }: { title: string, value: string }) {
  return (
    <div className="bg-card border border-border p-6 rounded-2xl space-y-1">
      <span className="text-xs font-semibold text-muted-foreground uppercase tracking-wider">{title}</span>
      <p className="text-2xl font-bold">{value}</p>
    </div>
  )
}
