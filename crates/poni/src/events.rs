//! Messages from service threads to the UI thread.
//!
//! Services hold an [`EventSink`]. `send` pushes onto an mpsc channel and posts
//! `WM_APP_EVENT` to the island window; the UI thread drains the channel.

use crate::gfx::Image;
use crate::state::{MediaSession, PowerState, SysStats};
use poni_core::timeline::Timeline;
use std::sync::mpsc::Sender;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};

pub const WM_APP_EVENT: u32 = WM_APP + 1;
pub const WM_APP_FRAME: u32 = WM_APP + 2;
pub const WM_APP_TRAY: u32 = WM_APP + 3;
pub const WM_APP_APPBAR: u32 = WM_APP + 4;

/// An HWND that can cross threads (only used for PostMessage).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SendHwnd(pub isize);
unsafe impl Send for SendHwnd {}
unsafe impl Sync for SendHwnd {}

impl SendHwnd {
    pub fn new(h: HWND) -> Self {
        Self(h.0 as isize)
    }
    pub fn hwnd(self) -> HWND {
        HWND(self.0 as *mut core::ffi::c_void)
    }
}

#[derive(Clone)]
pub struct EventSink {
    tx: Sender<AppEvent>,
    hwnd: SendHwnd,
}

impl EventSink {
    pub fn new(tx: Sender<AppEvent>, hwnd: SendHwnd) -> Self {
        Self { tx, hwnd }
    }

    pub fn send(&self, ev: AppEvent) {
        if self.tx.send(ev).is_ok() {
            unsafe {
                let _ = PostMessageW(self.hwnd.hwnd(), WM_APP_EVENT, WPARAM(0), LPARAM(0));
            }
        }
    }

    pub fn hwnd(&self) -> SendHwnd {
        self.hwnd
    }
}

pub enum AppEvent {
    /// The current media session changed (None = nothing playing / no session).
    Media(Option<MediaSession>),
    /// Only the timeline/playback state of the current track changed.
    MediaTimeline { track_key: String, timeline: Timeline },
    /// Album art finished decoding for a track.
    MediaArt { track_key: String, art: Option<Image> },
    /// Master volume changed (0..=1).
    Volume { level: f32, muted: bool },
    Power(PowerState),
    Bluetooth { name: String, connected: bool, battery: Option<u8>, audio: bool },
    /// A full-screen app took over (or left) the island's monitor. `true` is
    /// sent again when another window comes to the foreground while the
    /// monitor stays full-screen, so the island can be raised back on top.
    Fullscreen(bool),
    /// Another window came to the foreground on a monitor that isn't
    /// full-screen. It may have made itself topmost and now cover the island.
    Foreground,
    /// Step aside: the app windows at the top edge of the island's monitor
    /// changed (a browser maximized or restored, a window snapped or dragged
    /// to the top). Screen pixels (left, top, right, bottom), each clipped
    /// to the thin band along the edge.
    TopWindows(Vec<[i32; 4]>),
    /// Files dropped on the island.
    FilesDropped(Vec<String>),
    /// Files are being dragged over the island (true) or left (false).
    DragHover(bool),
    /// An app icon the System page asked for (`services::imaging`).
    Thumb { path: String, image: Option<Image> },
    Stats(SysStats),
    /// The System page's latest reading and the minute behind it
    /// (`services::sysmon`, each second while the page is open).
    System(Box<poni_core::sysmon::Monitor>),
    /// Show over games: whether Windows lets Poni read frames (a check, or
    /// the one-time OK's outcome).
    FpsAccess(crate::state::FpsAccess),
    ClipboardText(String),
    /// What the Outlook agenda add-on just read (`services::agenda`).
    Agenda(crate::services::agenda::AgendaRead),
    /// A process the person asked about ended (`services::watch`), with its
    /// exit code when Windows tells it.
    ProcessExited { pid: u32, code: Option<u32> },
    /// Start with Windows is on (true) or off, read for the Settings page.
    Autostart(bool),
    /// Free-form log line from a service thread.
    Log(String),
    /// The Glass theme's screen copy has new pixels (or stopped).
    Backdrop,
}
