# Architecture

Dyna Island, a local, Windows-only "living notch" for Windows
10/11. A black island hangs from the top-centre of the screen, shows live
activities while collapsed (music, the focus timer, pop-ups, notices, the
readout over games, the next meeting) and expands on hover into pages: Music,
Focus, Tasks, Notes, Clipboard, Today, System, Agenda (an add-on), Dock,
Script (an add-on) and Add-ons, with Settings behind the gear.

**One network connection, and only when asked for.** The Outlook agenda
add-on reads the ICS link the person pasted (`services::agenda`, WinHTTP):
`https` on Outlook's own calendar hosts only (`poni_core::ics::check_link`),
redirects off, nothing sent but the request, the link never logged beyond
its host. Nothing else talks to the network: the System page reads the PC's
address from Windows' own address table, nothing downloads, and there is no
telemetry or update check. The only things the app hands to the shell are
local: the settings file, the data folder, Windows' startup settings and the
apps on the Dock.

It started as a fork of Oyenza Studio (MIT; see the README's credits), which
was called Poni inside. The code keeps that short name where people don't see it: the crates (`poni`, `poni-core`),
`poni.exe`, its folders (`%LOCALAPPDATA%\Programs\Poni`, `%APPDATA%\Poni`), the
uninstall key and Run value, `Local\Poni.Island` and the `PONI_*` variables.

## Crates

| Crate | What | Builds on |
|---|---|---|
| `crates/poni-core` | Pure logic + models, no OS calls beyond `std` file I/O (`store`): geometry and colour (`geom`, `look`, `glass`, `hdr`), springs and frame timing (`spring`, `frametime`, `timeline`), player metadata fix-ups and transport rules (`nowplaying`), Pomodoro (`pomodoro`), tasks (`todo`), quick notes (`notes`), the Outlook agenda's meetings and countdown (`agenda`, in local wall-clock seconds) and the calendar file it reads (`ics`: folded lines, `VTIMEZONE` rules, all-day events, `RRULE` daily to yearly with `BYDAY`/`BYMONTHDAY`/`BYSETPOS`, `EXDATE`, changed and cancelled occurrences; and the link check), clipboard history (`cliphist`), the Dock's apps (`dock`), the Script reader (`script`), add-ons (`addons`: built-in only, what the Add-ons page says), settings and the JSON store (`settings`, `store`), dates (`datetime`), the SVG path parser (`svgpath`), the System page's arithmetic (`sysmon`: CPU load, per-app usage from two process lists, the process list's binary layout, a minute of history, number formats), frame rates over games (`frames`), and the whole PC's power (`power`: the Corsair HXi/RMi and MSI Ai power supplies' USB protocols, and the estimate when a PSU doesn't report) | any OS (`cargo test -p poni-core`) |
| `crates/poni` | The app (Win32 + Direct2D + WinRT via `windows` 0.58) | Windows only (`cargo wcheck` / `cargo wbuild` cross from Linux) |

## Threads and data flow

```
 services (own threads / COM callbacks)          UI thread (owns everything below)
 ─────────────────────────────────────          ─────────────────────────────────
 media (GSMTC)      ─┐                           App ── AppState (state.rs)
 audio (volume)     ─┤  EventSink.send(AppEvent)  │      ├─ pages/*      draw via Ui, push Commands
 bluetooth          ─┼──► mpsc + PostMessage ───► │      ├─ activities  collapsed pill content
 fullscreen/appbar  ─┤     (WM_APP_EVENT)         │      └─ island       spring-animated outline
 clipboard          ─┤                            │
 imaging (icons)    ─┤
 sysmon / frames    ─┤ only while the System page or the games readout needs them
 watch (per process)─┤ one blocked thread per bell, until that process ends
 agenda (HTTPS)     ─┘ while the agenda add-on has a link: a read every 5 min / on the Agenda tab
 frame clock thread ── WM_APP_FRAME (display vblank, only while animating)
```

