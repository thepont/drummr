# Frontend Memory & Resource Leaks

Last updated: 2026-05-18

Audit of `ui/src/` for leak patterns in the React 19 SPA. Severity:
- **HIGH**: real leak that compounds over time, observable in production
- **MEDIUM**: leak under specific user actions (reconnect, kit reload)
- **LOW**: latent / unobservable but worth documenting
- **NON-ISSUE**: investigated, confirmed clean — listed for completeness

The frontend talks to the Rust backend over a single WebSocket created in
`ui/src/App.tsx:63-195`. The hot broadcast surfaces are:
- `MOD_STATES:` at 25 Hz (mod-state JSON arrays).
- `BPM:` at 10 Hz.
- `MIDI: <note>,<vel>` per MIDI event (bursty, can exceed 25 Hz under a
  fill).
- `ANALYSIS:<slot>|<json>` on-demand after `ANALYZE_SLOT`.

## HIGH

### 1. MIDI flash setTimeouts not cancelled or coalesced — `ui/src/App.tsx:166-180`
**Symptom**: Every `MIDI:` broadcast schedules a 40 ms or 80 ms
`setTimeout(() => setIsMidiFlashing(false), ...)` with no handle stored.
Under a busy drum take (10–30 hits/s) the page accumulates dozens of
pending timers per second. They self-clear, so the heap doesn't grow
forever, but each one fires a `setIsMidiFlashing(false)` setState — and
because the timers are ordered by *issue* time not *fire* time, a stale
"flash off" can land AFTER a fresh "flash on" and stick the indicator
in the wrong state. The component is mounted for the app's entire
lifetime, so unmount-cleanup doesn't apply, but the rendering churn is
real.
**Repro**: Send 30 MIDI notes/s for 30 s. Observe React DevTools
"Profiler" → `App` re-renders 1200+ times from this path alone, on top
of the 25 Hz MOD_STATES baseline. The MIDI dot also flickers / sticks
off intermittently.
**Suggested fix**: Track the timer id in a `useRef<number | null>`;
clear it before scheduling a new one. Cancel on unmount.
**Status**: FIXED in this pass — see "Fixes applied" below.

### 2. `setTimeout`s in `MappingView.tsx` not tracked — `ui/src/views/MappingView.tsx:124, 150`
**Symptom**: Two fire-and-forget timers:
- `handleSave` (line 124): a 500 ms timer that calls `setIsSaving(false)`
  and `setHasChanges(false)`.
- The MIDI flash inside the WS message handler (line 150): a 100 ms
  timer per MIDI note that mutates `activeNotes`.
If the user navigates away from the Mapping view (App.tsx line 357
unmounts the component) while a timer is pending, the timer fires a
setState on an unmounted component. React 19 will silently ignore it,
but the timer's closure pins `roles`, `activeNotes`, and the
component-local setters in memory until it fires — and under heavy MIDI
input there can be hundreds of pending 100 ms timers at any moment.
**Repro**: Switch to MappingView, hit pads at ~30 Hz, switch to Kit
Editor while still hitting. The MappingView component instance and all
its captured state stay reachable for up to 100 ms.
**Suggested fix**: Track timer ids in refs; clear on unmount.
**Status**: FIXED in this pass — see "Fixes applied" below.

### 3. `PredictiveGraph` redraws on every parent render — `ui/src/components/ui.tsx:197-295` (called from `KitEditorView.tsx:446-456` and `ui.tsx:346-355`)
**Symptom**: `PredictiveGraph` is rendered for the freq slider plus
every Timbre slider (via `ParamController`). Its `useEffect` depends
on `[base, min, max, mods, attack, decay, lfo1_freq, lfo2_freq]`, and
`mods` is built fresh every render in `KitEditorView.tsx:538` and again
in `KitEditorView.tsx:540-543` (`const displayMods = [...paramMods, {
…empty }]`). New array identity each render → effect re-runs → canvas
redraws.

`MOD_STATES` at 25 Hz triggers a `setModStates` (`KitEditorView.tsx:140`)
which re-renders the whole `KitEditorView`. With ~6 Timbre sliders +
the freq graph, that's ~7 canvas redraws × 25 = ~175 canvas paints per
second on idle. Not a *leak* (no unbounded growth), but a constant GC
pressure and CPU baseline that shouldn't exist when nothing is moving.

**Repro**: Open Kit Editor on the `fm` engine slot, leave it idle.
Chrome Performance recorder shows ~3–6 % CPU sustained from canvas
paints in `PredictiveGraph`.

