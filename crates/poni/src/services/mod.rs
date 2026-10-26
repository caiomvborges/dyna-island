//! Background services. Each one reports to the UI thread through an
//! [`EventSink`]; a failing service is logged and skipped, never fatal.

pub mod applemusic_ui;
pub mod agenda;
pub mod audio;
pub mod backdrop;
pub mod bluetooth;
pub mod clipboard;
pub mod frames;
pub mod fullscreen;
pub mod imaging;
pub mod media;
pub mod power;
pub mod psu;
pub mod sound;
pub mod sysmon;
pub mod sysstats;
pub mod watch;

use crate::commands::MediaCmd;
use crate::events::EventSink;
use poni_core::geom::Color;
use poni_core::settings::Settings;
use windows::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};

pub struct Services {
    media: Option<media::MediaService>,
    _audio: Option<audio::AudioService>,
    _bluetooth: Option<bluetooth::BluetoothService>,
    fullscreen: Option<fullscreen::FullscreenWatcher>,
    clip: Option<clipboard::ClipboardHistory>,
}

fn started<T, E: std::fmt::Display>(name: &str, r: Result<T, E>) -> Option<T> {
    match r {
        Ok(v) => {
            crate::pinfo!("service {name}: started");
            Some(v)
        }
        Err(e) => {
            crate::pwarn!("service {name}: unavailable ({e})");
            None
        }
    }
}

impl Services {
    pub fn start(sink: &EventSink, _island: HWND, helper: HWND, settings: &Settings) -> Self {
        let media = started("media", media::MediaService::start(sink.clone()));
        let audio = started("audio", audio::AudioService::start(sink.clone()));
        let bluetooth = started("bluetooth", bluetooth::BluetoothService::start(sink.clone()));
        let fullscreen = started("fullscreen", fullscreen::FullscreenWatcher::start(helper, sink.clone()));
        let clip = started("clipboard history", clipboard::ClipboardHistory::start(sink.clone(), settings.clipboard_history));
        Self { media, _audio: audio, _bluetooth: bluetooth, fullscreen, clip }
    }

    pub fn stop(self, _helper: HWND) {
        if let Some(c) = self.clip {
            c.stop();
        }
        if let Some(f) = self.fullscreen {
            f.stop();
        }
    }

    /// Turn clipboard history capture on or off (the setting changed).
    pub fn set_clipboard_history(&self, on: bool) {
        if let Some(c) = &self.clip {
            c.set_enabled(on);
        }
    }

    /// Step aside on (the band along the top edge of the island's monitor,
    /// screen pixels) or off: the full-screen watcher reports what reaches it.
    pub fn watch_top(&self, band: Option<RECT>) {
        if let Some(f) = &self.fullscreen {
            f.watch_top(band);
        }
    }

    /// Explorer restarted: AppBar registrations are lost and must be redone.
    pub fn on_taskbar_created(&self) {
        if let Some(f) = &self.fullscreen {
            f.on_taskbar_created();
        }
    }

    pub fn media_cmd(&self, c: MediaCmd) {
        if let Some(m) = &self.media {
            m.send(c);
        }
    }

    /// Handle the AppBar callback message; `Some(true)` when a full-screen
    /// app covers the island's monitor.
    pub fn on_appbar_message(&self, wp: WPARAM, lp: LPARAM, monitor: isize) -> Option<bool> {
        self.fullscreen.as_ref().and_then(|f| f.on_appbar_message(wp, lp, monitor))
    }
}

/// Windows apps are set to light mode (Settings › Personalization › Colors).
pub fn apps_use_light_theme() -> bool {
    use windows::UI::ViewManagement::{UIColorType, UISettings};
    // The system background is white in light mode and black in dark mode.
    UISettings::new().and_then(|s| s.GetColorValue(UIColorType::Background)).map(|c| c.R as u32 + c.G as u32 + c.B as u32 > 382).unwrap_or(true)
}

/// The Windows accent colour as it is: the Glass theme's text and fills sit
/// on light and dark glass alike, so it isn't lightened for black.
pub fn system_accent_base() -> Option<Color> {
    use windows::UI::ViewManagement::{UIColorType, UISettings};
    let s = UISettings::new().ok()?;
    let c = s.GetColorValue(UIColorType::Accent).ok()?;
    Some(Color::rgba(c.R as f32 / 255.0, c.G as f32 / 255.0, c.B as f32 / 255.0, 1.0))
}

/// The Windows accent colour, lightened for use on the black island.
pub fn system_accent() -> Option<Color> {
    use windows::UI::ViewManagement::{UIColorType, UISettings};
    let s = UISettings::new().ok()?;
    let c = s.GetColorValue(UIColorType::AccentLight2).ok()?;
    Some(Color::rgba(c.R as f32 / 255.0, c.G as f32 / 255.0, c.B as f32 / 255.0, 1.0).for_island_accent())
}
