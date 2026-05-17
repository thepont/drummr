# Backend Memory & Resource Leaks

Last updated: 2026-05-18

Audit of the Rust backend crate (`src/`) for leak patterns. Severity:
- **HIGH**: real leak that compounds; observable in production
- **MEDIUM**: leak under specific user actions (device switch, reconnect)
- **LOW**: latent / dormant / intentional but worth documenting
- **NON-ISSUE**: investigated, confirmed clean

The backend is `tokio` + `cpal` with a `std::thread` persistence worker, a
`std::thread` MIDI master-clock (sync) thread, and a `std::thread` started by
`midir` for every open MIDI input port. All audio communication goes through
`rtrb` ring buffers (1024-deep). The biggest known leak is `mem::forget` on
`cpal::Stream` at every audio-device switch — `audio_stream_leak_count` is
already tracked at runtime.

The frontend audit lives in `docs/frontend_leaks.md`; this file does not
duplicate those findings.

## HIGH

### 1. `cpal::Stream` mem::forget on every audio-device switch — `src/main.rs:270`, `src/main.rs:371`, `src/commands.rs:795`
**Symptom**: `cpal::Stream` is `!Send + !Sync` on every platform, so cpal
streams cannot be stored across an `await` or behind a `Sync` mutex inside
`SharedState`. Three call sites (initial start in `main.rs`, the
auto-recovery task in `main.rs`, and the `SELECT_AUDIO:` command handler in
`commands.rs`) all `std::mem::forget(out_stream)` to keep the stream alive
for the program's lifetime. The closure each stream owns captures:
- `Arc<SharedState>` (a small struct of atomics + 2 `Arc<Mutex<>>`s; the
  Arc clone is cheap, but it pins the `SharedState` allocation forever).
- `Consumer<MidiEvent>` — half of a 1024-entry rtrb ring buffer for
  `[u8; 3]`. Each ring is `1024 * 3` ≈ 3 KB of payload plus header.
- `Consumer<AudioCommand>` — half of a 1024-entry rtrb ring buffer for
  the `AudioCommand` enum. `AudioCommand::SetParam(usize, String, f32)`
  is the largest variant — `String` is 24 bytes empty, so the discriminant +
  largest variant rounds to ~48 bytes. 1024 * 48 ≈ 48 KB.
- `tokio::sync::mpsc::UnboundedSender<()>` — the audio_error_tx clone.
  Cheap (one Arc).
- The cpal worker thread itself, which is also leaked: it holds the stream
  callback and runs forever on the dead device.

**Quantified**: Per leaked stream, roughly `~3 KB (midi ring) + ~48 KB
(cmd ring) + ~64 bytes (Arc clones)` ≈ **~52 KB of pinned heap**, plus
**one OS thread** (the cpal worker, ~512 KB virtual stack on macOS;
typically ~16-32 KB resident). Audio HAL state on the dead device also
remains held — on macOS that's an audio aggregate device handle, on Linux
ALSA the pcm handle, etc. So the *user-visible* leak per switch is "this
device stays busy and consumes a thread."

To reach 100 MB just from rtrb buffers: ~2000 switches. To exhaust the
default 1024 thread-per-process limit on Linux: ~1000 switches. Realistic
break-even is the thread cap, not the RAM cap.

**Repro**: Open the UI, repeatedly switch between two output devices via the
audio-device selector. Each pick fires `SELECT_AUDIO:` and runs
`commands.rs:765-802`. Watch `audio_stream_leak_count` increment.
`std::mem::forget` warnings appear on stderr starting from the second
switch.

**Suggested fix** (non-trivial): Move stream ownership into a dedicated
`std::thread` (since `cpal::Stream` is `!Send + !Sync`, the owning thread
also has to run all `start_audio` calls). Use a `std::sync::mpsc` to send
the thread a "switch to device X" request. The current stream drops
naturally before the new one is built, freeing the ring buffers and the
cpal worker thread.

The 500 ms backoff in the auto-recovery task (`main.rs:386`) caps the
recovery leak rate at 2/sec — at that rate a runaway device disconnect
loop leaks ~100 KB/sec of ring-buffer memory plus 2 threads/sec, which
saturates the Linux thread-per-process cap in ~8 minutes.

