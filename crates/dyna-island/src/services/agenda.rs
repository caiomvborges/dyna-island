//! The Outlook agenda add-on's reader: the calendar Outlook publishes as an
//! ICS link (Outlook.com: Settings › Calendar › Shared calendars › Publish),
//! read into today's and tomorrow's meetings.
//!
//! **The app's only network connection.** It reaches exactly the link the
//! person pasted, which `dyna_core::ics::check_link` holds to `https` on
//! Outlook's own calendar hosts; redirects are turned off, so no other
//! address is ever asked, and nothing is sent but the request for the file.
//! The link is never logged (only its host).
//!
//! One thread, while the add-on is on and a link is set, sleeps on its
//! channel and reads when the app asks (`Agenda::refresh`: the add-on or
//! the link set, every few minutes while someone is at the PC, the Agenda
//! tab opening). Dropping the [`Agenda`] ends the thread.

use crate::events::{AppEvent, EventSink};
use dyna_core::agenda::{civil, Meeting};
use dyna_core::ics::{self, Instant, MAX_ICS_BYTES};
use std::sync::mpsc::{self, Sender};
use windows::core::{w, HSTRING, PCWSTR};
use windows::Win32::Foundation::SYSTEMTIME;
use windows::Win32::Networking::WinHttp::{
    WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest, WinHttpQueryHeaders, WinHttpReadData, WinHttpReceiveResponse,
    WinHttpSendRequest, WinHttpSetOption, WinHttpSetTimeouts, INTERNET_DEFAULT_HTTPS_PORT, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
    WINHTTP_DISABLE_REDIRECTS, WINHTTP_FLAG_SECURE, WINHTTP_OPTION_DISABLE_FEATURE, WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_STATUS_CODE,
};
use windows::Win32::System::Time::SystemTimeToTzSpecificLocalTime;

/// Give up on a slow answer after this long (milliseconds).
const TIMEOUT_MS: i32 = 15_000;

/// What a read found.
#[derive(Clone, Debug, PartialEq)]
pub enum AgendaRead {
    Meetings(Vec<Meeting>),
    /// The calendar couldn't be reached (offline, timed out): try again later.
    Offline(String),
    /// Outlook answered, but not with the calendar (a removed link, a redirect).
    Failed(String),
}

struct Ask {
    link: String,
    /// Local wall-clock seconds at the start of today.
    day_start: i64,
}

pub struct Agenda {
    tx: Sender<Ask>,
}

impl Agenda {
    pub fn start(sink: EventSink) -> std::io::Result<Self> {
        let (tx, rx) = mpsc::channel::<Ask>();
        std::thread::Builder::new().name("dyna-agenda".into()).spawn(move || {
            // Blocks while idle: nothing runs between reads.
            while let Ok(mut ask) = rx.recv() {
                // A burst of asks is one read.
                while let Ok(a) = rx.try_recv() {
                    ask = a;
                }
                sink.send(AppEvent::Agenda(read(&ask)));
            }
        })?;
        Ok(Self { tx })
    }

    /// Read `link` for the day starting at `day_start` (local wall-clock
    /// seconds) and the next one.
    pub fn refresh(&self, link: &str, day_start: i64) {
        let _ = self.tx.send(Ask { link: link.to_string(), day_start });
    }
}

fn read(ask: &Ask) -> AgendaRead {
    let Ok(link) = ics::check_link(&ask.link) else {
        return AgendaRead::Failed("That isn't an Outlook calendar link".into());
    };
    let Some((host, path)) = ics::host_and_path(&link) else {
        return AgendaRead::Failed("That isn't an Outlook calendar link".into());
    };
    let body = match fetch(&host, &path) {
        Ok(b) => b,
        Err(Fetch::Offline(why)) => {
            crate::pinfo!("agenda: {host} not reached ({why})");
            return AgendaRead::Offline(why);
        }
        Err(Fetch::Refused(why)) => {
            crate::pwarn!("agenda: {host} answered {why}");
            return AgendaRead::Failed(why);
        }
    };
    let text = String::from_utf8_lossy(&body);
    if !text.trim_start_matches('\u{feff}').trim_start().to_ascii_uppercase().starts_with("BEGIN:VCALENDAR") {
        crate::pwarn!("agenda: {host} answered something that isn't a calendar");
        return AgendaRead::Failed("The link didn't give a calendar".into());
    }
    let cal = ics::parse(&text);
    // The window in UTC, with a day either side (the PC's offset is at most
    // 14 h); what isn't today or tomorrow on the PC's clock goes below.
    let now_utc = crate::clock::unix_now();
    let from = now_utc - 2 * 86_400;
    let to = now_utc + 3 * 86_400;
    let (first, last) = (ask.day_start, ask.day_start + 2 * 86_400);
    let meetings: Vec<Meeting> = cal
        .occurrences(from, to)
        .into_iter()
        .filter_map(|o| {
            let (start, end) = (local_wall(o.start)?, local_wall(o.end)?);
            (start < last && end > first).then(|| Meeting { subject: o.summary, location: o.location, join: o.join, start, end, all_day: o.all_day })
        })
        .collect();
    crate::pinfo!("agenda: {} meetings from {host}", meetings.len());
    AgendaRead::Meetings(dyna_core::agenda::tidy(meetings))
}