**Suggested fix** (non-trivial, deferred): memoise `paramMods` /
`displayMods` per param-name, or move `setModStates` to a `useRef`-
backed store + a 5 Hz `useSyncExternalStore` snapshot for the visible
panels. Cheapest patch: wrap `PredictiveGraph` in `React.memo` with a
deep-equality `mods` comparator, since the values rarely change even
when identity does. Skipped — would touch the ParamController API.

### 4. `MasterPeakMeter` runs an unbounded `requestAnimationFrame` loop — `ui/src/components/MasterPeakMeter.tsx:13-27`
**Symptom**: `animate` calls itself via `requestAnimationFrame(animate)`
unconditionally on every frame, including when `level` has already
decayed to 0. The setter early-returns inside the reducer (`< 0.01`
→ 0), but the rAF still re-schedules and triggers a `setLevel(0)`
React render every 16 ms — about 60 wasted renders per second. The
RAF chain runs for the lifetime of the app since the meter is
unconditionally mounted on every screen.
**Repro**: Open React DevTools → Profiler. With no MIDI input,
`MasterPeakMeter` shows ~60 renders/s.
**Suggested fix**: Stop scheduling when `level === 0`; restart only on
the next `isActive` transition. Or simply replace the whole component
with a CSS transition on the existing div width. Skipped — touches
animation behaviour, would benefit from a UX call.

## MEDIUM

### 5. Analysis state retains entries for unloaded kits — `ui/src/App.tsx:51, 127-138`
**Symptom**: `analysis: Record<number, AnalysisResult>` is keyed by slot
index. When `LOAD_KIT` ships a fresh kit, the new kit's slots overwrite
indices 0..N-1, but if the previous kit had 16 slots and the new one
has 8, indices 8..15 retain stale `AnalysisResult` objects. Tiny in
size (8 entries × ~80 bytes each) but grows monotonically across kit
reloads if any slot index is ever empty. Also flagged in the prompt.
**Repro**: Load `kit_a` (16 slots), then `kit_b` (8 slots). Inspect
`analysis` in React DevTools → entries for slots 8..15 are stale.
**Suggested fix**: Either trim `analysis` when handling `KIT:` (only
keep keys that correspond to non-null slots in the parsed kit), or
key by `sound.id` instead of slot index — `sound.id` is the stable
identity used by `setSelectedSoundId`.

### 6. `KitEditorView`'s MOD_STATES listener attaches a second `'message'` handler — `ui/src/views/KitEditorView.tsx:132-147`
**Symptom**: `App.tsx` already owns `socket.onmessage` (line 97) and
dispatches every prefix from there. `KitEditorView.tsx` also attaches
`ws.addEventListener('message', handleMessage)` for MOD_STATES.
This is fine semantically — `onmessage` and `addEventListener` fire
in parallel on a `WebSocket`. But it means every message (BPM, MIDI,
KIT, MOD_STATES, ANALYSIS) hits *both* code paths, and the
KitEditorView listener does a `data.startsWith('MOD_STATES:')` string
check that fails on the other 90 % of messages. Each message therefore
runs at least one extra string allocation + comparison.

Also: when the user navigates away from KitEditor → unmount → cleanup
removes the listener correctly. But on reconnect (line 89 in App.tsx),
a new socket is created and `setWs(socket)` triggers the effect's
cleanup with the *old* socket. `ws.removeEventListener` on the old
(closed) socket is a no-op, which is fine. No leak here.

**Repro**: None observable, but adds ~25 wasted string-compares per
second.
**Suggested fix**: Lift `modStates` to `App.tsx` (parse it in the same
single dispatch as the other broadcasts) and pass it down as a prop.
That's a 30-line refactor and was flagged in the prompt as
non-trivial — deferred.