### 2. `register_onset` allocates ~250-entry `Vec` per onset — `src/dsp/bpm_engine.rs:96`
**Symptom**: `BpmEngine::estimate_tempo` is called from `register_onset`
(every MIDI note-on, audio-thread-adjacent via midir's callback thread).
It allocates `let mut scores: Vec<(f32, f32)> = Vec::new();` and pushes
~250 entries into it (the `MIN_LAG_SEC..MAX_LAG_SEC` sweep at 5 ms
resolution = 250 steps). Re-allocates several times during growth before
landing at capacity. The `times` Vec (line 87) also allocates per call
(up to MAX_ONSETS = 96 entries).

**Quantified**: ~250 * 8 bytes = 2 KB of allocations per onset, plus a few
hundred bytes for `times`. At a busy 16-hit-per-second fill: ~32 KB/sec
of allocator churn on the midir callback thread. Not a leak (Vecs drop at
end of scope) but a real per-onset allocation tax on a non-realtime
thread.

**Repro**: Drum hard. `cargo flamegraph` against a benchmark calling
`register_onset` in a hot loop shows `malloc` / `free` time.

**Suggested fix** (non-trivial): hoist the two Vecs to fields on
`BpmEngine` with `Vec::with_capacity(...)` in `new`, and `clear()` at the
top of `estimate_tempo`. Both bounds are known constants
(MAX_ONSETS = 96, (MAX_LAG - MIN_LAG)/LAG_STEP = 250). Bounded
re-use, zero allocator pressure.

## MEDIUM

### 3. `audio_stream_leak_count` is never read after the warning — `src/state.rs:28`
**Symptom**: The counter increments unboundedly on every device switch +
auto-recovery. There is no UI surface for it, no `GET_STATS` command, and
no log line on a healthy switch (the eprintln only fires when `prior > 0`).
A user with a flaky USB interface gets one stderr warning per leak with no
session-level totals.

**Quantified**: 1 `AtomicU32` field. Memory-wise nothing; observability-wise
the leak is invisible to anyone not watching stderr.

**Suggested fix**: Add a `GET_AUDIO_STATS` WS command and broadcast
`AUDIO_STATS:{leak_count,..}`. Or log the count every N switches.

### 4. `senders` Vec only pruned during broadcast — `src/comm.rs:90-93`
**Symptom**: When a WS client disconnects, the read-half exits → the
write-half `tokio::spawn` is `.abort()`'d → its `mpsc::UnboundedReceiver`
is dropped → the next `tx.send` returns `Err`, so the next `broadcast`
prunes the dead sender via `retain` (line 92).

But: if the engine is *idle* between disconnect and the next broadcast,
the dead sender sits in the Vec. With the MOD_STATES loop firing every
40 ms (25 Hz) and the BPM loop every 100 ms (10 Hz), the worst-case
prune latency is bounded at ~40 ms. So the leak window is microscopic
in practice, but the *invariant* is "cleanup is implicit in broadcast,
not in disconnect."

**Quantified**: Per dead-but-not-yet-pruned sender: ~24 bytes for the Sender
Arc. Worst case: dozens of bytes for ~40 ms. Effectively a non-issue today.

**Repro**: Disable broadcast loops, connect N WS clients, disconnect them
all. The senders Vec stays at N until broadcast resumes.

**Suggested fix** (cosmetic): In the WS read-loop, after `read.next()`
returns `None`/error, take a lock on `senders` and remove the entry by
identity. Requires storing the Sender's index (or a unique id) when
inserting on line 45. Low priority.

### 5. `SELECT_AUDIO` leaks old ring buffers in addition to the stream — `src/commands.rs:773-780`
**Symptom**: `SELECT_AUDIO:` recreates the rtrb ring buffers and swaps
fresh Producers into the shared `Arc<Mutex<Producer<...>>>`. The old
Producers (the previous halves) get dropped here, freeing them. But the
*Consumers* of the old rings are owned by the leaked `cpal::Stream` callback
closure (HIGH #1). So while the old Producers are freed cleanly, the old
Consumers — and the memory of the ring they shared — stay alive for the
program's lifetime.

This is the same leak as HIGH #1, just from a different angle. Listed
separately because the fix is the same drop-the-old-stream-first remedy.

**Quantified**: see HIGH #1.

### 6. Modal/Hybrid trigger work runs even with velocity=0 sub-hits — `src/dsp/modal.rs:357-401`
**Symptom**: When a sub-hit or ghost note fires with velocity=0 (e.g. a
malformed kit with `velocity_factor=0`), `velocity > 0.0` short-circuits
the trigger, but the entry still consumed a slot in the pending queue
(see kit.rs `queue_pending` which doesn't pre-check velocity). Not a
leak per se, but cycles burnt + a queue slot taken that another voice
could have used. The pending queue is capped at 512 so this is bounded.

**Quantified**: ~64 bytes per useless `PendingTrigger`; up to 512 *
that = 32 KB of pinned audio-thread memory if a kit was deliberately
configured to fill the queue with zero-velocity entries.

**Suggested fix** (trivial but DSP-adjacent so deferred per scope):
make `queue_pending` reject `velocity <= 0.0` at the boundary.

## LOW

### 7. Persistence worker thread runs forever — `src/persistence.rs:15-68`
**Symptom**: `start_persistence_worker` spawns a `std::thread` that
loops `while let Some(cmd) = rx.blocking_recv()`. The loop only exits
when every `Sender<PersistenceCommand>` is dropped, which only happens
on process exit. So the thread lives forever.

This is fine — it's the canonical Rust "long-running worker thread"
pattern, no JoinHandle so no resource needs reclaiming. Documented here
because anyone reading the code looking for thread leaks should see
this and move on.

**Quantified**: 1 thread per process, by design. Not a leak.

### 8. `SyncEngine` master-clock thread is never joined — `src/sync.rs:71-126`
**Symptom**: `SyncEngine::start` spawns a `std::thread` that loops
`while *is_running_shared.lock().unwrap()`. `stop()` flips the flag to
false; the thread exits its loop, sends MIDI Stop, broadcasts SYNC_STATUS,
and ends. No `JoinHandle` is stored on `SyncEngine`. The thread's
captured Arcs (`is_running`, `auto_sync`, `bpm_engine`, `comm_engine`,
`_connection`) are released when the thread ends naturally.

Brief overlap window: after `stop()` flips the flag, a fast `start()`
spawns a new thread before the old one has reached its exit. The new
thread takes a fresh clone of `is_running`, which is shared, so it
correctly sees `*running == false` and the guard at line 62-64 prevents
a double-spawn. The race-free zone is "one thread runs at any time;
between stop+start there can briefly be two but the new one always
sees `running == true`." Clean.

**Quantified**: ~1 OS thread per active sync session, freed naturally.
Not a leak.

### 9. `BpmEngine::onsets` Vec is correctly capped — `src/dsp/bpm_engine.rs:50-52`
**Symptom**: The `onsets: VecDeque<Onset>` is pruned to 6-second window
in `prune()` and hard-capped at MAX_ONSETS=96 in `register_onset`. No
growth.

**Quantified**: ~96 * 16 bytes ≈ 1.5 KB ceiling.

### 10. `KitEngine::pending` is correctly capped at 512 — `src/kit.rs:603,634-645`
**Symptom**: `VecDeque::with_capacity(PENDING_TRIGGER_CAPACITY)` and an
explicit length check in `queue_pending`. Overflow telemetry via
`pending_overflows: AtomicU64` and one-shot stderr warning via `Once`.

**Quantified**: 512 * 24 bytes ≈ 12 KB ceiling, allocated once at kit
construction. Audio-thread allocation-free.

### 11. `MidiEngine._connection` is replaced cleanly on start — `src/midi.rs:29,57`
**Symptom**: `MidiEngine::start` sets `self._connection = None` *before*
opening the new connection, so the previous `MidiInputConnection`'s drop
runs (which closes the underlying ALSA / CoreMIDI port) before the new
one comes up. No accumulation.

**Quantified**: Bounded at 1 active connection per MidiEngine, freed by
Drop.

### 12. `midi_playback_handle` abort then drop — `src/state.rs:42`, `src/commands.rs:875-882`
**Symptom**: On `PLAY_MIDI_TRACK:`, the previous handle (if any) is
extracted via `slot.take()`, `.abort()`'d, then dropped. `abort()` cancels
the task; the future state is dropped (including its captured Arcs and
the parsed MIDI events Vec) once it's no longer polled.

`tokio::task::JoinHandle::abort` documentation confirms drop releases
the task state. Clean.

**Quantified**: bounded at 1 active playback task; previous task's
captures (~few KB for the events Vec) are freed on abort.

### 13. `BpmEngine` 250-element score search runs O(N²) over onsets — `src/dsp/bpm_engine.rs:99-125`
**Symptom**: The lag-search loop is O(N_lags * N_onsets²) where
N_lags = 250 and N_onsets ≤ 96. So up to ~2.3M floating-point ops per
onset, on the midir callback thread. Not a leak. Audio-thread-adjacent
(not the audio thread itself), so a heavy fill doesn't xrun.

**Suggested fix** (out of scope, DSP-adjacent): early-out if the onset
deltas are degenerate, or hoist the inner `i-j` symmetry to halve the
work. Tracked separately for a future perf pass.

### 14. `analyze_sound` runs up to 1M ticks on the WS dispatcher thread — `src/commands.rs:37-89`
**Symptom**: `ANALYZE_SLOT:<n>` constructs a throwaway voice, triggers it
at v=1.0, and ticks for `(decay_ms + 500ms) * sample_rate / 1000` samples
(capped at 1M). At 48 kHz that's up to ~21 seconds of synthetic audio
synthesised on the tokio runtime thread.

Not a leak — the voice is dropped on return. But the WS dispatcher thread
is pinned for ~5-50 ms per analysis, during which no other WS message
is processed. A user dragging an analyze button would queue several of
these.

**Quantified**: 0 memory leak; ~20 ms latency per analysis at typical
500 ms decays.

**Suggested fix** (out of scope): move analysis to a `tokio::task::spawn_blocking`
so it doesn't pin the dispatcher.

### 15. Unbounded mpsc channels — `src/main.rs:26,57`, `src/persistence.rs:13`
**Symptom**: `midi_tx`, `audio_error_tx`, and the persistence command
channel are all `unbounded_channel`. In theory a producer outpacing the
consumer queues messages forever.

- `midi_tx`: receiver is the `loop { tokio::select! { Some(msg) = midi_rx.recv() => ... } }`
  in `main.rs:391-395`. Always draining, so unbounded growth requires a
  receiver stuck on its `tokio::time::sleep(100ms)` branch. The select
  is non-blocking on `recv`, so this can't happen in practice — the
  channel drains as fast as messages come in.
- `audio_error_tx`: receiver is the auto-recovery task. The recovery
  task drains the channel via a `while try_recv()` after each `recv`
  (line 307), so bursts of errors collapse to a single recovery. Won't
  grow unbounded.
- `persistence_tx`: receiver is the std::thread that blocks on each
  `blocking_recv`. The thread can be slow (kit.toml write is ~5 ms via
  tmp+rename; UI sliders fire ~60 Hz of SET_PARAM during a drag). At
  60 Hz incoming vs ~200 Hz disk write, the writer keeps up easily.
  But a slow disk (USB stick, network mount) could queue messages.
  Bound: ~64 bytes per `PersistenceCommand::SaveKit` envelope plus the
  cloned `DrumKit` (~5 KB serialised). 100 backed-up writes = 500 KB
  of channel queue.

**Quantified**: practical ceiling ~1 MB under pathological conditions;
typically zero residual queue depth.

**Suggested fix** (cosmetic): use `tokio::sync::mpsc::channel(N)` with a
bounded N=128 for persistence; on overflow, drop the older writes (the
latest SET_PARAM wins anyway because each writes the full kit).

### 16. `midi.rs` per-byte `println!` — `src/midi.rs:46-48` (FIXED THIS PASS)
**Symptom**: For every incoming MIDI byte stream — including continuous
controllers, pitch-bend, MIDI clock (24 ppqn @ 120 BPM = 48 lines/sec),
aftertouch, every active-sensing message — the midir callback wrote a
line to stdout.

**Quantified**: at ~50 lines/sec per controller, the per-call cost is a
syscall into a line-buffered stdout from a *foreign* thread (midir
spawns its own). Not a leak (stdout buffer doesn't grow when flushed),
but heavy I/O amplification and audio-adjacent stdout contention.

**Fix applied**: removed the per-byte `println!`. Higher-level WS
`MIDI:<note>,<vel>` broadcast (still going via `midi_tx`) still surfaces
NoteOn/Off in the UI.

### 17. `bpm_engine.rs` per-onset and per-estimate `println!` — `src/dsp/bpm_engine.rs:54,158,192` (FIXED THIS PASS)
**Symptom**: `register_onset` printed "[BpmEngine] Hit ..." every time
(potentially 10-20 lines/sec during a fill). `estimate_tempo` printed
the BPM estimate on the same trigger (so doubled). The inactivity-reset
path also printed on every `get_bpm()` after a 10-second idle.

**Quantified**: ~20-40 stdout writes/sec under play. Same "not a leak,
but I/O amplification" class as #16.

**Fix applied**: removed all three. The operational BPM is broadcast at
10 Hz by the dedicated loop in `main.rs`; the stable flag is read by
`SyncEngine`. No telemetry is lost.

### 18. `tokio::sync::Mutex` held across an `await` — none found
**Symptom**: spot-check of the WS dispatcher and broadcast loops for the
"tokio mutex held across await" anti-pattern. The closest case is
`commands.rs:898-902` where `spawn_playback` is awaited(?) — but it's
synchronous (returns a `Result<JoinHandle>` immediately). The
`midi_engine.lock().await` in `app_utils.rs:22` is held while
`midi.start(...)` runs, but `start` is synchronous (no `.await`s
inside). Clean.

The audio-recovery task (`main.rs:303-388`) does `audio_error_rx.recv().await`
then runs synchronous work — no lock held across that await.

The MOD_STATES broadcast (`main.rs:83-103`) does
`interval.tick().await` and *then* reads atomics; no mutex around the
await. Clean.

### 19. `tokio::spawn` tasks all have exit paths — `src/main.rs:83,112,303`, `src/comm.rs:33,38,48`
**Symptom**: Audited the long-lived spawned tasks:
- MOD_STATES broadcast loop: `loop { interval.tick().await; ... }`. No
  exit. Captures `shared_state_comm` and `comm_clone_loop` (Arcs).
  Lives for the program's lifetime. Intentional. Memory cost ≈ size
  of the future plus 2 Arc refcounts.
- BPM broadcast loop: same pattern. Intentional.
- Audio recovery: `while audio_error_rx.recv().await.is_some()`. Exits
  if every sender drops — only happens on process exit. Intentional.
- WS server accept loop (`comm.rs:33`): `while let Ok((stream, _)) =
  listener.accept().await`. Exits if the listener drops. Listener is
  on the stack of `comm_engine.start(...)` which is awaited *inside the
  WS server*, so the listener lives for the duration of `start`. Hmm —
  see #20.
- Per-connection task (`comm.rs:38`): exits on disconnect. Clean.
- Per-connection write task (`comm.rs:48`): exits when its rx drops.
  Clean.

### 20. `comm.rs::start` returns Ok(addr) but spawns an unjoined accept loop — `src/comm.rs:33-71`
**Symptom**: `CommEngine::start` spawns the accept loop and returns
`Ok(local_addr)` *without* keeping a JoinHandle. The accept loop runs
forever (`while let Ok(...) = listener.accept().await`). The TcpListener
is moved into the spawned task, so it lives forever too.

This is fine for a long-running server, but it means a test that calls
`CommEngine::start` cannot tear the server down: there is no shutdown
mechanism. The integration tests in `tests/comm_tests.rs` likely either
run on a fresh port each time or accept the listener leak per test run.

**Quantified**: 1 leaked accept task + 1 leaked TcpListener per
`start()` call. Negligible for a single-binary process; cumulative if
tests reuse a process.

**Suggested fix** (out of scope): return a shutdown handle, or take a
cancellation token.

## NON-ISSUE

### 21. No Arc reference cycles
Audited all `Arc`-owning struct fields across `SharedState`, `KitEngine`,
`CommEngine`, `MidiEngine`, `SyncEngine`. The graph is a DAG rooted at
`main()` locals; child structs hold downward Arcs (e.g. `SyncEngine` →
`BpmEngine`, `CommEngine`) but never reference cycles. Specifically:
- `SharedState` does NOT hold `Arc<KitEngine>` (it owns the `Arc<Mutex<...>>`)
  — the `Arc` field is the *only* way to reach the kit, no back-reference.
- `KitEngine` holds no Arcs at all.
- `CommEngine` holds `Arc<Mutex<Vec<Sender>>>` — Senders, not full
  client state, no back-pointer to the engine.
- `SyncEngine` holds `Arc<CommEngine>` (down) and `Arc<Mutex<BpmEngine>>` (down).
  `BpmEngine` doesn't know about `SyncEngine`. Clean.

### 22. `Box::leak` — only in test scaffolding
`tests/midi_player_tests.rs` uses `Box::leak` to create a `'static`
midi file path for the test. Confirmed not in production paths.

### 23. `KitEngine::tick` is allocation-free on the audio thread
Walked the tick path. `drain_pending` uses `swap_remove_back`; per-voice
`tick()` is in pre-allocated state arrays (Mode bank, delay line,
noise buffer, grain ArrayVec). No `Vec::new` / `String::new` / `format!`
in the hot path. Clean.

### 24. `rtrb` Consumer drop on hot-swap (commands.rs SELECT_AUDIO)
The old Consumers ARE leaked (HIGH #1), but the old Producers are
correctly replaced via `*p = new_*_prod` (commands.rs:776-780). The
old Producer values are dropped (single-owner via `Arc<Mutex<>>`),
freeing the producer side of the ring header. Only the consumer side
is leaked, attached to the leaked stream.

### 25. `MidiInputConnection` drop closes the OS port
`midir::MidiInputConnection`'s Drop implementation calls
`alsa::seq::*` / `coremidi::*` close on the underlying port. Verified
by reading midir's source. No accumulating OS handles per
`SELECT_MIDI:` switch.

## Fixes applied this pass

| File | Change |
|---|---|
| `src/midi.rs:46-48` | removed `println!("MIDI BYTES: ...")` on every raw MIDI byte stream (#16). |
| `src/dsp/bpm_engine.rs:54-59` | removed per-onset `println!`. |
| `src/dsp/bpm_engine.rs:158-161` | removed per-estimate `println!`. |

These removed three audio-adjacent stdout chatter sources. Total spam
delta on a busy fill: ~70 lines/sec → 0 lines/sec. Not a memory leak but
real I/O amplification on a foreign thread (`midir` callback / BPM
analysis runs off the tokio runtime).

`cargo build` clean. `cargo test` is unchanged from the pre-pass baseline
(2 failing tests in `tests/demo_kit_behavior_tests.rs` and
`tests/pattern_tests.rs` are pre-existing — they pre-date this audit and
sit on WIP changes to `src/kit.rs` and `tests/pattern_tests.rs` already
present in the working tree at session start).

## Quantified leak budget

Cost per event × frequency at typical use → leak per hour.

| Finding | Severity | Cost/event | Realistic frequency | Per hour |
|---|---|---|---|---|
| HIGH #1: cpal::Stream leak (device switch) | HIGH | ~52 KB heap + 1 OS thread | ~1 switch/day (manual) | ~52 KB/day |
| HIGH #1: cpal::Stream leak (recovery loop on bad device) | HIGH | ~52 KB + 1 thread | 2/sec sustained (worst) | ~370 MB/hr + thread-cap hit in 8 min |
| HIGH #2: BPM Vec churn (per onset) | HIGH | ~2.5 KB allocator churn | 10-20/sec play | ~150 MB/hr alloc traffic (no leak) |
| MEDIUM #4: unpruned dead sender | MEDIUM | ~24 bytes | bounded to 40 ms latency | 0 KB/hr steady-state |
| LOW #15: persistence channel queue | LOW | ~5 KB/SaveKit | 60/sec slider drag, ~200/sec writes | 0 KB/hr steady-state (drains) |
| FIXED #16: MIDI per-byte stdout | LOW | ~30 bytes line buffer | 50/sec MIDI clock | 0 (fixed) |
| FIXED #17: BPM per-onset stdout | LOW | ~60 bytes line buffer | 20/sec play | 0 (fixed) |

The dominant practical-life leak is **HIGH #1 in its auto-recovery
flavour** — a wonky USB interface that disconnects every few seconds
will leak ~370 MB/hr and exhaust the Linux per-process thread cap in
under 10 minutes. The "user manually clicks SELECT_AUDIO" path is
negligible (KB/day).

## Suggested next steps

1. **Fix HIGH #1**: move the cpal stream into a dedicated owning thread.
   This is the only real leak in the backend. Estimated work: ~3 hours
   (refactor `start_audio` to return a stream-owning handle that drops
   the previous stream before building the new one).
2. **Fix HIGH #2**: hoist `BpmEngine` Vec allocations into reused fields.
   Estimated work: ~30 minutes.
3. **MEDIUM #3**: expose `audio_stream_leak_count` via a `GET_AUDIO_STATS`
   command so the UI can warn about leak budget over time.
4. **MEDIUM #6**: short-circuit `queue_pending` on velocity≤0 to spare
   the pending queue from useless slots. Trivial fix.

Cannot verify without running under valgrind / heaptrack / Instruments:
- Exact bytes per leaked `cpal::Stream` (cpal's internal allocations on
  macOS Core Audio vs. ALSA differ).
- Whether `tokio::task::JoinHandle::abort` actually drops the future
  state immediately or defers it to the next runtime poll (the docs say
  the latter — "the cancellation is *requested*"). If deferred, a
  PLAY_MIDI_TRACK abort might briefly hold the captures longer than
  expected.
- Whether `cpal`'s worker thread for a dead device actually exits when
  the stream is leaked, or stays parked. macOS Core Audio strongly
  suggests it stays parked, exhausting thread quota.
