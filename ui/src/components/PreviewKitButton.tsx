import { useState, useRef, useEffect } from 'react';
import { Play, Stop, CaretDown, MusicNotes } from '@phosphor-icons/react';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

interface PreviewKitButtonProps {
  ws: WebSocket | null;
  /** Track list received via `MIDI_TRACKS:` broadcast (no `.mid` extension). */
  tracks: string[];
  /** Currently-playing track name, or `null` if nothing is playing. */
  playingTrack: string | null;
}

/**
 * Compact Preview Kit control. Idle state shows a dropdown of available
 * MIDI tracks; clicking one sends `PLAY_MIDI_TRACK:<name>` and the button
 * switches to a Stop affordance that shows the playing track name. The
 * parent (App.tsx) owns `playingTrack` because the WS listener lives there;
 * this component just renders the current state and dispatches commands.
 */
export function PreviewKitButton({ ws, tracks, playingTrack }: PreviewKitButtonProps) {
  const [open, setOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement | null>(null);

  // Close the dropdown on outside click. Keeps the menu unobtrusive when
  // the user clicks anywhere else in the header / transport bar.
  useEffect(() => {
    if (!open) return;
    const onDocClick = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener('mousedown', onDocClick);
    return () => document.removeEventListener('mousedown', onDocClick);
  }, [open]);

  const isPlaying = playingTrack !== null;
  const disabled = !ws || tracks.length === 0;

  const handlePlay = (name: string) => {
    setOpen(false);
    ws?.send(`PLAY_MIDI_TRACK:${name}`);
  };

  const handleStop = () => {
    ws?.send('STOP_MIDI_PLAYBACK');
  };

  return (
    <div ref={menuRef} className="relative">
      {isPlaying ? (
        <button
          onClick={handleStop}
          aria-label={`Stop preview (playing ${playingTrack})`}
          className={cn(
            'px-3 py-2 rounded-lg text-[10px] font-black uppercase tracking-wider transition-all border flex items-center gap-2',
            'bg-emerald-500/20 border-emerald-500 text-emerald-400 shadow-[0_0_15px_rgba(16,185,129,0.25)]',
            'hover:bg-emerald-500/30 focus:outline-none focus-visible:ring-2 focus-visible:ring-emerald-400',
          )}
        >
          <Stop size={14} weight="fill" />
          <span className="hidden sm:inline truncate max-w-[10rem]">{playingTrack}</span>
        </button>
      ) : (
        <button
          onClick={() => setOpen((v) => !v)}
          aria-haspopup="menu"
          aria-expanded={open}
          aria-label="Preview Kit with MIDI track"
          disabled={disabled}
          className={cn(
            'px-3 py-2 rounded-lg text-[10px] font-black uppercase tracking-wider transition-all border flex items-center gap-2',
            disabled
              ? 'bg-muted/30 border-border text-muted-foreground/60 cursor-not-allowed'
              : 'bg-background/50 border-border text-muted-foreground hover:text-foreground hover:border-primary/30',
            'focus:outline-none focus-visible:ring-2 focus-visible:ring-primary',
          )}
        >
          <Play size={14} weight="fill" />
          <span className="hidden sm:inline">Preview Kit</span>
          <CaretDown size={10} className={cn('transition-transform', open && 'rotate-180')} />
        </button>
      )}

      {open && !isPlaying && (
        <div
          role="menu"
          className="absolute top-full left-0 mt-2 w-64 max-h-80 overflow-y-auto bg-card border border-border rounded-xl shadow-2xl backdrop-blur-xl z-50"
        >
          <div className="p-2 border-b border-border flex items-center gap-2 text-[10px] font-bold uppercase tracking-wider text-muted-foreground">
            <MusicNotes size={12} /> Choose a track
          </div>
          {tracks.length === 0 ? (
            <div className="p-4 text-xs text-center text-muted-foreground italic">
              No MIDI tracks found
            </div>
          ) : (
            <ul className="py-1">
              {tracks.map((t) => (
                <li key={t}>
                  <button
                    role="menuitem"
                    onClick={() => handlePlay(t)}
                    className="w-full text-left px-3 py-2 text-xs text-foreground hover:bg-primary/10 hover:text-primary transition-colors flex items-center gap-2"
                  >
                    <Play size={10} weight="fill" className="opacity-60" />
                    <span className="truncate">{formatTrackName(t)}</span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
    </div>
  );
}

/**
 * Turn `rock_100_beat` into `Rock 100 BPM Beat` for display. We keep the
 * underscore convention on disk (matches the file names so backend lookups
 * are trivial) but humanise it in the dropdown.
 */
function formatTrackName(slug: string): string {
  const parts = slug.split('_');
  return parts
    .map((p, i) => {
      if (/^\d+$/.test(p) && i > 0) return `${p} BPM`;
      return p.charAt(0).toUpperCase() + p.slice(1);
    })
    .join(' ');
}
