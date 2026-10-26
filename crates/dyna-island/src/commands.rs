//! Side effects requested by pages; executed by the app after each frame.

use crate::pages::PageKind;

#[derive(Clone, Debug, PartialEq)]
pub enum MediaCmd {
    PlayPause,
    Next,
    Prev,
    /// Seek to absolute position in seconds.
    Seek(f64),
    /// The pointer reached the progress bar or the back button: get ready
    /// (Apple Music's own controls are looked up before the press, not during it).
    Prepare,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sound {
    /// Gentle chime (timer finished, reminder).
    Chime,
    /// Short tick (button feedback) — currently unused.
    Tick,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    Media(MediaCmd),
    /// Put text on the clipboard (not re-recorded into history).
    CopyText(String),
    /// Give the island keyboard focus for a text field.
    FocusText,
    /// The Script page's Paste: the clipboard's text, line breaks kept,
    /// becomes the script.
    ScriptPaste,
    /// Persist `AppState::store` soon.
    SaveStore,
    /// Persist `AppState::settings` now (after a page toggled a setting).
    SaveSettings,
    SetPage(PageKind),
    /// Keep the island open even when the cursor leaves.
    SetPinned(bool),
    Collapse,
    Play(Sound),
    /// Start/stop the 1 Hz system-stats sampler.
    WantStats(bool),
    /// Start with Windows on or off (the Settings page's switch).
    SetAutostart(bool),
    /// Open the System page on this module (a Today ring was clicked).
    OpenSystem(dyna_core::sysmon::Module),
    /// Request an app's icon for the System page (arrives in `AppState::app_icons`).
    LoadAppIcon(String),
    /// Tell me when this process ends (the System page's bell), or stop.
    WatchProcess { pid: u32, name: String },
    UnwatchProcess(u32),
    /// The Agenda page's Paste link: the clipboard's ICS link becomes the calendar.
    AgendaPasteLink,
    /// The Agenda page's Remove link: forget the calendar and its meetings.
    AgendaRemoveLink,
    /// Bring forward the app that plays (the Music page's player name).
    OpenPlayer,
    /// Open an app from the Dock (a program, a shortcut, …).
    LaunchApp(String),
    /// The Dock's Add: Windows' Open dialog, then the pick goes on the Dock.
    PickDockApp,
    /// Request a Dock app's icon (arrives in `AppState::dock_icons`).
    LoadDockIcon(String),
    /// Show over games: find out whether Windows lets Dyna Island read frames
    /// (the answer lands in `AppState::game.access`).
    CheckFps,
    /// Show over games: ask Windows once for the right to read frames.
    AllowFps,
}
