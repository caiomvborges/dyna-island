//! "Go to the player": bring forward the app the Music page shows.
//!
//! Windows' media controls name the player by its app id: a Store app's
//! (`SpotifyAB.SpotifyMusic_…!Spotify`), which Windows itself activates, or a
//! desktop app's executable ("Spotify.exe", "Chrome", "MSEdge"). For those
//! the island looks for that program's main window, preferring one whose
//! title names the song (the browser tab that plays it), restores it if it is
//! minimized and brings it to the front. A player hidden in the tray with no
//! window is started again from its own file, which single-instance players
//! answer by showing themselves (browsers are left alone: that would open a
//! new window).
//!
//! It runs on the UI thread right after the click, so Windows lets the island
//! hand over the foreground. Nothing here runs at rest.

use windows::core::{HSTRING, PWSTR};
use windows::Win32::Foundation::{CloseHandle, BOOL, HWND, LPARAM};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_LOCAL_SERVER};
use windows::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS};
use windows::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION};
use windows::Win32::UI::Shell::{ApplicationActivationManager, IApplicationActivationManager, AO_NONE};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumWindows, GetWindow, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindowVisible,
    SetForegroundWindow, ShowWindow, GW_OWNER, SW_RESTORE,
};

/// Browsers: their window is looked for, never started again.
const BROWSERS: [&str; 7] = ["chrome", "msedge", "firefox", "brave", "opera", "vivaldi", "arc"];

/// How it went, for the log and a notice when nothing came forward.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Its window came to the front.
    Shown,
    /// Windows activated the Store app, or the player was started again.
    Asked,
    NotFound,
}

/// Bring forward the player `app_id`, preferring the window titled with `song`.
pub fn bring_forward(app_id: &str, song: &str) -> Outcome {
    // A Store app: Windows activates it (and brings its window up).
    if app_id.contains('!') {
        let ok = unsafe {
            CoCreateInstance::<_, IApplicationActivationManager>(&ApplicationActivationManager, None, CLSCTX_LOCAL_SERVER)
                .and_then(|m| m.ActivateApplication(&HSTRING::from(app_id), &HSTRING::new(), AO_NONE))
        };
        return match ok {
            Ok(_) => Outcome::Asked,
            Err(e) => {
                crate::pwarn!("player: activating {app_id} failed ({e})");
                Outcome::NotFound
            }
        };
    }
    let stem = stem(app_id);
    if let Some(w) = best_window(&stem, song) {
        unsafe {
            if IsIconic(w).as_bool() {
                let _ = ShowWindow(w, SW_RESTORE);
            }
            let _ = SetForegroundWindow(w);
            let _ = BringWindowToTop(w);
        }
        return Outcome::Shown;
    }
    if !BROWSERS.contains(&stem.as_str()) {
        if let Some(path) = running_exe(&stem) {
            crate::shell::open(&path);
            return Outcome::Asked;
        }
    }
    Outcome::NotFound
}

/// "Spotify.exe" → "spotify", "MSEdge" → "msedge".
fn stem(app_id: &str) -> String {
    let file = app_id.rsplit(['\\', '/']).next().unwrap_or(app_id).to_ascii_lowercase();
    file.strip_suffix(".exe").map(str::to_string).unwrap_or(file)
}

struct Search<'a> {
    stem: &'a str,
    song: String,
    best: Option<(i32, HWND)>,
}

/// The player's main window: a visible, unowned, titled window of that
/// program (+2), all the better when its title names the song (+3). A window
/// of another program that names the song counts too (a browser whose app id
/// isn't its file name).
fn best_window(stem: &str, song: &str) -> Option<HWND> {
    let mut s = Search { stem, song: song.trim().to_lowercase(), best: None };
    unsafe {
        let _ = EnumWindows(Some(each), LPARAM(&mut s as *mut Search as isize));
    }
    s.best.map(|(_, w)| w)
}

unsafe extern "system" fn each(w: HWND, lp: LPARAM) -> BOOL {
    let s = &mut *(lp.0 as *mut Search);
    if !IsWindowVisible(w).as_bool() || GetWindow(w, GW_OWNER).is_ok_and(|o| !o.is_invalid()) || cloaked(w) {
        return true.into();
    }
    let len = GetWindowTextLengthW(w);
    if len <= 0 {
        return true.into();
    }
    let mut buf = vec![0u16; len as usize + 1];
    let n = GetWindowTextW(w, &mut buf) as usize;
    let title = String::from_utf16_lossy(&buf[..n]).to_lowercase();
    let mut pid = 0u32;
    GetWindowThreadProcessId(w, Some(&mut pid));
    let ours = image_path(pid).is_some_and(|p| stem(&p) == s.stem);
    let names_song = s.song.len() >= 3 && title.contains(&s.song);
    let score = if ours { 2 } else { 0 } + if names_song { 3 } else { 0 };
    if score >= 2 && s.best.is_none_or(|(b, _)| score > b) {
        s.best = Some((score, w));
    }
    true.into()
}

/// Windows hides some windows by cloaking them (other desktops, suspended Store apps).
unsafe fn cloaked(w: HWND) -> bool {
    let mut v = 0u32;
    DwmGetWindowAttribute(w, DWMWA_CLOAKED, &mut v as *mut u32 as *mut _, 4).is_ok() && v != 0
}

fn image_path(pid: u32) -> Option<String> {
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len).is_ok();
        let _ = CloseHandle(h);
        ok.then(|| String::from_utf16_lossy(&buf[..len as usize]))
    }
}

/// The file of a running process of program `stem` (for a player with no window).
fn running_exe(stem_: &str) -> Option<String> {
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).ok()?;
        let mut e = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut found = None;
        let mut more = Process32FirstW(snap, &mut e).is_ok();
        while more {
            let n = e.szExeFile.iter().position(|c| *c == 0).unwrap_or(e.szExeFile.len());
            if stem(&String::from_utf16_lossy(&e.szExeFile[..n])) == stem_ {
                found = image_path(e.th32ProcessID);
                if found.is_some() {
                    break;
                }
            }
            more = Process32NextW(snap, &mut e).is_ok();
        }
        let _ = CloseHandle(snap);
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_ids_become_program_names() {
        assert_eq!(stem("Spotify.exe"), "spotify");
        assert_eq!(stem("MSEdge"), "msedge");
        assert_eq!(stem(r"C:\Program Files\VLC\vlc.exe"), "vlc");
    }
}