/// An instant on the PC's wall clock (local wall-clock seconds).
fn local_wall(i: Instant) -> Option<i64> {
    match i {
        Instant::LocalWall(s) => Some(s),
        Instant::Utc(s) => {
            let (y, m, d, h, min) = civil(s);
            let utc = SYSTEMTIME { wYear: y as u16, wMonth: u16::from(m), wDay: u16::from(d), wHour: u16::from(h), wMinute: u16::from(min), ..Default::default() };
            let mut local = SYSTEMTIME::default();
            unsafe { SystemTimeToTzSpecificLocalTime(None, &utc, &mut local) }.ok()?;
            let day = dyna_core::datetime::days_from_civil(i32::from(local.wYear), local.wMonth as u8, local.wDay as u8);
            Some(day * 86_400 + i64::from(local.wHour) * 3600 + i64::from(local.wMinute) * 60 + s.rem_euclid(60))
        }
    }
}

enum Fetch {
    Offline(String),
    Refused(String),
}

/// A WinHTTP handle, closed when it goes.
struct Handle(*mut core::ffi::c_void);

impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                let _ = WinHttpCloseHandle(self.0);
            }
        }
    }
}

/// GET `https://host/path`, nothing else: no redirects, at most
/// [`MAX_ICS_BYTES`].
fn fetch(host: &str, path: &str) -> Result<Vec<u8>, Fetch> {
    let off = |e: windows::core::Error| Fetch::Offline(e.message());
    unsafe {
        let session = Handle(WinHttpOpen(w!("DynaIsland"), WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, PCWSTR::null(), PCWSTR::null(), 0));
        if session.0.is_null() {
            return Err(off(windows::core::Error::from_win32()));
        }
        WinHttpSetTimeouts(session.0, TIMEOUT_MS, TIMEOUT_MS, TIMEOUT_MS, TIMEOUT_MS).map_err(off)?;
        let connect = Handle(WinHttpConnect(session.0, &HSTRING::from(host), INTERNET_DEFAULT_HTTPS_PORT, 0));
        if connect.0.is_null() {
            return Err(off(windows::core::Error::from_win32()));
        }
        let request = Handle(WinHttpOpenRequest(connect.0, w!("GET"), &HSTRING::from(path), PCWSTR::null(), PCWSTR::null(), std::ptr::null(), WINHTTP_FLAG_SECURE));
        if request.0.is_null() {
            return Err(off(windows::core::Error::from_win32()));
        }
        // Only this address: a redirect comes back as what it is, never followed.
        WinHttpSetOption(Some(request.0), WINHTTP_OPTION_DISABLE_FEATURE, Some(&WINHTTP_DISABLE_REDIRECTS.to_le_bytes())).map_err(off)?;
        WinHttpSendRequest(request.0, None, None, 0, 0, 0).map_err(off)?;
        WinHttpReceiveResponse(request.0, std::ptr::null_mut()).map_err(off)?;
        let mut status = 0u32;
        let mut len = std::mem::size_of::<u32>() as u32;
        WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            PCWSTR::null(),
            Some(&mut status as *mut u32 as *mut core::ffi::c_void),
            &mut len,
            std::ptr::null_mut(),
        )
        .map_err(off)?;
        if status != 200 {
            return Err(Fetch::Refused(match status {
                301..=308 => "The link points somewhere else now: publish the calendar again".to_string(),
                401 | 403 | 404 | 410 => "The link no longer works: publish the calendar again".to_string(),
                s => format!("Outlook answered {s}"),
            }));
        }
        let mut body = Vec::new();
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let mut got = 0u32;
            WinHttpReadData(request.0, buf.as_mut_ptr().cast(), buf.len() as u32, &mut got).map_err(off)?;
            if got == 0 {
                break;
            }
            body.extend_from_slice(&buf[..got as usize]);
            if body.len() > MAX_ICS_BYTES {
                return Err(Fetch::Refused("The calendar is too big to read".into()));
            }
        }
        Ok(body)
    }
}
