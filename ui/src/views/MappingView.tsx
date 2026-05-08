import { useState, useEffect, useCallback, useMemo } from 'react'
import { Plus, List, Target, MagnifyingGlass, Trash, FloppyDisk } from "@phosphor-icons/react"
import { clsx, type ClassValue } from 'clsx'
import { twMerge } from 'tailwind-merge'

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

const PRESET_ROLES = [
  { id: 0, name: "Kick Drum" },
  { id: 1, name: "Snare Drum" },
  { id: 2, name: "Hi-Hat" },
  { id: 3, name: "Tom 1" },
  { id: 4, name: "Tom 2" },
  { id: 5, name: "Cymbal" },
];

interface Role {
  slot: number; // The sound slot index
  name: string; // The display name
  note: number;
}

interface PadProps {
  id: string;
  name: string;
  isActive: boolean;
  midiNote?: number;
  isLearning: boolean;
  onClick: () => void;
}

function Pad({ name, isActive, midiNote, isLearning, onClick, id }: PadProps) {
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
      <div className="flex flex-col items-center">
        <span className="text-[9px] font-black text-muted-foreground/60 uppercase tracking-tighter mb-0.5">Slot {id}</span>
        <span className={cn(
          "text-xs font-bold uppercase tracking-wider text-center px-2 line-clamp-2",
          isActive ? "text-primary-foreground" : "text-foreground",
          isLearning && "text-amber-500"
        )}>
          {name}
        </span>
      </div>
      {midiNote !== undefined && (
        <span className={cn(
          "text-[10px] font-mono",
          isActive ? "text-primary-foreground/70" : "text-muted-foreground/50",
          isLearning && "text-amber-500/70"
        )}>
          Note {midiNote}
        </span>
      )}
      
      {isLearning && (
        <div className="absolute inset-0 flex items-center justify-center bg-background/40 backdrop-blur-[1px] rounded-2xl">
          <Target size={24} className="text-amber-500 animate-spin-slow" />
        </div>
      )}

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
  const [learningSlot, setLearningSlot] = useState<number | null>(null);
  const [searchQuery, setSearchQuery] = useState('');
  const [roles, setRoles] = useState<Role[]>([]);
  const [isSaving, setIsSaving] = useState(false);
  const [hasChanges, setHasChanges] = useState(false);
  const [isLoaded, setIsLoaded] = useState(false);

  const filteredRoles = useMemo(() => {
    return roles.filter(r => r.name.toLowerCase().includes(searchQuery.toLowerCase()));
  }, [roles, searchQuery]);

  const updateRoleNote = useCallback((slot: number, newNote: number) => {
    setRoles(prev => prev.map(r => r.slot === slot ? { ...r, note: newNote } : r));
    setLearningSlot(null);
    setHasChanges(true);
    
    // Live update
    if (ws && ws.readyState === WebSocket.OPEN) {
      ws.send(`UPDATE_MAPPING:${slot}:${newNote}`);
    }
  }, [ws]);

  const deleteRole = (slot: number) => {
    setRoles(prev => prev.filter(r => r.slot !== slot));
    setHasChanges(true);
  };

  const addRole = (slot: number, name: string) => {
    setRoles(prev => [...prev, { slot, name, note: 0 }]);
    setLearningSlot(slot);
    setHasChanges(true);
  };

  const handleSave = () => {
    if (!ws) return;
    setIsSaving(true);
    ws.send(`SAVE_MAPPING:${JSON.stringify(roles)}`);
    setTimeout(() => {
      setIsSaving(false);
      setHasChanges(false);
    }, 500);
  };

  useEffect(() => {
    if (!ws) return;

    if (!isLoaded) {
      ws.send('GET_MAPPING');
    }

    const handleMessage = (event: MessageEvent) => {
      const data = event.data as string;
      if (data.startsWith('MIDI: ')) {
        const parts = data.replace('MIDI: ', '').split(',');
        const note = parseInt(parts[0]);
        const velocity = parseInt(parts[1]);

        if (isNaN(note) || isNaN(velocity)) return;

        if (velocity > 0) {
          setActiveNotes(prev => new Set(prev).add(note));
          
          // Flash duration: remove note after 100ms
          setTimeout(() => {
            setActiveNotes(prev => {
              const next = new Set(prev);
              next.delete(note);
              return next;
            });
          }, 100);

          if (learningSlot !== null) {
            updateRoleNote(learningSlot, note);
          }
        }
      } else if (data.startsWith('KIT: ')) {
        // When the kit changes, we need to refresh the mapping to get the new sound names
        ws.send('GET_MAPPING');
      } else if (data.startsWith('MAPPING: ')) {
        if (!hasChanges) {
          try {
            const mapping = JSON.parse(data.replace('MAPPING: ', '')) as Role[];
            setRoles(mapping);
            setIsLoaded(true);
          } catch (e) {
            console.error('Failed to parse mapping:', e);
          }
        }
      }
    };

    ws.addEventListener('message', handleMessage);
    return () => ws.removeEventListener('message', handleMessage);
  }, [ws, learningSlot, updateRoleNote, hasChanges, isLoaded]);

  return (
    <div className="space-y-8 animate-in fade-in slide-in-from-bottom-4 duration-500 pb-20">
      <div className="flex flex-col md:flex-row md:items-center justify-between gap-4">
        <div>
          <h3 className="text-2xl font-bold tracking-tight">Drum Map</h3>
          <p className="text-muted-foreground mt-1">Assign your MIDI triggers to drum roles.</p>
        </div>
        
        <div className="flex flex-wrap gap-2">
          {hasChanges && (
            <button 
              onClick={handleSave}
              disabled={isSaving}
              className={cn(
                "flex items-center gap-2 px-4 py-2 rounded-lg transition-all text-sm font-medium",
                isSaving ? "bg-muted text-muted-foreground animate-pulse" : "bg-primary text-primary-foreground hover:scale-105 active:scale-95"
              )}
            >
              <FloppyDisk size={18} />
              {isSaving ? 'Saving...' : 'Save Changes'}
            </button>
          )}
          
          <div className="relative group">
            <button className="flex items-center gap-2 px-4 py-2 bg-secondary text-secondary-foreground rounded-lg hover:bg-muted transition-colors text-sm font-medium">
              <Plus size={18} />
              Add Preset Role
            </button>
            <div className="absolute right-0 top-full mt-2 w-48 bg-card border border-border rounded-xl shadow-xl opacity-0 invisible group-hover:opacity-100 group-hover:visible transition-all z-50 overflow-hidden">
              <div className="max-h-60 overflow-y-auto divide-y divide-border">
                {PRESET_ROLES.map(role => (
                  <button 
                    key={role.id}
                    onClick={() => addRole(role.id, role.name)}
                    className="w-full text-left px-4 py-2.5 text-xs hover:bg-muted transition-colors"
                  >
                    {role.name}
                  </button>
                ))}
              </div>
            </div>
          </div>
        </div>
      </div>

      {learningSlot !== null && (
        <div className="bg-amber-500/10 border border-amber-500/20 p-4 rounded-xl flex items-center gap-4 text-amber-500 animate-in zoom-in-95 duration-200">
          <Target size={24} className="animate-spin-slow" />
          <div className="flex-1">
            <p className="text-sm font-bold uppercase tracking-wider">Learning Mode Active</p>
            <p className="text-sm opacity-80">Hit a physical pad to assign it to <span className="underline font-bold">{roles.find(r => r.slot === learningSlot)?.name}</span>.</p>
          </div>
          <button onClick={() => setLearningSlot(null)} className="text-xs font-bold underline px-2">Cancel</button>
        </div>
      )}

      <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-6 gap-4">
        {roles.map((role) => (
          <Pad 
            key={role.slot}
            id={role.slot.toString()}
            name={role.name}
            midiNote={role.note}
            isActive={activeNotes.has(role.note)}
            isLearning={learningSlot === role.slot}
            onClick={() => setLearningSlot(role.slot)}
          />
        ))}
      </div>

      <div className="mt-12 border border-border rounded-2xl bg-card/30 overflow-hidden">
        <div className="p-4 border-b border-border flex flex-col sm:flex-row sm:items-center justify-between gap-4">
          <div className="flex items-center gap-2">
            <List size={20} className="text-muted-foreground" />
            <h4 className="font-semibold whitespace-nowrap">Role List</h4>
          </div>
          <div className="relative flex-1 max-w-md">
            <MagnifyingGlass className="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground" size={18} />
            <input 
              type="text" 
              placeholder="Search roles..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="w-full bg-muted/50 border border-border rounded-lg py-2 pl-10 pr-4 text-sm outline-none focus:border-primary/50 transition-colors"
            />
          </div>
        </div>
        
        <div className="divide-y divide-border">
          {filteredRoles.map(role => (
            <div key={role.slot} className="flex items-center justify-between p-4 hover:bg-muted/50 transition-colors group">
              <div className="flex items-center gap-3">
                <input 
                  type="text"
                  value={role.name}
                  onChange={(e) => {
                    setRoles(prev => prev.map(r => r.slot === role.slot ? { ...r, name: e.target.value } : r));
                    setHasChanges(true);
                  }}
                  className="bg-transparent border-none outline-none text-sm font-medium w-48 focus:text-primary"
                />
              </div>
              
              <div className="flex items-center gap-4">
                <div className="flex items-center gap-2">
                  <span className="text-[10px] font-bold text-muted-foreground uppercase">Note</span>
                  <input 
                    type="number" 
                    min="0"
                    max="127"
                    value={role.note}
                    onChange={(e) => updateRoleNote(role.slot, parseInt(e.target.value))}
                    className="w-16 bg-muted px-2 py-1 rounded text-xs font-mono outline-none focus:ring-1 focus:ring-primary/50"
                  />
                </div>
                
                <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                  <button onClick={() => setLearningSlot(role.slot)} className="p-2 text-primary hover:bg-primary/10 rounded-lg transition-colors"><Target size={18} /></button>
                  <button onClick={() => deleteRole(role.slot)} className="p-2 text-destructive hover:bg-destructive/10 rounded-lg transition-colors"><Trash size={18} /></button>
                </div>
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  )
}
