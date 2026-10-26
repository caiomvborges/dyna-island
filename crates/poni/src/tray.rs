//! Notification-area icon and its menu (also shown on right-click of the island).

use crate::events::WM_APP_TRAY;
use poni_core::settings::{IdleStyle, Settings, Theme};
use windows::core::{Result, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW};
use windows::Win32::UI::WindowsAndMessaging::*;

pub struct Tray {
    hwnd: HWND,
}

impl Tray {
    pub fn add(hwnd: HWND) -> Result<Tray> {
        unsafe {
            let hinst = GetModuleHandleW(None)?;
            // Ask for the small-icon size so Windows picks poni.ico's own 16-24 px
            // frames; LoadIconW gives the 32 px one and shrinks it (blurry).
            let (cx, cy) = (GetSystemMetrics(SM_CXSMICON), GetSystemMetrics(SM_CYSMICON));
            let icon = LoadImageW(hinst, PCWSTR(1 as _), IMAGE_ICON, cx, cy, LR_DEFAULTCOLOR)
                .map(|h| HICON(h.0))
                .or_else(|_| LoadIconW(hinst, PCWSTR(1 as _)))
                .or_else(|_| LoadIconW(None, IDI_APPLICATION))?;
            let mut nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: hwnd,
                uID: 1,
                uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
                uCallbackMessage: WM_APP_TRAY,
                hIcon: icon,
                ..Default::default()
            };
            let tip: Vec<u16> = format!("Dyna Island {}", crate::version::VERSION).encode_utf16().collect();
            nid.szTip[..tip.len()].copy_from_slice(&tip);
            if !Shell_NotifyIconW(NIM_ADD, &nid).as_bool() {
                return Err(windows::core::Error::from_win32());
            }
        }
        Ok(Tray { hwnd })
    }

    pub fn remove(self) {
        unsafe {
            let nid = NOTIFYICONDATAW { cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32, hWnd: self.hwnd, uID: 1, ..Default::default() };
            let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuCmd {
    ToggleIdleStyle(IdleStyle),
    SetTheme(Theme),
    ToggleClipboard,
    ToggleSystemHistory,
    /// true: hide in full-screen apps; false: stay on top of them.
    SetHideFullscreen(bool),
    /// true: step aside while a maximized window is under the island;
    /// false: stay over it.
    SetStepAside(bool),
    /// Show over games (frame rate, GPU and CPU on the island over a game).
    ToggleGameStats,
    /// Show over games for a few seconds at a time instead of all along.
    ToggleGamePeek,
    ToggleSounds,
    TogglePinned,
    ToggleAutostart,
    OpenSettings,
    ReloadSettings,
    OpenDataFolder,
    Quit,
}

pub enum MenuItem {
    Item { label: String, cmd: MenuCmd, checked: bool },
    /// A greyed line that only says something ("About Dyna Island 0.2.3").
    Info { label: String },
    Sub { label: String, items: Vec<MenuItem> },
    Separator,
}

fn item(label: &str, cmd: MenuCmd, checked: bool) -> MenuItem {
    MenuItem::Item { label: label.to_string(), cmd, checked }
}

/// An idle look as the menu names it.
fn idle_label(s: IdleStyle) -> &'static str {
    match s {
        IdleStyle::Clock => "Clock",
        IdleStyle::Eyes => "Eyes",
        IdleStyle::Minimal => "Minimal",
        IdleStyle::Hidden => "Hidden until hover",
        IdleStyle::Status => "Status bar",
    }
}

pub fn menu_items(s: &Settings, pinned: bool) -> Vec<MenuItem> {
    // The look the theme draws is the one checked: outside Pill, Status shows the clock.
    let shown = crate::pill::status::shown(s.theme, s.idle_style);
    let style = |st: IdleStyle| item(idle_label(st), MenuCmd::ToggleIdleStyle(st), shown == st);
    let theme = |label: &str, t: Theme| item(label, MenuCmd::SetTheme(t), s.theme == t);
    let fullscreen = |label: &str, hide: bool| item(label, MenuCmd::SetHideFullscreen(hide), s.hide_in_fullscreen == hide);
    let aside = |label: &str, on: bool| item(label, MenuCmd::SetStepAside(on), s.step_aside == on);
    vec![
        item("Keep island open", MenuCmd::TogglePinned, pinned),
        MenuItem::Sub { label: "Theme".into(), items: vec![theme("Classic", Theme::Classic), theme("Light", Theme::Light), theme("Glass", Theme::Glass), theme("Pill", Theme::Pill)] },
        // The status bar is listed only in Pill, as on the Settings page: the
        // other themes would draw the clock for it.
        MenuItem::Sub { label: "Idle look".into(), items: crate::pill::status::looks(s.theme).iter().map(|l| style(*l)).collect() },
        MenuItem::Separator,
        item("Clipboard history", MenuCmd::ToggleClipboard, s.clipboard_history),
        item("Keep System page history", MenuCmd::ToggleSystemHistory, s.system_history),
        item("Sounds", MenuCmd::ToggleSounds, s.sounds),
        MenuItem::Sub {
            label: "In full-screen apps".into(),
            items: vec![
                fullscreen("Hide", true),
                fullscreen("Stay on top", false),
                MenuItem::Separator,
                item("Show FPS, GPU and CPU over games", MenuCmd::ToggleGameStats, s.game_stats),
                item("Only for a few seconds (no added delay)", MenuCmd::ToggleGamePeek, s.game_peek),
            ],
        },
        // A browser's tabs under the island: out of their way until pointed at.
        MenuItem::Sub { label: "Over maximized windows".into(), items: vec![aside("Step aside", true), aside("Stay", false)] },
        item("Start with Windows", MenuCmd::ToggleAutostart, crate::autostart::is_enabled()),
        MenuItem::Separator,
        item("Open settings file", MenuCmd::OpenSettings, false),
        item("Reload settings", MenuCmd::ReloadSettings, false),
        item("Open data folder", MenuCmd::OpenDataFolder, false),
        // Which version and build this is (a greyed line: nothing to open).
        MenuItem::Info { label: crate::version::about_label() },
        MenuItem::Separator,
        item("Quit Dyna Island", MenuCmd::Quit, false),
    ]
}

fn build(menu: HMENU, items: &[MenuItem], cmds: &mut Vec<MenuCmd>) {
    for it in items {
        unsafe {
            match it {
                MenuItem::Item { label, cmd, checked } => {
                    cmds.push(*cmd);
                    let id = cmds.len();
                    let w: Vec<u16> = label.encode_utf16().chain(Some(0)).collect();
                    let flags = MF_STRING | if *checked { MF_CHECKED } else { MF_UNCHECKED };
                    let _ = AppendMenuW(menu, flags, id, PCWSTR(w.as_ptr()));
                }
                MenuItem::Info { label } => {
                    let w: Vec<u16> = label.encode_utf16().chain(Some(0)).collect();
                    let _ = AppendMenuW(menu, MF_STRING | MF_GRAYED | MF_DISABLED, 0, PCWSTR(w.as_ptr()));
                }
                MenuItem::Sub { label, items } => {
                    if let Ok(sub) = CreatePopupMenu() {
                        build(sub, items, cmds);
                        let w: Vec<u16> = label.encode_utf16().chain(Some(0)).collect();
                        let _ = AppendMenuW(menu, MF_POPUP, sub.0 as usize, PCWSTR(w.as_ptr()));
                    }
                }
                MenuItem::Separator => {
                    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
                }
            }
        }
    }
}

/// Show the menu at `pt` (screen pixels); returns the chosen command.
pub fn show_menu(owner: HWND, pt: POINT, items: &[MenuItem]) -> Option<MenuCmd> {
    unsafe {
        let menu = CreatePopupMenu().ok()?;
        let mut cmds = Vec::new();
        build(menu, items, &mut cmds);
        // Required so the menu closes when clicking elsewhere; afterwards the
        // previous app gets its focus back.
        let prev = GetForegroundWindow();
        let _ = SetForegroundWindow(owner);
        let chosen = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY, pt.x, pt.y, 0, owner, None);
        let _ = PostMessageW(owner, WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);
        let idx = chosen.0 as usize;
        let cmd = if idx == 0 { None } else { cmds.get(idx - 1).copied() };
        let launches = matches!(
            cmd,
            Some(MenuCmd::OpenSettings | MenuCmd::OpenDataFolder)
        );
        if !launches && !prev.is_invalid() && prev != owner && IsWindow(prev).as_bool() && GetForegroundWindow() == owner {
            let _ = SetForegroundWindow(prev);
        }
        cmd
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fullscreen_choices(s: &Settings) -> Vec<(String, MenuCmd, bool)> {
        let items = menu_items(s, false);
        let Some(MenuItem::Sub { items, .. }) = items.into_iter().find(|i| matches!(i, MenuItem::Sub { label, .. } if label == "In full-screen apps")) else {
            panic!("no full-screen submenu");
        };
        items
            .into_iter()
            .filter_map(|i| match i {
                MenuItem::Item { label, cmd, checked } => Some((label, cmd, checked)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn the_menu_says_which_version_this_is() {
        let items = menu_items(&Settings::default(), false);
        let about = items.iter().position(|i| matches!(i, MenuItem::Info { .. })).expect("no About line");
        let MenuItem::Info { label } = &items[about] else { unreachable!() };
        assert!(label.starts_with(&format!("About Dyna Island {}", env!("CARGO_PKG_VERSION"))), "{label}");
        // Right above the separator before Quit, with Open data folder kept above it.
        assert!(matches!(&items[about - 1], MenuItem::Item { cmd: MenuCmd::OpenDataFolder, .. }));
        assert!(matches!(&items[about + 1], MenuItem::Separator));
        assert!(matches!(&items[about + 2], MenuItem::Item { cmd: MenuCmd::Quit, .. }));
    }

    fn idle_choices(s: &Settings) -> Vec<(String, bool)> {
        let items = menu_items(s, false);
        let Some(MenuItem::Sub { items, .. }) = items.into_iter().find(|i| matches!(i, MenuItem::Sub { label, .. } if label == "Idle look")) else {
            panic!("no idle look submenu");
        };
        items
            .into_iter()
            .filter_map(|i| match i {
                MenuItem::Item { label, checked, .. } => Some((label, checked)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn status_bar_is_listed_only_in_the_pill_theme() {
        let pill = Settings { theme: Theme::Pill, idle_style: IdleStyle::Status, ..Settings::default() };
        let labels: Vec<String> = idle_choices(&pill).into_iter().map(|c| c.0).collect();
        assert_eq!(labels, ["Clock", "Eyes", "Minimal", "Hidden until hover", "Status bar"]);
        assert_eq!(idle_choices(&pill).last(), Some(&("Status bar".to_string(), true)));
        // Another theme draws the clock for the status bar: Clock is checked,
        // and Status comes back with Pill.
        for theme in [Theme::Classic, Theme::Light, Theme::Glass] {
            let s = Settings { theme, ..pill.clone() };
            let choices = idle_choices(&s);
            assert_eq!(choices.len(), 4, "{theme:?}");
            assert_eq!(choices[0], ("Clock".to_string(), true), "{theme:?}");
            assert!(choices.iter().skip(1).all(|c| !c.1), "{theme:?}");
        }
    }

    #[test]
    fn maximized_windows_submenu_follows_the_full_screen_one_and_checks_the_choice() {
        let mut s = Settings::default();
        for on in [true, false] {
            s.step_aside = on;
            let items = menu_items(&s, false);
            let at = |name: &str| items.iter().position(|i| matches!(i, MenuItem::Sub { label, .. } if label == name));
            let (Some(fs), Some(i)) = (at("In full-screen apps"), at("Over maximized windows")) else {
                panic!("no maximized windows submenu");
            };
            assert_eq!(i, fs + 1, "right after In full-screen apps");
            let MenuItem::Sub { items, .. } = &items[i] else { unreachable!() };
            let choices: Vec<(&str, MenuCmd, bool)> = items
                .iter()
                .filter_map(|i| match i {
                    MenuItem::Item { label, cmd, checked } => Some((label.as_str(), *cmd, *checked)),
                    _ => None,
                })
                .collect();
            assert_eq!(choices, [("Step aside", MenuCmd::SetStepAside(true), on), ("Stay", MenuCmd::SetStepAside(false), !on)]);
        }
    }

    #[test]
    fn full_screen_submenu_checks_the_current_choice() {
        let mut s = Settings::default();
        for hide in [true, false] {
            s.hide_in_fullscreen = hide;
            let choices = fullscreen_choices(&s);
            assert_eq!(
                choices,
                vec![
                    ("Hide".to_string(), MenuCmd::SetHideFullscreen(true), hide),
                    ("Stay on top".to_string(), MenuCmd::SetHideFullscreen(false), !hide),
                    ("Show FPS, GPU and CPU over games".to_string(), MenuCmd::ToggleGameStats, false),
                    ("Only for a few seconds (no added delay)".to_string(), MenuCmd::ToggleGamePeek, false),
                ]
            );
        }
    }
}
