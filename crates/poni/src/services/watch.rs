//! "Tell me when it's done": wait for a process the person picked on the
//! System page to end, and report it (`AppEvent::ProcessExited`) with its
//! exit code.
//!
//! Each watch is one thread blocked in `WaitForMultipleObjects` on the
//! process and a stop event: no polling, no CPU while it waits. Dropping the
//! [`Watch`] sets the event and the thread ends.

use crate::events::{AppEvent, EventSink};
use windows::Win32::Foundation::{CloseHandle, DuplicateHandle, DUPLICATE_SAME_ACCESS, HANDLE, WAIT_OBJECT_0};
use windows::Win32::System::Threading::{
    CreateEventW, GetCurrentProcess, GetExitCodeProcess, OpenProcess, SetEvent, WaitForMultipleObjects, INFINITE, PROCESS_QUERY_LIMITED_INFORMATION,
    PROCESS_SYNCHRONIZE,
};

/// A handle that can cross to the waiting thread (only that thread uses it
/// after the hand-over, and it closes it).
struct SendHandle(HANDLE);
unsafe impl Send for SendHandle {}

pub struct Watch {
    stop: HANDLE,
}

// The stop event is only signalled and closed; both are thread-safe.
unsafe impl Send for Watch {}

impl Watch {
    /// Start waiting for `pid`. Fails when the process is already gone or
    /// Windows won't let this account wait on it.
    pub fn start(pid: u32, sink: EventSink) -> windows::core::Result<Self> {
        let process = unsafe { OpenProcess(PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION, false, pid)? };
        let stop = match unsafe { CreateEventW(None, true, false, None) } {
            Ok(e) => e,
            Err(e) => {
                unsafe {
                    let _ = CloseHandle(process);
                }
                return Err(e);
            }
        };
        // The thread waits on its own copy of the stop event and closes it, so
        // either side can let go first.
        let mut theirs = HANDLE::default();
        if let Err(e) = unsafe { DuplicateHandle(GetCurrentProcess(), stop, GetCurrentProcess(), &mut theirs, 0, false, DUPLICATE_SAME_ACCESS) } {
            unsafe {
                let _ = CloseHandle(process);
                let _ = CloseHandle(stop);
            }
            return Err(e);
        }
        let (p, s) = (SendHandle(process), SendHandle(theirs));
        let spawned = std::thread::Builder::new().name("poni-watch".into()).spawn(move || {
            let (process, stop) = (p, s);
            let which = unsafe { WaitForMultipleObjects(&[process.0, stop.0], false, INFINITE) };
            if which == WAIT_OBJECT_0 {
                let mut code = 0u32;
                let code = unsafe { GetExitCodeProcess(process.0, &mut code) }.ok().map(|_| code);
                sink.send(AppEvent::ProcessExited { pid, code });
            }
            unsafe {
                let _ = CloseHandle(process.0);
                let _ = CloseHandle(stop.0);
            }
        });
        if let Err(e) = spawned {
            unsafe {
                let _ = CloseHandle(process);
                let _ = CloseHandle(stop);
                let _ = CloseHandle(theirs);
            }
            return Err(windows::core::Error::new(windows::core::HRESULT(-1), e.to_string()));
        }
        Ok(Self { stop })
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        // The thread may still be waiting (it wakes and ends) or already be
        // done; it holds its own copy of the event, so this one can go.
        unsafe {
            let _ = SetEvent(self.stop);
            let _ = CloseHandle(self.stop);
        }
    }
}
