import { useState } from 'react'
import { MagnifyingGlass, Books, Folder, Files, Plus } from "@phosphor-icons/react"
import { cn } from '../components/ui'

interface LibraryViewProps {
  availableKits: string[];
  activeKitName?: string;
  soundPresets: string[];
  ws: WebSocket | null;
  selectedSoundId?: any;
}

export default function LibraryView({ 
  availableKits, 
  activeKitName,
  soundPresets, 
  ws, 
  selectedSoundId
}: LibraryViewProps) {
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
    <div className="space-y-6 max-w-4xl mx-auto">
      <header className="flex flex-col md:flex-row md:items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-black uppercase tracking-tighter flex items-center gap-3">
            <Books className="text-primary" size={32} />
            Kit Library
          </h1>
          <p className="text-sm text-muted-foreground mt-1 font-medium">Manage your kits and sound presets</p>
        </div>

        <div className="flex p-1 bg-card/50 backdrop-blur-md rounded-2xl border border-border w-full md:w-64 shrink-0">
          <button
            onClick={() => setActiveTab('kits')}
            className={cn(
              "flex-1 flex items-center justify-center gap-2 py-2.5 rounded-xl text-xs font-bold uppercase tracking-tight transition-all",
              activeTab === 'kits' ? "bg-primary text-primary-foreground shadow-lg shadow-primary/20" : "text-muted-foreground hover:text-foreground hover:bg-muted/40"
            )}
          >
            <Folder size={16} />
            Kits
          </button>
          <button
            onClick={() => setActiveTab('presets')}
            className={cn(
              "flex-1 flex items-center justify-center gap-2 py-2.5 rounded-xl text-xs font-bold uppercase tracking-tight transition-all",
              activeTab === 'presets' ? "bg-primary text-primary-foreground shadow-lg shadow-primary/20" : "text-muted-foreground hover:text-foreground hover:bg-muted/40"
            )}
          >
            <Files size={16} />
            Presets
          </button>
        </div>
      </header>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
        <aside className="md:col-span-1 space-y-6">
          <section className="bg-card/30 border border-border rounded-3xl overflow-hidden backdrop-blur-sm">
            <header className="p-5 border-b border-border bg-card/50">
              <h3 className="font-bold text-xs uppercase tracking-widest text-muted-foreground">Search & Filter</h3>
            </header>
            <div className="p-5 space-y-4">
               <div className="relative">
                <MagnifyingGlass className="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground" size={18} />
                <input 
                  type="text"
                  placeholder={`Search ${activeTab}...`}
                  className="w-full bg-background/50 border border-border rounded-xl pl-10 pr-4 py-3 text-sm outline-none focus:border-primary transition-colors font-medium"
                  value={search}
                  onChange={(e) => setSearch(e.target.value)}
                />
              </div>

              <div className="space-y-2 pt-2 border-t border-border/50">
                <label className="text-[10px] font-black uppercase tracking-widest text-muted-foreground/50 ml-1">New Entry</label>
                <div className="flex items-center gap-2">
                  <input 
                    type="text" 
                    placeholder={activeTab === 'kits' ? "New kit name..." : "New preset name..."}
                    className="flex-1 min-w-0 bg-background/50 border border-border rounded-xl px-4 py-2.5 text-sm outline-none focus:border-primary/50 transition-colors font-medium"
                    value={newName}
                    onChange={e => setNewName(e.target.value)}
                    onKeyDown={e => e.key === 'Enter' && handleSave()}
                  />
                  <button
                    onClick={handleSave}
                    disabled={!newName || (activeTab === 'presets' && (selectedSoundId === null || selectedSoundId === undefined))}
                    className="p-3 bg-primary text-primary-foreground rounded-xl disabled:opacity-50 hover:scale-105 active:scale-95 transition-all shadow-lg shadow-primary/10 shrink-0"
                  >
                    <Plus size={20} weight="bold" />
                  </button>
                </div>
                {activeTab === 'presets' && (selectedSoundId === null || selectedSoundId === undefined) && (
                  <p className="text-[9px] text-amber-500 font-bold uppercase text-center bg-amber-500/5 py-2 rounded-lg border border-amber-500/20">
                    Select a sound to save presets
                  </p>
                )}
              </div>
            </div>
          </section>
        </aside>

        <main className="md:col-span-2">
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 pb-8">
            {activeTab === 'kits' ? (
              filteredKits.length > 0 ? (
                filteredKits.map(kit => (
                  <button
                    key={kit}
                    onClick={() => ws?.send(`LOAD_KIT:${kit}`)}
                    className={cn(
                      "flex items-center gap-4 p-4 rounded-2xl border transition-all text-left group relative overflow-hidden",
                      kit === activeKitName 
                        ? "bg-emerald-500/10 border-emerald-500/50 ring-1 ring-emerald-500/50 shadow-[0_0_20px_rgba(16,185,129,0.1)]" 
                        : "bg-card/20 border-border hover:border-primary/30 hover:bg-card/40 hover:translate-y-[-2px]"
                    )}
                  >
                    <div className={cn(
                      "w-12 h-12 rounded-xl flex items-center justify-center transition-colors shrink-0",
                      kit === activeKitName ? "bg-emerald-500 text-white shadow-[0_0_15px_rgba(16,185,129,0.5)]" : "bg-muted text-muted-foreground group-hover:bg-primary/20 group-hover:text-primary"
                    )}>
                      <Folder size={24} weight="duotone" />
                    </div>
                    <div className="min-w-0">
                      <div className={cn("font-bold truncate", kit === activeKitName ? "text-emerald-400" : "text-foreground")}>{kit}</div>
                      <div className="text-[10px] uppercase tracking-widest text-muted-foreground/60 font-bold">
                        {kit === activeKitName ? 'Active Kit' : 'Stored Kit'}
                      </div>
                    </div>
                    {kit === activeKitName && (
                      <div className="absolute right-4 top-1/2 -translate-y-1/2">
                        <div className="w-2 h-2 rounded-full bg-emerald-500 animate-ping" />
                      </div>
                    )}
                  </button>
                ))
              ) : (
                <div className="col-span-full py-20 text-center bg-card/10 border border-dashed border-border rounded-3xl">
                   <Folder size={48} className="mx-auto text-muted-foreground/20 mb-4" />
                   <p className="text-muted-foreground italic font-medium">No kits found matching your search</p>
                </div>
              )
            ) : (
              filteredPresets.length > 0 ? (
                filteredPresets.map(preset => (
                  <button
                    key={preset}
                    disabled={selectedSoundId === null || selectedSoundId === undefined}
                    onClick={() => {
                      if (selectedSoundId !== null && ws) {
                        ws.send(`LOAD_SOUND_PRESET:${preset}:${selectedSoundId}`);
                      }
                    }}
                    className={cn(
                      "flex items-center gap-4 p-4 rounded-2xl border transition-all text-left group relative",
                      (selectedSoundId === null || selectedSoundId === undefined)
                        ? "opacity-40 grayscale cursor-not-allowed border-border/50"
                        : "bg-card/20 border-border hover:border-primary/30 hover:bg-card/40 hover:translate-y-[-2px]"
                    )}
                  >
                    <div className={cn(
                      "w-12 h-12 rounded-xl flex items-center justify-center transition-colors shrink-0",
                      "bg-muted text-muted-foreground group-hover:bg-primary/20 group-hover:text-primary"
                    )}>
                      <Files size={24} weight="duotone" />
                    </div>
                    <div className="min-w-0">
                      <div className="font-bold truncate text-foreground">{preset}</div>
                      <div className="text-[10px] uppercase tracking-widest text-muted-foreground/60 font-bold">Sound Preset</div>
                    </div>
                  </button>
                ))
              ) : (
                <div className="col-span-full py-20 text-center bg-card/10 border border-dashed border-border rounded-3xl">
                   <Files size={48} className="mx-auto text-muted-foreground/20 mb-4" />
                   <p className="text-muted-foreground italic font-medium">No presets found matching your search</p>
                </div>
              )
            )}
          </div>
        </main>
      </div>
    </div>
  )
}
