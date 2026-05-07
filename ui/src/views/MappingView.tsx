import { useState, useEffect, useCallback, useMemo } from 'react'
import { Plus, List, Target, CheckCircle, WarningCircle, MagnifyingGlass, Trash, FloppyDisk } from "@phosphor-icons/react"
import { clsx, type ClassValue } from 'clsx'
import { twMerge } from 'tailwind-merge'

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

interface Role {
  id: string;
  name: string;
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
  const [searchQuery, setSearchQuery] = useState('');
  const [roles, setRoles] = useState<Role[]>([]);
  const [isSaving, setIsSaving] = useState(false);
  const [hasChanges, setHasChanges] = useState(false);

  const filteredRoles = useMemo(() => {
    return roles.filter(r => r.name.toLowerCase().includes(searchQuery.toLowerCase()));
  }, [roles, searchQuery]);

  const updateRoleNote = useCallback((id: string, newNote: number) => {
    setRoles(prev => prev.map(r => r.id === id ? { ...r, note: newNote } : r));
    setLearningRoleId(null);
    setHasChanges(true);
  }, []);

  const deleteRole = (id: string) => {
    setRoles(prev => prev.filter(r => r.id !== id));
    setHasChanges(true);
  };

  const addRole = () => {
    const id = `role_${Date.now()}`;
    setRoles(prev => [...prev, { id, name: 'New Role', note: 0 }]);
    setLearningRoleId(id);
    setHasChanges(true);
  };

  const handleSave = () => {
    if (!ws) return;
    setIsSaving(true);
    // Send mapping as JSON
    ws.send(`SAVE_MAPPING:${JSON.stringify(roles)}`);
    // Simulated delay for UI feedback
    setTimeout(() => {
      setIsSaving(false);
      setHasChanges(false);
    }, 500);
  };

  useEffect(() => {
    if (!ws) return;

    // Request current mapping on load
    ws.send('GET_MAPPING');

    const handleMessage = (event: MessageEvent) => {
      const data = event.data as string;
      if (data.startsWith('MIDI: ')) {
        const parts = data.replace('MIDI: ', '').split(',');
        const note = parseInt(parts[0]);
        const velocity = parseInt(parts[1]);

        if (velocity > 0) {
          setActiveNotes(prev => new Set(prev).add(note));
          setTimeout(() => {
            setActiveNotes(prev => {
              const next = new Set(prev);
              next.delete(note);
              return next;
            });
          }, 100);

          if (learningRoleId) {
            updateRoleNote(learningRoleId, note);
          }
        }
      } else if (data.startsWith('MAPPING: ')) {
        try {
          const mapping = JSON.parse(data.replace('MAPPING: ', '')) as Role[];
          setRoles(mapping);
        } catch (e) {
          console.error('Failed to parse mapping:', e);
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
          {hasChanges && (
            <button 
              onClick={handleSave}
              disabled={isSaving}
              className={cn(
                "flex items-center gap-2 px-4 py-2 rounded-lg transition-all text-sm font-medium",
                isSaving 
                  ? "bg-muted text-muted-foreground animate-pulse" 
                  : "bg-primary text-primary-foreground hover:scale-105 active:scale-95"
              )}
            >
              <FloppyDisk size={18} />
              {isSaving ? 'Saving...' : 'Save Changes'}
            </button>
          )}
          {learningRoleId && (
            <button 
              onClick={() => setLearningRoleId(null)}
              className="flex items-center gap-2 px-4 py-2 bg-destructive/10 text-destructive rounded-lg hover:bg-destructive/20 transition-colors text-sm font-medium"
            >
              Cancel Learning
            </button>
          )}
          <button 
            onClick={addRole}
            className="flex items-center gap-2 px-4 py-2 bg-secondary text-secondary-foreground rounded-lg hover:bg-muted transition-colors text-sm font-medium"
          >
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
            <div key={role.id} className="flex items-center justify-between p-4 hover:bg-muted/50 transition-colors group">
              <div className="flex items-center gap-3">
                <input 
                  type="text"
                  value={role.name}
                  onChange={(e) => {
                    setRoles(prev => prev.map(r => r.id === role.id ? { ...r, name: e.target.value } : r));
                    setHasChanges(true);
                  }}
                  className="bg-transparent border-none outline-none text-sm font-medium w-32 focus:text-primary"
                />
                {role.note === undefined && <WarningCircle className="text-destructive" size={16} />}
              </div>
              
              <div className="flex items-center gap-4">
                <div className="flex items-center gap-2">
                  <span className="text-[10px] font-bold text-muted-foreground uppercase">Note</span>
                  <input 
                    type="number" 
                    min="0"
                    max="127"
                    value={role.note}
                    onChange={(e) => updateRoleNote(role.id, parseInt(e.target.value))}
                    className="w-16 bg-muted px-2 py-1 rounded text-xs font-mono outline-none focus:ring-1 focus:ring-primary/50"
                  />
                </div>
                
                <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                  <button 
                    onClick={() => setLearningRoleId(role.id)}
                    className="p-2 text-primary hover:bg-primary/10 rounded-lg transition-colors"
                    title="MIDI Learn"
                  >
                    <Target size={18} />
                  </button>
                  <button 
                    onClick={() => deleteRole(role.id)}
                    className="p-2 text-destructive hover:bg-destructive/10 rounded-lg transition-colors"
                    title="Delete Role"
                  >
                    <Trash size={18} />
                  </button>
                </div>
              </div>
            </div>
          ))}
          {filteredRoles.length === 0 && (
            <div className="p-12 text-center text-muted-foreground text-sm italic">
              No roles found matching "{searchQuery}"
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
