//! The camera and microphone in use (`AppEvent::InUse`), from Windows' own
//! privacy records: `CapabilityAccessManager\ConsentStore\webcam` and
//! `\microphone`, for this user and for the machine. Windows writes when an
//! app starts and stops using a device; an app whose stop time is 0 is using
//! it now.
//!
//! One thread blocked in `WaitForMultipleObjects` on registry change events
//! and a stop event: no polling, no CPU while nothing changes. Dropping the
//! [`InUseWatch`] sets the event and the thread ends.

use crate::events::{AppEvent, EventSink};
use dyna_core::inuse::{app_name, InUse};
use windows::core::{w, PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::System::Registry::*;
use windows::Win32::System::Threading::{CreateEventW, SetEvent, WaitForMultipleObjects, INFINITE};

const CAMERA: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\CapabilityAccessManager\\ConsentStore\\webcam");
const MIC: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\CapabilityAccessManager\\ConsentStore\\microphone");
/// Windows writes a start or stop as a few values in a row: read once they settle.
const SETTLE_MS: u32 = 150;

/// A handle that can cross to the watching thread.
struct SendHandle(HANDLE);
unsafe impl Send for SendHandle {}

pub struct InUseWatch {
    stop: HANDLE,
}

// The stop event is only signalled and closed; both are thread-safe.
unsafe impl Send for InUseWatch {}

impl InUseWatch {
    pub fn start(sink: EventSink) -> windows::core::Result<Self> {
        let stop = unsafe { CreateEventW(None, true, false, None)? };
        let theirs = SendHandle(stop);
        std::thread::Builder::new()
            .name("dyna-inuse".into())
            .spawn(move || watch(theirs, sink))
            .map_err(|e| windows::core::Error::new(windows::core::HRESULT(-1), e.to_string()))?;
        Ok(Self { stop })
    }
}

impl Drop for InUseWatch {
    fn drop(&mut self) {
        // The thread wakes, closes its keys and ends; the event stays open
        // until then because the thread owns no copy: leak it rather than
        // close it under a waiting thread (one handle, at quit).
        unsafe {
            let _ = SetEvent(self.stop);
        }
    }
}

/// One watched key: its handle and the event Windows signals when anything under it changes.
struct Watched {
    key: HKEY,
    event: HANDLE,
    camera: bool,
}

fn watch(stop: SendHandle, sink: EventSink) {
    let mut keys: Vec<Watched> = Vec::new();
    for (root, path, camera) in [(HKEY_CURRENT_USER, CAMERA, true), (HKEY_CURRENT_USER, MIC, false), (HKEY_LOCAL_MACHINE, CAMERA, true), (HKEY_LOCAL_MACHINE, MIC, false)] {
        let mut key = HKEY::default();
        if unsafe { RegOpenKeyExW(root, path, 0, KEY_READ | KEY_NOTIFY, &mut key) }.is_err() {
            continue;
        }
        match unsafe { CreateEventW(None, false, false, None) } {
            Ok(event) => keys.push(Watched { key, event, camera }),
            Err(_) => unsafe {
                let _ = RegCloseKey(key);
            },
        }
    }
    if keys.is_empty() {
        crate::pwarn!("inuse: Windows' privacy records can't be read; camera and mic won't show");
        return;
    }
    let mut last = InUse::default();
    let arm_and_read = |keys: &[Watched], last: &mut InUse| {
        for k in keys {
            arm(k);
        }
        let now = read(keys);
        if now != *last {
            *last = now.clone();
            sink.send(AppEvent::InUse(now));
        }
    };
    arm_and_read(&keys, &mut last);
    let mut handles = vec![stop.0];
    handles.extend(keys.iter().map(|k| k.event));
    loop {
        let which = unsafe { WaitForMultipleObjects(&handles, false, INFINITE) };
        if which == WAIT_OBJECT_0 || which.0 >= WAIT_OBJECT_0.0 + handles.len() as u32 {
            break;
        }
        // Let the rest of the writes land, then read everything again.
        let settled = unsafe { WaitForMultipleObjects(&[stop.0], false, SETTLE_MS) };
        if settled == WAIT_OBJECT_0 {
            break;
        }
        arm_and_read(&keys, &mut last);
    }
    for k in keys {
        unsafe {
            let _ = RegCloseKey(k.key);
            let _ = CloseHandle(k.event);
        }
    }
}

/// Ask Windows to signal `k.event` on the next change under the key (once:
/// it is asked again after each change).
fn arm(k: &Watched) {
    let filter = REG_NOTIFY_CHANGE_NAME | REG_NOTIFY_CHANGE_LAST_SET;
    let r = unsafe { RegNotifyChangeKeyValue(k.key, true, filter, k.event, true) };
    if r.is_err() {
        crate::pwarn!("inuse: watching a privacy record failed ({r:?})");
    }
}

/// Who uses the camera and the microphone now.
fn read(keys: &[Watched]) -> InUse {
    let mut now = InUse::default();
    for k in keys {
        let list = if k.camera { &mut now.camera } else { &mut now.mic };
        for (name, sub) in subkeys(k.key) {
            if name.eq_ignore_ascii_case("NonPackaged") {
                // Desktop apps, one key each under this one.
                for (app, appkey) in subkeys(sub) {
                    if in_use(appkey) {
                        list.push(app_name(&app));
                    }
                    unsafe {
                        let _ = RegCloseKey(appkey);
                    }
                }
            } else if in_use(sub) {
                list.push(app_name(&name));
            }
            unsafe {
                let _ = RegCloseKey(sub);
            }
        }
    }
    now.tidy()
}

/// The subkeys of `key`, by name, each open for reading (the caller closes them).
fn subkeys(key: HKEY) -> Vec<(String, HKEY)> {
    let mut out = Vec::new();
    let mut buf = [0u16; 512];
    for i in 0.. {
        let mut len = buf.len() as u32;
        let r = unsafe { RegEnumKeyExW(key, i, PWSTR(buf.as_mut_ptr()), &mut len, None, PWSTR::null(), None, None) };
        if r.is_err() {
            break;
        }
        let name = String::from_utf16_lossy(&buf[..len as usize]);
        let mut sub = HKEY::default();
        if unsafe { RegOpenKeyExW(key, PCWSTR(buf.as_ptr()), 0, KEY_READ, &mut sub) }.is_ok() {
            out.push((name, sub));
        }
    }
    out
}

/// The app this key records started using the device and hasn't stopped.
fn in_use(key: HKEY) -> bool {
    let qword = |name: PCWSTR| {
        let mut v = 0u64;
        let mut size = std::mem::size_of::<u64>() as u32;
        let r = unsafe { RegGetValueW(key, None, name, RRF_RT_REG_QWORD, None, Some(&mut v as *mut u64 as *mut _), Some(&mut size)) };
        r.is_ok().then_some(v)
    };
    matches!((qword(w!("LastUsedTimeStart")), qword(w!("LastUsedTimeStop"))), (Some(start), Some(0)) if start != 0)
}
