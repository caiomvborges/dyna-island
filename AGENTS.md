# Notes for coding agents

Codex, Claude Code, Kimi or any other agent working in this repo: read this before you change the app. This is Dyna Island, a local, Windows-only fork of Oyenza Studio (MIT): its one network connection is the Outlook agenda add-on reading the calendar link the person pasted, and nothing here is published.

## Keep it light

Dyna Island sits on screen all day, so how little it costs is part of the product. The maintainer's rule (24 Oct 2026): keep it very light on CPU, memory and download size, and fast, however many features it gets. Every change follows these:

- **Nothing runs at rest unless the person switched it on or its page is open.** No new thread, timer, poll or network call that keeps going while the island is idle. React to Windows' own notifications where there are some. When something has to repeat, give the interval a reason in a comment and stop it as soon as nothing needs it.
- **Anything that repeats rests when nobody is there:** while the session is locked, the display is off or battery saver is on. The app already knows all three (`App::locked`, `App::display_off`, `st.power.saver`), and ambient animation already pauses for the first two.
- **Motion costs a frame every refresh.** Something that moves by itself on the closed island (the equalizer, the eyes' blink) redraws for as long as it shows. Add one only where people watch it, and let it stop.
- **Load heavy Windows parts when the feature first needs them, not at start.** Performance counters, graphics devices and WinRT classes bring their own libraries: the System page's counters alone add about 6 MB once loaded.
- **Nothing else talks to the network.** The only connection is `services::agenda`: a GET of the ICS link the person pasted, held by `dyna_core::ics::check_link` to `https` on Outlook's own hosts, with redirects off, only while the agenda add-on is on, and never logged beyond its host. Add-ons are built in (`dyna_core::addons`) and nothing downloads. Don't add another connection. `include_bytes!` is for small things the app can't start without, such as the icon and the Outfit font.
- **A new crate needs a reason and its size.** Build with `CARGO_PROFILE_RELEASE_STRIP=false cargo wbuild` before and after, and compare `x86_64-w64-mingw32-nm --size-sort -S -C` totals per crate. Code that runs for moments (a handshake, a parser, a file format) can be built for size in the workspace `Cargo.toml` (`[profile.release.package.<name>] opt-level = "s"`). Code that runs every frame stays at full speed.
- **Measure CPU in cycles, not milliseconds.** `TotalProcessorTime`, `GetProcessTimes` and Task Manager count in 15.6 ms clock ticks, so a reading that takes a fraction of a millisecond shows as zero. Use `QueryProcessCycleTime` or `QueryThreadCycleTime`.
- **Every change says what it costs:** download size, memory at rest (`smoke-out/metrics.txt` from the smoke run), and anything new that runs at rest or on every frame. "Nothing new" is a fine answer.

## Names

Everything is called Dyna Island: the window title, `dyna-island.exe`, the crates (`dyna-island`, `dyna-core`), the folders and Run value (`DynaIsland`), `Local\DynaIsland.Island` and the `DYNA_*` variables. The Oyenza Studio brand (name, logo, the Oyen cat) is not used anywhere. The original's inner name, Poni, survives only where older installs are read: `paths::migrate_from_poni` moves `%APPDATA%\Poni` and `%LOCALAPPDATA%\Poni` over once, `autostart::migrate_from_poni` swaps the old `Poni` Run value, and the app won't start next to a running `Local\Poni.Island`. The Light theme's settings value also reads the old `"oyen"`.
