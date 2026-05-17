//! Preview Kit playback: schedule note-ons from a curated MIDI file into the
//! existing `midi_producer` ring buffer so the audio thread plays them through
//! whatever kit is currently loaded.
//!
//! Scheduling lives on a tokio task. The task absorbs tempo meta-events and
//! routes any GM-percussion-range (notes 35..=81) note-on -- regardless of
//! channel -- since Groove MIDI files mostly use channel 10/idx 9 but we want
//! to be tolerant of slightly off-spec sources.
//!
//! The task pushes `[0x90, note, velocity]` into the same ring buffer that
//! live MIDI input uses, so all downstream logic (mapping, BPM detection,
//! voice triggering) is identical to a real MIDI controller.

use crate::state::{MidiEvent, SharedState};
use anyhow::{Result, anyhow};
use midly::{MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};
use rtrb::Producer;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::time::{Duration, Instant, sleep_until};

const PRESETS_MIDI_DIR: &str = "presets/midi";
const DEFAULT_TEMPO_US_PER_QUARTER: u32 = 500_000; // 120 BPM
const GM_PERCUSSION_LOW: u8 = 35;
const GM_PERCUSSION_HIGH: u8 = 81;

/// Return the sorted list of `.mid` track names (without extension) available
/// for Preview Kit playback. Used by `LIST_MIDI_TRACKS`.
pub fn list_tracks() -> Vec<String> {
    let dir = Path::new(PRESETS_MIDI_DIR);
    let mut names: Vec<String> = match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let name = e.file_name().into_string().ok()?;
                if name.ends_with(".mid") {
                    Some(name.trim_end_matches(".mid").to_string())
                } else {
                    None
                }
            })
            .collect(),
        Err(_) => Vec::new(),
    };
    names.sort();
    names
}

/// Validate `name` resolves to a `.mid` file inside `presets/midi/` -- no
/// path traversal, no other extensions. Returns the full path if OK.
fn resolve_track(name: &str) -> Result<PathBuf> {
    if name.is_empty()
        || name.contains('/')
        || name.contains('\\')
        || name.contains("..")
        || name.contains('\0')
    {
        return Err(anyhow!("invalid track name: {:?}", name));
    }
    let path = Path::new(PRESETS_MIDI_DIR).join(format!("{}.mid", name));
    if !path.is_file() {
        return Err(anyhow!("track not found: {}", path.display()));
    }
    Ok(path)
}

/// One scheduled note-on, with its absolute offset from the start of the track.
#[derive(Debug, Clone)]
struct ScheduledNote {
    offset_us: u64,
    note: u8,
    velocity: u8,
}

/// Result of parsing a MIDI file: scheduled note-ons plus the track's
/// authoritative tempo (BPM) so clock-aware kits can sync their decays /
/// LFOs / patterns to the demo track instead of the user's stale live BPM.
struct ParsedTrack {
    notes: Vec<ScheduledNote>,
    bpm: f32,
}

/// Convert a tempo expressed as microseconds-per-quarter to BPM.
fn us_per_quarter_to_bpm(us: u32) -> f32 {
    if us == 0 { 120.0 } else { 60_000_000.0 / (us as f32) }
}

