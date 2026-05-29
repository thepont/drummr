import React, { useState } from 'react'
import { MagnifyingGlass, Books, X, Play, Folder, Files, Plus } from "@phosphor-icons/react"
import { cn } from './ui'

interface LibrarySidebarProps {
  availableKits: string[];
  activeKitName?: string;
  soundPresets: string[];
  ws: WebSocket | null;
  selectedSoundId?: any;
  isOpen: boolean;
  onClose: () => void;
}

export default function LibrarySidebar({ 
  availableKits, 
  activeKitName,
  soundPresets, 
  ws, 
  selectedSoundId,
  isOpen,
  onClose
}: LibrarySidebarProps) {
  const [search, setSearch] = useState("");
  const [activeTab, setActiveTab] = useState<'kits' | 'presets'>('kits');
  const [newName, setNewName] = useState("");

  const filteredKits = availableKits.filter(k => k.toLowerCase().includes(search.toLowerCase()));
  const filteredPresets = soundPresets.filter(p => p.toLowerCase().includes(search.toLowerCase()));

  const handleSave = () => {
    if (!newName || !ws) return;
    if (activeTab === 'kits') {
      ws.send(`SAVE_KIT_AS:${newName}`);
    } else if (selectedSoundId !== null && selectedSoundId !== undefined) {
      ws.send(`SAVE_SOUND_PRESET:${newName}:${selectedSoundId}`);
    }
    setNewName("");
  };

  return (
    <>
      {/* Backdrop for mobile */}
      {isOpen && (
        <div 
          className="fixed inset-0 bg-background/80 backdrop-blur-sm z-30 lg:hidden" 
          onClick={onClose}
        />
      )}
      <aside className={cn(
        "fixed inset-y-0 right-0 z-40 w-80 bg-card border-l border-border flex flex-col shadow-2xl transition-transform duration-300 transform lg:static lg:translate-x-0 lg:shadow-none lg:border-none h-full",
        isOpen ? "translate-x-0" : "translate-x-full",
        !isOpen && "lg:hidden"
      )}>
        <header className="p-4 border-b border-border flex items-center justify-between bg-card/50 backdrop-blur-md sticky top-0 z-10">
          <div className="flex items-center gap-2">
            <Books size={20} className="text-primary" />
            <h3 className="font-bold uppercase tracking-widest text-xs">Library</h3>
          </div>
          <button
            onClick={onClose}
            aria-label="Close library"
            className="lg:hidden p-2 hover:bg-muted rounded-full focus:outline-none focus-visible:ring-2 focus-visible:ring-primary"
          >
            <X size={20} />
          </button>
        </header>

        <div className="p-4 space-y-4">
          <div className="relative">
            <MagnifyingGlass className="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground" size={16} />
            <input 
              type="text"
              placeholder="Search..."
              className="w-full bg-muted/50 border border-border rounded-xl pl-10 pr-4 py-2 text-sm outline-none focus:border-primary transition-colors"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
            />
          </div>

          <div className="flex p-1 bg-muted/30 rounded-xl border border-border/50">
            <button
              onClick={() => setActiveTab('kits')}
              aria-pressed={activeTab === 'kits'}
              className={cn(
                "flex-1 flex items-center justify-center gap-2 py-2 rounded-lg text-xs font-bold uppercase tracking-tight transition-all focus:outline-none focus-visible:ring-2 focus-visible:ring-primary",
                activeTab === 'kits' ? "bg-primary text-primary-foreground shadow-lg" : "text-muted-foreground hover:text-foreground hover:bg-muted/40"
              )}
            >
              <Folder size={14} />
              Kits
            </button>
            <button
              onClick={() => setActiveTab('presets')}
              aria-pressed={activeTab === 'presets'}
              className={cn(
                "flex-1 flex items-center justify-center gap-2 py-2 rounded-lg text-xs font-bold uppercase tracking-tight transition-all focus:outline-none focus-visible:ring-2 focus-visible:ring-primary",
                activeTab === 'presets' ? "bg-primary text-primary-foreground shadow-lg" : "text-muted-foreground hover:text-foreground hover:bg-muted/40"
              )}
            >
              <Files size={14} />
              Presets
            </button>
          </div>
        </div>

        <div className="flex-1 overflow-y-auto px-2 pb-4 space-y-1 custom-scrollbar">
          {activeTab === 'kits' ? (
            filteredKits.length > 0 ? (
              filteredKits.map(kit => (
                <LibraryItem 
                  key={kit}
                  label={kit}
                  icon={<Folder size={18} />}
                  isActive={kit === activeKitName}
                  onClick={() => ws?.send(`LOAD_KIT:${kit}`)}
                />
              ))

            ) : (
              <EmptyState message="No kits found" />
            )
          ) : (
            filteredPresets.length > 0 ? (
              filteredPresets.map(preset => (
                <LibraryItem 
                  key={preset}
                  label={preset}
                  icon={<Files size={18} />}
                  disabled={selectedSoundId === null || selectedSoundId === undefined}
                  onClick={() => {
                    if (selectedSoundId !== null && ws) {
                      ws.send(`LOAD_SOUND_PRESET:${preset}:${selectedSoundId}`);
                    }
                  }}
                />
              ))
            ) : (
              <EmptyState message="No presets found" />
            )
          )}
        </div>

        <div className="p-4 border-t border-border bg-muted/10 space-y-3">
           <div className="flex items-center gap-2">
              <input 
                type="text" 
                placeholder={activeTab === 'kits' ? "New kit name..." : "New preset name..."}
                className="flex-1 min-w-0 bg-background border border-border rounded-lg px-3 py-2 text-xs outline-none focus:border-primary/50 transition-colors"
                value={newName}
                onChange={e => setNewName(e.target.value)}
                onKeyDown={e => e.key === 'Enter' && handleSave()}
              />
              <button
                onClick={handleSave}
                disabled={!newName || (activeTab === 'presets' && (selectedSoundId === null || selectedSoundId === undefined))}
                aria-label={activeTab === 'kits' ? 'Save current kit' : 'Save current sound as preset'}
                className="p-2 bg-primary text-primary-foreground rounded-lg disabled:opacity-50 disabled:hover:scale-100 hover:scale-105 active:scale-95 transition-all focus:outline-none focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-offset-1 focus-visible:ring-offset-background shrink-0"
              >
                <Plus size={16} weight="bold" />
              </button>
           </div>
           {activeTab === 'presets' && (selectedSoundId === null || selectedSoundId === undefined) && (
              <p className="text-[9px] text-amber-500 font-bold uppercase text-center">
                Select a sound to save presets
              </p>
           )}
        </div>
      </aside>
    </>
  )
}