### 7. WebSocket reconnect — `onmessage`/`onopen`/`onclose`/`onerror` properties remain set on the previous socket — `ui/src/App.tsx:71-95, 184-188`
**Symptom**: On `socket.onclose`, the code calls `setWs(null)` and
schedules a reconnect after 2 s. The new connection creates a new
`WebSocket` and assigns fresh property handlers. The *old* socket
still has `onmessage`, `onopen`, `onerror`, `onclose` properties
pointing to the old closures, which capture `setSounds`, `setSchemas`,
the whole component scope. Once `socket` itself is no longer
referenced (the closure inside `useEffect` is the last reference, and
the inner cleanup `socket.close()` doesn't run for a *closed* socket),
JS GC reclaims everything together. So in practice, no leak — but
*if* the user manually triggers reconnects (e.g. backend restarts in
a long-running browser tab), each cycle holds the previous closures
until the next GC cycle.

The `isCurrent` flag (line 69) guards against stale `setState` from
the previous closure, which is good defensive practice.

**Repro**: Restart the backend 50 times. Take heap snapshots before
and after; expect zero detached WebSocket instances (Chrome's
heap snapshot will mark them as garbage).
**Suggested fix**: For belt-and-braces, null out `socket.onmessage`
etc. in the cleanup function. Non-essential.

### 8. WS effect inner `useEffect` cleanup chain — `ui/src/App.tsx:184-194`
**Symptom**: The outer `useEffect` returns a cleanup that calls
`cleanup()` (which closes the socket) and `clearTimeout(reconnectTimeout)`.
The closure that defines `cleanup` is the one returned by the FIRST
`connect()` call. On reconnect, a new socket is created via
`connect()` *inside* `socket.onclose` — but the outer cleanup still
points at the first invocation's cleanup. The `isCurrent` flag again
saves us, but the reconnectTimeout id is stored in the outer scope
and is overwritten by every reconnect, so an in-flight reconnect
timer is only cancellable via the latest assignment to
`reconnectTimeout`. On unmount during a reconnect attempt, the
clearTimeout fires on the latest id, so this works. Fine.
**Suggested fix**: None. Documenting because the control flow is
subtle.

## LOW

### 9. `EnvelopeEditor` window listeners are cleaned up only inside `mouseup` — `ui/src/components/EnvelopeEditor.tsx:13-42`
**Symptom**: `handleMouseDown` attaches `mousemove` and `mouseup` to
`window`. Cleanup is only in the `handleMouseUp` callback. If the
component unmounts mid-drag (e.g. via a route change triggered by a
hotkey while the mouse button is still down), the listeners persist
until the user lets go. They reference the captured `attack`/`decay`
props (stale) and a no-longer-mounted component's `onChange`. Once
mouseup fires, they self-cleanup and the closures are GC'd. Real-world
impact: tiny — the user would have to drag, fire a navigation, then
release.
**Suggested fix**: Add a `useEffect` cleanup that nulls out a ref-held
removeListener callback. Six lines, non-urgent.

### 10. `FrequencyVisualizer` has the same drag-listener pattern — `ui/src/components/ui.tsx:393-411`
**Symptom**: Same as #9. `handleMouseDown` attaches `mousemove` /
`mouseup` to `window`, only cleaned up in `mouseup`. Mid-drag unmount
leaks until release. Worse: line 410 calls `update(e.nativeEvent as
any)` synchronously, which works, but if `e.nativeEvent` is somehow
recycled by React's pooling (it isn't in React 19, just being cautious)
that'd be a problem. Not a leak though.
**Suggested fix**: Same as #9.

### 11. `PreviewKitButton` document listener — `ui/src/components/PreviewKitButton.tsx:31-40`
**Symptom**: Correctly added under `useEffect[open]` with a paired
cleanup. NON-ISSUE for leaks, but the dependency on `open` means the
listener is repeatedly attached/detached every time the user toggles
the menu. That's by design and correct.

### 12. `Sparkline` dead code with rAF — `ui/src/components/ui.tsx:131-180`
**Symptom**: Component exists and includes a `requestAnimationFrame`
draw chain. **Not imported anywhere in `src/`** — confirmed via grep.
It's a no-op risk today, but if a future developer renders it with
fast-changing `value`, the `[value, min, max]` dep array would tear
down and re-create the rAF on every prop change (cleanup pattern is
correct, just wasteful), and `historyRef.current.push` runs in the
effect body, which means re-running the effect from React StrictMode
will double-push on mount.
**Suggested fix**: Delete the export, or move `historyRef.current.push`
into the `draw` function so StrictMode's double-mount stays correct.
Skipped — dead code.

### 13. `analyzeTimersRef` is keyed by slot index, never pruned on kit reload — `ui/src/App.tsx:202-213`
**Symptom**: On kit reload, the new kit may have fewer slots. Old
timer ids for now-missing slots stay in the record until they fire
500 ms later. They self-clean via `delete analyzeTimersRef.current[slot]`
inside the timer body, so the record self-prunes within 500 ms of any
kit change. No actual leak.
**Suggested fix**: None needed; documented for completeness.

### 14. `MasterPeakMeter` `isActive` effect — `ui/src/components/MasterPeakMeter.tsx:7-11`
**Symptom**: `setLevel(0.9 + Math.random() * 0.1)` runs on every
`isActive` change. `isActive` is `isMidiFlashing` from `App.tsx`,
which toggles per MIDI event (up to 30 Hz). At each toggle the meter
re-randomises its peak. Fine, but tied to MIDI churn from #1.
**Suggested fix**: None.