/// Parse a MIDI file into a flat, time-sorted list of note-on events with
/// absolute microsecond offsets. Honours tempo meta-events; for `Timing::Metrical`
/// files we walk all tracks merged by absolute tick and apply tempo changes as
/// we encounter them. For `Timing::Timecode` files we use the SMPTE rate directly.
/// Also returns the track's first tempo (or 120 BPM if none present) so the
/// audio path can lock clock-aware kits to the playing track.
fn parse_events(path: &Path) -> Result<ParsedTrack> {
    let bytes = std::fs::read(path)?;
    let smf = Smf::parse(&bytes).map_err(|e| anyhow!("parse {}: {}", path.display(), e))?;

    // Flatten all tracks into (abs_tick, event) and sort by tick. Tempo events
    // from any track apply globally from their absolute tick onward.
    let mut flat: Vec<(u64, TrackEventKind)> = Vec::new();
    for track in &smf.tracks {
        let mut abs_tick: u64 = 0;
        for event in track {
            abs_tick += u32::from(event.delta) as u64;
            flat.push((abs_tick, event.kind));
        }
    }
    flat.sort_by_key(|(t, _)| *t);

    let mut out: Vec<ScheduledNote> = Vec::new();
    // Record the first tempo encountered so we can broadcast it to the audio
    // thread when playback starts. Most Groove MIDI files have a single tempo
    // at tick 0; for files with multiple tempos we use the first one as the
    // representative BPM (the curated tracks don't vary tempo mid-file).
    let mut first_tempo_us: Option<u32> = None;

    match smf.header.timing {
        Timing::Metrical(tpq) => {
            let ticks_per_quarter = u16::from(tpq) as u64;
            if ticks_per_quarter == 0 {
                return Err(anyhow!("invalid ticks_per_quarter=0"));
            }
            // Walk events, maintaining (last_tick, accumulated_us, current_tempo).
            let mut cur_tempo: u32 = DEFAULT_TEMPO_US_PER_QUARTER;
            let mut last_tick: u64 = 0;
            let mut accum_us: u64 = 0;
            for (abs_tick, kind) in flat {
                let delta_ticks = abs_tick - last_tick;
                // microseconds = delta_ticks * tempo_us_per_quarter / ticks_per_quarter
                accum_us += delta_ticks * (cur_tempo as u64) / ticks_per_quarter;
                last_tick = abs_tick;

                match kind {
                    TrackEventKind::Meta(MetaMessage::Tempo(t)) => {
                        cur_tempo = u32::from(t);
                        if first_tempo_us.is_none() {
                            first_tempo_us = Some(u32::from(t));
                        }
                    }
                    TrackEventKind::Midi { message, .. } => {
                        if let MidiMessage::NoteOn { key, vel } = message {
                            let note = u8::from(key);
                            let velocity = u8::from(vel);
                            if velocity > 0
                                && (GM_PERCUSSION_LOW..=GM_PERCUSSION_HIGH).contains(&note)
                            {
                                out.push(ScheduledNote {
                                    offset_us: accum_us,
                                    note,
                                    velocity,
                                });
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        Timing::Timecode(fps, subframe) => {
            // Microseconds per tick = 1e6 / (fps * subframe).
            let fps_f = fps.as_f32() as f64;
            let subframe_f = subframe as f64;
            if fps_f <= 0.0 || subframe_f <= 0.0 {
                return Err(anyhow!("invalid SMPTE timing"));
            }
            let us_per_tick = 1_000_000.0_f64 / (fps_f * subframe_f);
            // SMPTE-timed files have no tempo meta; assume the default.
            first_tempo_us = Some(DEFAULT_TEMPO_US_PER_QUARTER);
            for (abs_tick, kind) in flat {
                if let TrackEventKind::Midi { message, .. } = kind {
                    if let MidiMessage::NoteOn { key, vel } = message {
                        let note = u8::from(key);
                        let velocity = u8::from(vel);
                        if velocity > 0
                            && (GM_PERCUSSION_LOW..=GM_PERCUSSION_HIGH).contains(&note)
                        {
                            let offset_us = (abs_tick as f64 * us_per_tick) as u64;
                            out.push(ScheduledNote {
                                offset_us,
                                note,
                                velocity,
                            });
                        }
                    }
                }
            }
        }
    }
    let bpm = us_per_quarter_to_bpm(first_tempo_us.unwrap_or(DEFAULT_TEMPO_US_PER_QUARTER));
    Ok(ParsedTrack { notes: out, bpm })
}

/// Public entry point. Loads `<name>.mid` from `presets/midi/`, parses note-on
/// events, and returns a tokio task that pushes each event into `midi_producer`
/// at its scheduled time. The caller stores the JoinHandle and aborts it on
/// stop / replacement. `on_finish` is invoked once the last note has been
/// pushed (used to broadcast `MIDI_TRACK_STOPPED`).
///
/// The track's recorded tempo is also written to `shared_state.store_bpm()`
/// so clock-aware kits (decay_division, pattern, lfo*_division) sync to the
/// demo track rather than the user's stale live BPM.
pub fn spawn_playback(
    name: &str,
    midi_producer: Arc<std::sync::Mutex<Producer<MidiEvent>>>,
    shared_state: Arc<SharedState>,
    on_finish: impl FnOnce() + Send + 'static,
) -> Result<tokio::task::JoinHandle<()>> {
    let path = resolve_track(name)?;
    let parsed = parse_events(&path)?;
    // Tell the audio path the track's BPM so clock-aware kits adopt it
    // immediately, before the first note fires.
    shared_state.store_bpm(parsed.bpm);
    if parsed.notes.is_empty() {
        // Empty schedule: still broadcast MIDI_TRACK_STOPPED so the UI resets.
        return Ok(tokio::spawn(async move {
            on_finish();
        }));
    }

    let events = parsed.notes;
    let handle = tokio::spawn(async move {
        let start = Instant::now();
        for ev in events {
            let target = start + Duration::from_micros(ev.offset_us);
            sleep_until(target).await;
            if let Ok(mut p) = midi_producer.lock() {
                // Ring buffer full -> drop the event silently. 1024-deep queue
                // is generous enough that this shouldn't happen for normal
                // drum patterns. The audio thread is the only consumer.
                let _ = p.push([0x90, ev.note, ev.velocity]);
            }
        }
        on_finish();
    });
    Ok(handle)
}
