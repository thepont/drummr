import { useState, useEffect, useCallback } from 'react'
import { Plus, List, Target, CheckCircle, WarningCircle } from "@phosphor-icons/react"
import { clsx, type ClassValue } from 'clsx'
import { twMerge } from 'tailwind-merge'

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

interface PadProps {
  id: string;
  name: string;
  isActive: boolean;
  midiNote?: number;
  isLearning: boolean;
  onClick: () => void;
}

function Pad({ name, isActive, midiNote, isLearning, onClick }: PadProps) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "relative aspect-square rounded-2xl border-2 transition-all duration-75 flex flex-col items-center justify-center gap-2 group",
        isActive 
          ? "bg-primary border-primary shadow-[0_0_20px_rgba(255,255,255,0.3)] scale-95" 
          : "bg-card/50 border-border hover:border-muted-foreground/50",
        isLearning && "border-amber-500 animate-pulse bg-amber-500/10"
      )}
    >
      <span className={cn(
        "text-xs font-bold uppercase tracking-wider",
        isActive ? "text-primary-foreground" : "text-muted-foreground",
        isLearning && "text-amber-500"
      )}>
        {name}
      </span>
      {midiNote !== undefined && (
        <span className={cn(
          "text-[10px] font-mono",
          isActive ? "text-primary-foreground/70" : "text-muted-foreground/50",
          isLearning && "text-amber-500/70"
        )}>
          Note {midiNote}
        </span>
      )}
      
      {/* Learning overlay */}
      {isLearning && (
        <div className="absolute inset-0 flex items-center justify-center bg-background/40 backdrop-blur-[1px] rounded-2xl">
          <Target size={24} className="text-amber-500 animate-spin-slow" />
        </div>
      )}

      {/* Decorative indicator */}
      <div className={cn(
        "absolute top-3 right-3 w-1.5 h-1.5 rounded-full transition-colors",
        isActive ? "bg-primary-foreground" : "bg-muted",
        isLearning && "bg-amber-500"
      )} />
    </button>
  )
}

export default function MappingView({ ws }: { ws: WebSocket | null }) {
  const [activeNotes, setActiveNotes] = useState<Set<number>>(new Set());
  const [learningRoleId, setLearningRoleId] = useState<string | null>(null);
  
  // Example drum roles (In Phase 2.4 we'll load these from the backend)
  const [roles, setRoles] = useState([
    { id: 'kick', name: 'Kick', note: 36 },
    { id: 'snare', name: 'Snare', note: 38 },
    { id: 'hat_closed', name: 'Closed Hat', note: 42 },
    { id: 'hat_open', name: 'Open Hat', note: 46 },
    { id: 'tom_low', name: 'Low Tom', note: 41 },
    { id: 'tom_mid', name: 'Mid Tom', note: 43 },
    { id: 'tom_high', name: 'High Tom', note: 45 },
    { id: 'crash', name: 'Crash', note: 49 },
    { id: 'ride', name: 'Ride', note: 51 },
    { id: 'rim', name: 'Rimshot', note: 37 },
    { id: 'clap', name: 'Clap', note: 39 },
    { id: 'perc_1', name: 'Perc 1', note: 54 },
  ]);

  const updateRoleNote = useCallback((id: string, newNote: number) => {
    setRoles(prev => prev.map(r => r.id === id ? { ...r, note: newNote } : r));
    setLearningRoleId(null);
  }, []);

  useEffect(() => {
    if (!ws) return;

    const handleMessage = (event: MessageEvent) => {
      const data = event.data as string;
      if (data.startsWith('MIDI: ')) {
        const parts = data.replace('MIDI: ', '').split(',');
        const note = parseInt(parts[0]);
        const velocity = parseInt(parts[1]);

        if (velocity > 0) {
          // Trigger visual feedback
          setActiveNotes(prev => new Set(prev).add(note));
          setTimeout(() => {
            setActiveNotes(prev => {
              const next = new Set(prev);
              next.delete(note);
              return next;
            });
          }, 100);

          // Handle MIDI Learn
          if (learningRoleId) {
            updateRoleNote(learningRoleId, note);
          }
        }
      }
    };

    ws.addEventListener('message', handleMessage);
    return () => ws.removeEventListener('message', handleMessage);
  }, [ws, learningRoleId, updateRoleNote]);

  return (
    <div className="space-y-8 animate-in fade-in slide-in-from-bottom-4 duration-500">
      <div className="flex items-center justify-between">
        <div>
          <h3 className="text-2xl font-bold tracking-tight">Drum Map</h3>
          <p className="text-muted-foreground mt-1">Assign your MIDI triggers to drum roles.</p>
        </div>
        
        <div className="flex gap-2">
          {learningRoleId && (
            <button 
              onClick={() => setLearningRoleId(null)}
              className="flex items-center gap-2 px-4 py-2 bg-destructive/10 text-destructive rounded-lg hover:bg-destructive/20 transition-colors text-sm font-medium"
            >
              Cancel Learning
            </button>
          )}
          <button className="flex items-center gap-2 px-4 py-2 bg-secondary text-secondary-foreground rounded-lg hover:bg-muted transition-colors text-sm font-medium">
            <Plus size={18} />
            Add Role
          </button>
        </div>
      </div>

      {/* Learning Banner */}
      {learningRoleId && (
        <div className="bg-amber-500/10 border border-amber-500/20 p-4 rounded-xl flex items-center gap-4 text-amber-500 animate-in zoom-in-95 duration-200">
          <Target size={24} className="animate-spin-slow" />
          <div className="flex-1">
            <p className="text-sm font-bold uppercase tracking-wider">Learning Mode Active</p>
            <p className="text-sm opacity-80">Hit a physical pad to assign it to <span className="underline font-bold">{roles.find(r => r.id === learningRoleId)?.name}</span>.</p>
          </div>
        </div>
      )}

      {/* The Grid */}
      <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-6 gap-4">
        {roles.map((role) => (
          <Pad 
            key={role.id}
            id={role.id}
            name={role.name}
            midiNote={role.note}
            isActive={activeNotes.has(role.note)}
            isLearning={learningRoleId === role.id}
            onClick={() => setLearningRoleId(role.id)}
          />
        ))}
      </div>

      {/* Manual List Section */}
      <div className="mt-12 border border-border rounded-2xl bg-card/30 overflow-hidden">
        <div className="p-6 border-b border-border flex items-center justify-between">
          <div className="flex items-center gap-2">
            <List size={20} className="text-muted-foreground" />
            <h4 className="font-semibold">Role List</h4>
          </div>
        </div>
        <div className="divide-y divide-border">
          {roles.map(role => (
            <div key={role.id} className="flex items-center justify-between p-4 hover:bg-muted/50 transition-colors group">
              <div className="flex items-center gap-3">
                <span className="text-sm font-medium">{role.name}</span>
                {role.note === undefined && <WarningCircle className="text-destructive" size={16} />}
              </div>
              <div className="flex items-center gap-4">
                <span className={cn(
                  "text-[10px] font-mono px-2 py-1 rounded transition-colors",
                  role.note !== undefined ? "bg-muted text-muted-foreground" : "bg-destructive/10 text-destructive"
                )}>
                  {role.note !== undefined ? `NOTE ${role.note}` : "UNMAPPED"}
                </span>
                <button 
                  onClick={() => setLearningRoleId(role.id)}
                  className="text-xs text-primary opacity-0 group-hover:opacity-100 transition-opacity hover:underline flex items-center gap-1"
                >
                  <Target size={14} />
                  Learn
                </button>
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  )
}