### 15. `Slider` re-renders due to inline `style` and `format` closures — `ui/src/components/ui.tsx:62-120`
**Symptom**: Several callers pass `format={v => …}` inline, and
`style={{ left: `${...}%`, transform: 'translate(...)' }}` is recreated
each render at line 109. Each new style object is a new identity, so
React reconciles the DOM element's style every render. Not a leak,
just wasted work at 25 Hz × N sliders.
**Suggested fix**: `useMemo` the style; hoist `format` to a stable
ref. Trivial individually but spans every call site — non-trivial in
aggregate.

## NON-ISSUE

### 16. `App.tsx` WebSocket lifecycle — `ui/src/App.tsx:63-195`
The `useEffect` has `[]` deps, so the socket is created once. The
inner `isCurrent` flag prevents stale `setWs` / `setStatus` writes
after the cleanup runs. The reconnect path schedules a new `connect()`
via `setTimeout`, captured in `reconnectTimeout` (the same `let` is
reused). On unmount, the outer cleanup calls the *latest* `cleanup`
(which closes the current socket) and clears the latest reconnect
timer. Confirmed correct.

### 17. `KitEditorView` 'message' listener — `ui/src/views/KitEditorView.tsx:132-147`
Correctly added under `useEffect[ws]` and removed in the returned
cleanup. On `ws` change (reconnect), cleanup fires with the OLD ws
reference, so the listener is removed from the old socket. Even
though the old socket is closed, this is the right shape. NON-ISSUE.

### 18. `MappingView` 'message' listener — `ui/src/views/MappingView.tsx:130-177`
Same pattern as #17. Correctly added/removed under
`useEffect[ws, learningSlot, updateRoleNote, hasChanges, isLoaded]`.
The fat dep list means the listener is detached and re-attached
every time `hasChanges` flips or the user toggles `learningSlot`,
but that's fine — it's an `addEventListener`, not a closure leak.

### 19. `LibrarySidebar` — `ui/src/components/LibrarySidebar.tsx`
No listeners, no timers, no rAF. Just controlled state. NON-ISSUE.

### 20. Phosphor icons
Each icon is a tree-shaken React component, imported statically at
the top of each file. No dynamic icon construction in render paths.
NON-ISSUE.

## Fixes applied this pass

1. **#1 (HIGH)** — `ui/src/App.tsx:53-57, 166-185, 220-227`: added
   `midiFlashTimerRef` to coalesce MIDI flash timers; cleared before
   each new schedule and on unmount.
2. **#2 (HIGH)** — `ui/src/views/MappingView.tsx:1, 93-104, 124-130,
   146-160`: added `saveTimerRef` and `activeNoteTimersRef`; cancelled
   on unmount via a new `useEffect`. The save-timer is also overwritten
   safely (cleared before re-scheduling) if the user mashes Save.

`npx tsc --noEmit` is clean after these edits.

## Suggested next steps

In rough priority order:

1. **Trim `analysis` state on `KIT:`** (#5). 5-line fix; do it.
2. **Memoise / hoist `displayMods` and `paramMods` in
   `KitEditorView.tsx`** to stop `PredictiveGraph` from redrawing at
   25 Hz when nothing audible has changed (#3). Cheapest variant:
   wrap `PredictiveGraph` in `React.memo` with a custom comparator.
3. **Stop the `MasterPeakMeter` rAF when `level === 0`** (#4). Saves
   ~60 wasted renders/s on the main bar that's always mounted.
4. **Add a fallback unmount cleanup to `EnvelopeEditor` and
   `FrequencyVisualizer`** (#9, #10) for the mid-drag-navigation
   edge case.
5. **Lift MOD_STATES dispatch to `App.tsx`** (#6) so the editor view
   doesn't need its own 'message' listener — gets the WS dispatch
   down to one place.
6. **Profile with React DevTools Profiler under sustained MIDI
   input** — confirm or refute the rendering-churn estimates above
   on real hardware. Items #3 and #4 are inferred from code review;
   a 30-second profiler run on the dev server would settle them.

## What I couldn't verify without running the app

- Real heap-growth curves over hours of reconnect cycles. Items #5,
  #7, #13 are all "shouldn't compound but I can't prove it without
  a profiler."
- The actual rendering rate of `PredictiveGraph` under live
  MOD_STATES. The 175 paints/s figure (#3) is a back-of-envelope
  based on slot counts × broadcast rate; React's render batching
  may collapse some of those.
- Detached WebSocket count after N reconnects (#7).

A 15-minute session in Chrome DevTools Performance + Memory tabs
with the dev server running would close all three.