function LibraryItem({ label, icon, onClick, disabled, isActive }: { label: string, icon: React.ReactNode, onClick: () => void, disabled?: boolean, isActive?: boolean }) {
  return (
    <button 
      onClick={onClick}
      disabled={disabled}
      className={cn(
        "w-full flex items-center gap-3 px-4 py-3 rounded-xl transition-all border border-transparent group",
        disabled 
          ? "opacity-50 cursor-not-allowed" 
          : "hover:bg-primary/5 hover:border-primary/20 hover:translate-x-1",
        isActive && "bg-emerald-500/10 border-emerald-500/50 text-emerald-400 shadow-[0_0_15px_rgba(16,185,129,0.1)]"
      )}
    >
      <div className={cn(
        "w-8 h-8 rounded-lg flex items-center justify-center transition-colors",
        disabled ? "bg-muted text-muted-foreground" : (isActive ? "bg-emerald-500 text-white shadow-[0_0_10px_rgba(16,185,129,0.5)]" : "bg-muted group-hover:bg-primary/20 group-hover:text-primary")
      )}>
        {icon}
      </div>
      <span className={cn("text-sm font-bold truncate", isActive ? "text-emerald-400" : "text-foreground/80 group-hover:text-foreground")}>{label}</span>
      <div className={cn("ml-auto opacity-0 group-hover:opacity-100 transition-opacity", isActive && "opacity-100")}>
        <Play size={12} weight="fill" className={cn("text-emerald-500")} />
      </div>
    </button>
  )
}


function EmptyState({ message }: { message: string }) {
  return (
    <div className="py-12 text-center text-xs text-muted-foreground italic px-4">
      {message}
    </div>
  )
}