* **Rendering**: `gfx.rs`. The island window is a fixed-size, per-pixel-alpha
  layered window big enough for the largest island (widest, tallest page plus
  spring overshoot); the island is drawn centred in it, so a morph never moves
  or resizes the window and the centre stays on one pixel.
  * GPU renderer (default): Direct2D on a Direct3D 11 device (hardware, else
    WARP) draws into a premultiplied flip-model swap chain that
    DirectComposition shows on the window. The layered window's own bitmap is
    then an invisible hit mask (alpha 1/255 where the island takes the mouse,
    0 elsewhere), refreshed at ~30 Hz while the island moves and exactly at
    rest, so transparent pixels stay click-through.
  * Software renderer (fallback: no DirectComposition, e.g. Wine; repeated
    device loss; `PONI_RENDERER=software`): Direct2D in software into a DIB
    pushed with `UpdateLayeredWindowIndirect`, copying only the rectangle the
    island covered this frame and last.
  * Content that fades in or out during a morph is drawn into an offscreen
    layer and composited (`Canvas::layered`): group opacity, and its text is
    rasterised once instead of re-hinting while it scales.
  * The Glass theme (`glass.rs`, `res/glass.hlsl`) bends and frosts a copy of
    the screen under the window (`services::backdrop`, DXGI Desktop
    Duplication, kept in memory only and only while that theme is on).
  * All coordinates are **DIPs** (the render target DPI = monitor DPI), in
    window space: the island body is `Island::body_rect_at(cx)`.
* **Frame clock**: `window.rs`. A thread waits on the island display's vblank
  (`IDXGIOutput::WaitForVBlank`; DwmFlush or a steady timer as fallbacks) and
  posts `WM_APP_FRAME`. Timestamps are phase-locked to the display
  (`poni_core::frametime`), and each frame is stamped with the vblank it will
  be shown on, so every animation step is a whole number of refresh periods
  (smooth at 60, 144 or 240 Hz).
* **Idle = no frames**: frames are produced only while the island animates,
  a page calls `ui.request_anim()`, input arrives, or a live activity is
  showing. Real motion runs on every vblank; ambient motion on every n-th
  (the equalizer near 60 fps, a drifting progress bar near 30). Otherwise the
  frame clock thread is parked and one-shot timers wake the UI for the next
  visible change (countdown second, HUD expiry, minute tick). The one thing
  that reads at rest is the System page's kept minute (`system_history`, on
  by default): a light reading a second. It rests while the session is locked
  or the display is off, and on battery saver (`poni_core::sysmon::reads`).
  See "Keep it light" in AGENTS.md.
* **Window**: `WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE`.
  It never takes focus on click; text fields temporarily drop NOACTIVATE
  (`Command::FocusText`) and give focus back afterwards.
* **Helper window** (hidden): tray icon, AppBar registration (full-screen
  notifications), `WM_POWERBROADCAST`, `WM_CLIPBOARDUPDATE`.
* **Modal work** (the tray menu, the Dock's Open dialog) runs as
  `app::Deferred` after the window message that asked for it, never while the
  `App` is borrowed: those loops pump messages of their own.
* **Step aside** (Settings › Maximized windows, on by default): the
  full-screen watcher also keeps the app windows whose top edge is at the
  top of the island's monitor, within 8 DIPs, as `AppEvent::TopWindows`, from
  the foreground and location WinEvents it already listens to (no polling).
  Each frame the app checks whether one is under where the island rests and,
  if so, rests as `Mode::Aside`: the Hidden look's invisible strip until the
  pointer reaches it. Pop-ups, notices and a new song's title still drop down.

## Contracts

### Pages (`crates/poni/src/pages/*.rs`)

```rust
pub fn height(st: &AppState) -> f32;                      // content height in DIPs
pub fn draw(ui: &mut Ui, st: &mut AppState, area: Rect);  // draw + handle input
```

* `area` is the content rect below the header (x/y absolute in canvas DIPs);
  its width is `settings.expanded_width - 2*PAD` (default 620 - 36 = 584). The
  window fits the tallest page (System, 256): a new page stays at or below it.
* A page joins the header through `PageKind` in `pages/mod.rs` (its key, title,
  icon, `content_height`, `draw`, and `tabs` when it shows only sometimes).
* Immediate mode: `ui.button(id, rect) -> Resp { hovered, pressed, clicked }`.
  Ids from `wid("name")` / `wid_n("row", i)`; must be unique per frame. A
  widget registered later wins the hit test where two overlap.
* Widgets: `icon_button`, `pill`, `progress_bar` (returns the seek),
  `text_input_row` (returns submitted/cancelled), `wheel_in(rect)`.
* Drawing: `ui.c` is a `Canvas`: `fill_rrect`, `stroke_rrect`, `fill_circle`,
  `ring`, `arc`, `line`, `text(s, rect, &TextStyle)`, `measure`, `image`,
  `icon(Icon, rect, color)`, `clip/unclip`, `with_alpha`.
* Text: `TextStyle::new(size).weight(Weight::Semibold).color(c).center()/.right().tabular().wrap().top()`.
  Single-line text is ellipsized automatically.
* Colours: `poni_core::geom::palette` (island is pure black; text is white at
  0.96 / 0.62 / 0.50, disabled 0.30; accents GREEN, ORANGE, RED, BLUE, CYAN,
  PURPLE, YELLOW).
* Type scale: `gfx::ts` (11 / 12 / 13 / 15 / 18 / 30 / 50) via `TextStyle::caption()`,
  `footnote()`, `body()`, `headline()`, `value()`, `display()`, `hero()`. Nothing below 11.
* Buttons: `ui.pill(id, rect, label, icon, PillStyle::Utility | Neutral | Primary(c) | UtilityAccent(c))`
  at `PILL_SMALL` (28) or `PILL_REGULAR` (34) height, `PILL_GAP` 8 apart.
* Empty pages: `pages::empty_state(ui, area, icon, title, hint)` plus one
  primary pill under it (Script, Dock).
* Settings (`pages/settings.rs`) has no tab (`PageKind::tabbed`): the gear right of
  the tabs opens it, and pressing it again goes back. It changes what the tray menu
  changes: settings through `Command::SaveSettings` (applied as a tray change is),
  Start with Windows through `Command::SetAutostart` (read and set off the UI thread).
* Add-ons (`pages/addons.rs`) is the last tab, a "+": one row per add-on in
  `poni_core::addons::ALL` (the Script reader and the Outlook agenda) with a switch. Switching
  flips `settings.addons` through `Command::SaveSettings`; `App::settings_changed`
  sets `st.addons`. Add-ons are built in: nothing downloads.
* Script (`pages/script.rs`) is a tab only while its add-on is on
  (`AppState::script_on`). A `.txt` dropped on the island (`dragdrop.rs`,
  `AppEvent::FilesDropped`) becomes the script while that page shows.
* Notes (`pages/notes.rs`): `store.notes` (`poni_core::notes`), newest first.
  Enter adds (`text_input_row`), a click copies (`Command::CopyText`, kept out
  of the clipboard history), × deletes with Undo; `next_wake` covers the
  "Copied" flash and the Undo lapsing.
* System's bell (`pages/system.rs`, `processes`): on a hovered process row,
  and lit on a watched one. `Command::WatchProcess` starts `services::watch`
  for that pid; `AppEvent::ProcessExited` becomes a notice with how long it
  ran since the bell and, when non-zero, its exit code (`App::on_process_exited`).
* Agenda (`pages/agenda.rs`) is a tab only while the Outlook agenda add-on is
  on (`AppState::agenda_on`). With no link it explains where Outlook.com
  publishes one and offers Paste link (`Command::AgendaPasteLink`: the
  clipboard's text through `ics::check_link` into `settings.agenda_link`, and
  out of the clipboard history); with one, it shows `st.agenda` and a Remove
  link. `App::sync_agenda` starts or stops `services::agenda` with the add-on
  and the link, and `App::ask_agenda` reads when the link is set, each
  `AGENDA_EVERY_S` from the minute timer while someone is at the PC, and as
  the page opens. Offline, the meetings last read stay. `Activity::Meeting` takes the closed
  island from `agenda::LEAD_S` before the next meeting to `NOW_S` after it
  starts (`AppState::meeting_countdown`), ahead of music and the timer.
* Dock (`pages/dock.rs`): tiles for `store.dock` (`poni_core::dock`, up to
  `MAX_APPS`). A click sends `Command::LaunchApp` (`shell::open`, the same as a
  double-click: a shortcut keeps its arguments) and collapses the island; the
  × on a hovered tile takes the app off; Add sends `Command::PickDockApp`, which
  runs Windows' Open dialog (`picker.rs`, shortcuts not dereferenced) as
  deferred work and adds the pick (`App::add_dock_app`, with a notice). Icons
  come through `Command::LoadDockIcon` into `st.dock_icons`.
* Timed page changes (an Undo pill lapsing) report their deadline through
  `pages::next_wake` so the app wakes exactly then instead of polling.
  `st.accent` = Windows accent (lightened); `st.media_accent()` = album-art colour.
* Continuous animation: call `ui.request_anim()` while something must move
  every frame. Never animate idle.
* Side effects: push `Command`s (`st.cmd(...)`, `st.media_cmd(...)`, `st.save()`,
  `st.chime()`, `st.notify(...)`). Never call services from pages.
* Transient page state lives in `st.ui` (`PageUiState`).

### Services (`crates/poni/src/services/*.rs`)

* `start(sink: EventSink) -> windows::core::Result<Self>`; failures are logged
  and the feature is skipped. Never panic, never block the UI thread.
* Talk to the UI only through `sink.send(AppEvent::…)` (see `events.rs`).
* COM on worker threads: initialise with `CoInitializeEx(None, COINIT_MULTITHREADED)`.
* WinRT async: call `.get()` only on worker threads.
* Nothing here talks to the network but `agenda` (AGENTS.md).

### Events (`events.rs`) and commands (`commands.rs`)

`AppEvent` = everything services report. `Command` = everything pages request.
`app.rs` maps both onto `AppState` and service calls.

## Files

| File | Owner area |
|---|---|
| `app.rs` | message loop, island state machine, input, timers, commands, deferred modal work |
| `activities.rs` | collapsed modes + their geometry and drawing |
| `pill/*.rs` | the Pill theme (`Theme::Pill`, `look::PILL` with `Shape::Pill`): the island floats `pill::TOP_GAP` below the top edge (`Island::set_floating`); `shape` draws the floating outline, shadow and hit area, `motion` its springs, and one module per surface (`idle`, `status`, `media`, `live`, `hud`, `notice`, `expanded`, `music`) behind hooks in `activities.rs` and `pages` that fall back to Classic's drawing |
| `gfx.rs`, `glass.rs`, `icons.rs`, `ui.rs`, `island.rs`, `window.rs`, `png.rs` | rendering/UI foundation |
| `state.rs`, `events.rs`, `commands.rs` | shared contracts |
| `pages/*.rs` | expanded pages: `media` (Music), `focus`, `tasks`, `notes`, `clipboard`, `today`, `system`, `agenda`, `dock`, `script`, `addons`, `settings` |
| `services/*.rs` | watch (the System page's bell: waits for a process to end), agenda (the agenda add-on: a WinHTTP GET of the pasted link, the parsed meetings moved to the PC's clock with `SystemTimeToTzSpecificLocalTime`), media (Windows' media controls; Apple Music's own progress slider and back button through UI Automation in `applemusic_ui`), audio, bluetooth, fullscreen (AppBar and top windows), clipboard, imaging (WIC decoding, shell thumbnails and app icons on one worker thread, for the System page and the Dock), backdrop (the Glass theme's screen copy), power, sound, sysstats (Today's vitals), sysmon (the System page's readings: each second while the page is open or a game's readout shows, a light reading a second for the kept minute while it's closed; all of it rests while nobody is at the PC, the kept minute also on battery saver; on a desktop with the page open it also loads NVML for an NVIDIA card's watts and reads a power supply that reports over USB), psu (that power supply over HID, read with requests only), frames (Show over games' frame count: a real-time event tracing session of the game's Present calls, only while the readout shows; the one-time OK is `poni.exe --allow-fps <SID>`, run elevated) |
| `dragdrop.rs`, `picker.rs`, `shell.rs`, `tray.rs`, `autostart.rs`, `activate.rs` | OS integration: drops onto the island, the Open dialog, opening files and apps (validated local paths only), the tray menu, Start with Windows, and bringing the playing app forward (the Music page's player name: a Store app through `IApplicationActivationManager`, a desktop app's main window, preferring the one titled with the song) |
| `version.rs` | which build this is, for the tray's greyed About line |
| `selftest.rs` | `poni.exe --selftest <dir>` scripted screenshots and checks |

## Data

* Settings: `%APPDATA%\Poni\settings.json` (created with defaults; tray → "Open settings file"), including the agenda's calendar link.
* Data (tasks, notes, focus stats, the Script reader's script, the Dock's apps): `%APPDATA%\Poni\data.json`.
  The agenda's meetings are only held in memory.
  Drinks logged by an old Water page stay in it untouched (`water`).
* Log: `%LOCALAPPDATA%\Poni\poni.log`.
* `PONI_HOME` overrides both folders (the self-test uses it).
* Clipboard history is memory-only and skips content marked
  `ExcludeClipboardContentFromMonitorProcessing` / `CanIncludeInClipboardHistory=0`
  (password managers set these).

## Building

```
cargo test --workspace           # logic and app tests
cargo build -p poni --release    # on Windows: target\release\poni.exe
cargo wcheck                     # type-check the app from Linux
cargo wbuild                     # cross-build target/x86_64-pc-windows-gnu/release/poni.exe (needs mingw-w64)
python3 tools/make_notices.py    # rewrite THIRD-PARTY-NOTICES.txt after changing dependencies (--check compares)
python3 tools/make_brand.py      # redraw the icon and the installer pictures (needs Pillow)
```

Only one copy runs at a time (`Local\Poni.Island`): close the running one
(tray → Quit) before replacing `poni.exe`. The installers are
`installer/poni.nsi` (NSIS) and `installer/poni.iss` (Inno Setup).
