//! iCalendar (`.ics`) for the Outlook agenda add-on: the calendar Outlook.com
//! publishes as a link, read into meetings.
//!
//! What a published Outlook calendar uses is covered: folded lines and
//! escapes, `VTIMEZONE` (Windows zone names with their own standard and
//! daylight rules), all-day events, `RRULE` (daily, weekly, monthly, yearly,
//! with `INTERVAL`, `COUNT`, `UNTIL`, `BYDAY`, `BYMONTHDAY`, `BYMONTH`,
//! `BYSETPOS`), `EXDATE`, changed occurrences (`RECURRENCE-ID`) and
//! cancelled events. Times come out as [`Instant`]s: UTC when the file says
//! where they are, else the PC's own wall clock.
//!
//! The link itself is checked by [`check_link`]: only `https` to Outlook's
//! own calendar hosts, ending in `.ics`.

use crate::datetime::{civil_from_days, days_from_civil, days_in_month, weekday_from_days};
use std::collections::HashMap;

/// A published calendar is far smaller; anything bigger is refused.
pub const MAX_ICS_BYTES: usize = 4 * 1024 * 1024;
/// The only hosts a link may name: where Outlook publishes calendars.
pub const HOSTS: [&str; 3] = ["outlook.live.com", "outlook.office365.com", "outlook.office.com"];

// ------------------------------------------------------------ the link

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkError {
    Empty,
    /// Not `https://` (or `webcal://`, read as https).
    NotHttps,
    /// Not one of Outlook's calendar hosts.
    NotOutlook,
    /// Not a link to an `.ics` file.
    NotIcs,
}

impl LinkError {
    /// What the Agenda page says.
    pub fn say(self) -> &'static str {
        match self {
            LinkError::Empty => "Copy the ICS link first",
            LinkError::NotHttps | LinkError::NotIcs => "That isn't a calendar's ICS link",
            LinkError::NotOutlook => "Only Outlook's own calendar links are read",
        }
    }
}

/// Check a pasted link and return it as the app keeps it (`webcal://` read as
/// `https://`, spaces and wrapping quotes or brackets dropped).
pub fn check_link(text: &str) -> Result<String, LinkError> {
    let t = text.trim().trim_matches(|c| matches!(c, '"' | '\'' | '<' | '>')).trim();
    if t.is_empty() {
        return Err(LinkError::Empty);
    }
    let rest = if let Some(r) = strip_ci(t, "https://") {
        r
    } else if let Some(r) = strip_ci(t, "webcal://") {
        r
    } else {
        return Err(LinkError::NotHttps);
    };
    if t.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(LinkError::NotIcs);
    }
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    let host = host.to_ascii_lowercase();
    if !HOSTS.contains(&host.as_str()) {
        return Err(LinkError::NotOutlook);
    }
    let file = path.split(['?', '#']).next().unwrap_or("");
    if !file.to_ascii_lowercase().ends_with(".ics") {
        return Err(LinkError::NotIcs);
    }
    Ok(format!("https://{host}/{path}"))
}

/// (host, path with its query) of a link [`check_link`] accepted.
pub fn host_and_path(link: &str) -> Option<(String, String)> {
    let rest = strip_ci(link, "https://")?;
    let (host, path) = rest.split_once('/')?;
    let path = path.split('#').next().unwrap_or("");
    Some((host.to_ascii_lowercase(), format!("/{path}")))
}

fn strip_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    (s.len() >= prefix.len() && s.is_char_boundary(prefix.len()) && s[..prefix.len()].eq_ignore_ascii_case(prefix)).then(|| &s[prefix.len()..])
}

// ------------------------------------------------------------ times

/// A moment: in UTC, or on the PC's own wall clock (floating times, all-day
/// dates, and zones the file doesn't describe).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Instant {
    Utc(i64),
    LocalWall(i64),
}

impl Instant {
    /// Seconds, near enough to compare (within a day's offsets).
    fn rough(self) -> i64 {
        match self {
            Instant::Utc(s) | Instant::LocalWall(s) => s,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Time {
    Utc(i64),
    /// Wall-clock seconds in zone `tz` (None: floating).
    Wall { wall: i64, tz: Option<String> },
    /// A date (days since 1970-01-01), for all-day events.
    Date(i64),
}

impl Time {
    /// Wall-clock seconds as written (UTC times are their own wall clock).
    fn wall(&self) -> i64 {
        match self {
            Time::Utc(s) => *s,
            Time::Wall { wall, .. } => *wall,
            Time::Date(d) => d * 86_400,
        }
    }

    fn tz(&self) -> Option<&str> {
        match self {
            Time::Wall { tz, .. } => tz.as_deref(),
            _ => None,
        }
    }
}

/// "20261025T143000Z", "20261025T143000", "20261025".
fn parse_time(value: &str, tz: Option<&str>, date_only: bool) -> Option<Time> {
    let v = value.trim();
    let date = |s: &str| -> Option<i64> {
        if s.len() != 8 || !s.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let (y, m, d) = (s[0..4].parse().ok()?, s[4..6].parse().ok()?, s[6..8].parse().ok()?);
        ((1..=12).contains(&m) && d >= 1 && d <= days_in_month(y, m)).then(|| days_from_civil(y, m, d))
    };
    if date_only || v.len() == 8 {
        return date(v.get(..8)?).map(Time::Date);
    }
    let (d, rest) = v.split_once('T')?;
    let day = date(d)?;
    let (hms, utc) = match rest.strip_suffix('Z').or_else(|| rest.strip_suffix('z')) {
        Some(h) => (h, true),
        None => (rest, false),
    };
    if hms.len() < 6 || !hms[..6].bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let secs = hms[0..2].parse::<i64>().ok()? * 3600 + hms[2..4].parse::<i64>().ok()? * 60 + hms[4..6].parse::<i64>().ok()?;
    let wall = day * 86_400 + secs;
    Some(if utc { Time::Utc(wall) } else { Time::Wall { wall, tz: tz.map(str::to_string) } })
}

/// "+0100", "-0300", "+013000" in seconds.
fn parse_offset(v: &str) -> Option<i64> {
    let v = v.trim();
    let (sign, digits) = match v.as_bytes().first()? {
        b'+' => (1, &v[1..]),
        b'-' => (-1, &v[1..]),
        _ => (1, v),
    };
    if digits.len() < 4 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let s = digits[0..2].parse::<i64>().ok()? * 3600 + digits[2..4].parse::<i64>().ok()? * 60 + digits.get(4..6).and_then(|x| x.parse::<i64>().ok()).unwrap_or(0);
    Some(sign * s)
}

// ------------------------------------------------------------ recurrence

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Freq {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

#[derive(Clone, Debug, PartialEq)]
struct RRule {
    freq: Freq,
    interval: i64,
    count: Option<u32>,
    until: Option<Time>,
    /// (ordinal, weekday 0 = Sunday); ordinal 0 = every one.
    by_day: Vec<(i32, u8)>,
    by_month_day: Vec<i32>,
    by_month: Vec<u8>,
    by_set_pos: Vec<i32>,
}

fn parse_rrule(v: &str) -> Option<RRule> {
    let mut r = RRule { freq: Freq::Daily, interval: 1, count: None, until: None, by_day: Vec::new(), by_month_day: Vec::new(), by_month: Vec::new(), by_set_pos: Vec::new() };
    let mut freq = None;
    for part in v.split(';') {
        let (k, val) = part.split_once('=')?;
        let list = || val.split(',').filter(|x| !x.is_empty());
        match k.trim().to_ascii_uppercase().as_str() {
            "FREQ" => {
                freq = Some(match val.trim().to_ascii_uppercase().as_str() {
                    "DAILY" => Freq::Daily,
                    "WEEKLY" => Freq::Weekly,
                    "MONTHLY" => Freq::Monthly,
                    "YEARLY" => Freq::Yearly,
                    // Hourly and finer don't come from Outlook.
                    _ => return None,
                })
            }
            "INTERVAL" => r.interval = val.trim().parse::<i64>().ok().filter(|n| *n >= 1).unwrap_or(1),
            "COUNT" => r.count = val.trim().parse().ok(),
            "UNTIL" => r.until = parse_time(val, None, false),
            "BYDAY" => {
                for d in list() {
                    let d = d.trim().to_ascii_uppercase();
                    let (num, name) = d.split_at(d.len().saturating_sub(2));
                    let wd = ["SU", "MO", "TU", "WE", "TH", "FR", "SA"].iter().position(|n| *n == name)? as u8;
                    let ord = if num.is_empty() { 0 } else { num.trim_start_matches('+').parse().ok()? };
                    r.by_day.push((ord, wd));
                }
            }
            "BYMONTHDAY" => r.by_month_day = list().filter_map(|x| x.trim().parse().ok()).collect(),
            "BYMONTH" => r.by_month = list().filter_map(|x| x.trim().parse().ok()).filter(|m| (1..=12).contains(m)).collect(),
            "BYSETPOS" => r.by_set_pos = list().filter_map(|x| x.trim().parse().ok()).collect(),
            _ => {}
        }
    }
    r.freq = freq?;
    Some(r)
}

/// The days (since 1970) of month `m` of year `y` the rule picks.
fn month_days(y: i32, m: u8, r: &RRule, default_day: u8) -> Vec<i64> {
    let dim = days_in_month(y, m) as i32;
    let first = days_from_civil(y, m, 1);
    let mut days: Vec<i32> = Vec::new();
    for &md in &r.by_month_day {
        let d = if md > 0 { md } else { dim + md + 1 };
        if (1..=dim).contains(&d) {
            days.push(d);
        }
    }
    if !r.by_day.is_empty() {
        let mut picked = Vec::new();
        for &(ord, wd) in &r.by_day {
            let all: Vec<i32> = (1..=dim).filter(|d| weekday_from_days(first + i64::from(*d) - 1) == wd).collect();
            match ord {
                0 => picked.extend(all),
                n if n > 0 => picked.extend(all.get(n as usize - 1)),
                n => picked.extend(all.len().checked_sub(n.unsigned_abs() as usize).and_then(|i| all.get(i))),
            }
        }
        days = if r.by_month_day.is_empty() { picked } else { days.into_iter().filter(|d| picked.contains(d)).collect() };
    }
    if r.by_month_day.is_empty() && r.by_day.is_empty() && i32::from(default_day) <= dim {
        days.push(i32::from(default_day));
    }
    days.sort_unstable();
    days.dedup();
    days.into_iter().map(|d| first + i64::from(d) - 1).collect()
}

/// The starts (wall seconds, in the event's own clock) of rule `r` from
/// `start` up to `until_wall`/`count`, as far as `end_wall`.
fn expand(r: &RRule, start: i64, end_wall: i64, until_wall: Option<i64>) -> Vec<i64> {
    let sod = start.rem_euclid(86_400);
    let start_day = start.div_euclid(86_400);
    let (y0, m0, d0) = civil_from_days(start_day);
    let wd0 = weekday_from_days(start_day);
    let mon = |wd: u8| i64::from((wd + 6) % 7);
    let mut out = Vec::new();
    let mut emitted = 0u32;
    for k in 0..200_000i64 {
        let mut days: Vec<i64> = match r.freq {
            Freq::Daily => vec![start_day + k * r.interval],
            Freq::Weekly => {
                let base = start_day - mon(wd0) + k * 7 * r.interval;
                if r.by_day.is_empty() {
                    vec![base + mon(wd0)]
                } else {
                    let mut v: Vec<i64> = r.by_day.iter().map(|&(_, wd)| base + mon(wd)).collect();
                    v.sort_unstable();
                    v.dedup();
                    v
                }
            }
            Freq::Monthly => {
                let idx = i64::from(y0) * 12 + i64::from(m0) - 1 + k * r.interval;
                month_days(idx.div_euclid(12) as i32, (idx.rem_euclid(12) + 1) as u8, r, d0)
            }
            Freq::Yearly => {
                let y = y0 + (k * r.interval) as i32;
                let months = if r.by_month.is_empty() { vec![m0] } else { r.by_month.clone() };
                months.into_iter().flat_map(|m| month_days(y, m, r, d0)).collect()
            }
        };
        if r.freq == Freq::Daily || r.freq == Freq::Weekly {
            days.retain(|d| {
                let (_, m, dd) = civil_from_days(*d);
                (r.by_month.is_empty() || r.by_month.contains(&m))
                    && (r.freq == Freq::Weekly || r.by_day.is_empty() || r.by_day.iter().any(|&(_, wd)| wd == weekday_from_days(*d)))
                    && (r.by_month_day.is_empty() || r.by_month_day.contains(&i32::from(dd)))
            });
        }
        if !r.by_set_pos.is_empty() {
            let n = days.len() as i32;
            days = r.by_set_pos.iter().filter_map(|&p| if p > 0 { days.get(p as usize - 1) } else { days.get((n + p).max(-1) as usize) }).copied().collect();
            days.sort_unstable();
        }
        // The period's first possible day is past the window: nothing later counts.
        let period_floor = match r.freq {
            Freq::Daily => start_day + k * r.interval,
            Freq::Weekly => start_day - mon(wd0) + k * 7 * r.interval,
            _ => days.first().copied().unwrap_or(i64::MIN),
        };
        for d in days {
            let t = d * 86_400 + sod;
            if t < start {
                continue;
            }
            if until_wall.is_some_and(|u| t > u) || r.count.is_some_and(|c| emitted >= c) {
                return out;
            }
            emitted += 1;
            if t > end_wall {
                return out;
            }
            out.push(t);
        }
        if period_floor != i64::MIN && period_floor * 86_400 > end_wall {
            break;
        }
    }
    out
}

// ------------------------------------------------------------ time zones

#[derive(Clone, Debug, PartialEq)]
struct ZoneRule {
    start: i64,
    offset_to: i64,
    rrule: Option<RRule>,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Zone {
    rules: Vec<ZoneRule>,
}

impl Zone {
    /// The zone's offset (seconds east of UTC) at wall time `wall`: the rule
    /// whose latest onset at or before it is the most recent.
    fn offset_at(&self, wall: i64) -> Option<i64> {
        let mut best: Option<(i64, i64)> = None;
        for rule in &self.rules {
            let onset = match &rule.rrule {
                Some(r) => expand(r, rule.start, wall, r.until.as_ref().map(Time::wall)).last().copied(),
                None => (rule.start <= wall).then_some(rule.start),
            };
            if let Some(o) = onset {
                if best.is_none_or(|(b, _)| o > b) {
                    best = Some((o, rule.offset_to));
                }
            }
        }
        best.map(|(_, off)| off).or_else(|| self.rules.first().map(|r| r.offset_to))
    }
}

// ------------------------------------------------------------ events

#[derive(Clone, Debug, Default, PartialEq)]
struct Event {
    uid: String,
    summary: String,
    location: String,
    start: Option<Time>,
    end: Option<Time>,
    duration: Option<i64>,
    rrule: Option<RRule>,
    exdates: Vec<Time>,
    recurrence_id: Option<Time>,
    cancelled: bool,
}

/// A parsed calendar.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Calendar {
    events: Vec<Event>,
    zones: HashMap<String, Zone>,
}

/// One meeting as it happens: a single event, or one time of a repeating one.
#[derive(Clone, Debug, PartialEq)]
pub struct Occurrence {
    pub summary: String,
    pub location: String,
    pub start: Instant,
    pub end: Instant,
    pub all_day: bool,
}

struct Prop {
    name: String,
    params: Vec<(String, String)>,
    value: String,
}

impl Prop {
    fn param(&self, name: &str) -> Option<&str> {
        self.params.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    fn time(&self) -> Option<Time> {
        let date_only = self.param("VALUE").is_some_and(|v| v.eq_ignore_ascii_case("DATE"));
        parse_time(&self.value, self.param("TZID"), date_only)
    }
}

/// Folded lines joined (a line that starts with a space or tab continues the one before).
fn unfold(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in text.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if let Some(rest) = line.strip_prefix([' ', '\t']) {
            if let Some(last) = out.last_mut() {
                last.push_str(rest);
                continue;
            }
        }
        out.push(line.to_string());
    }
    out
}

fn parse_prop(line: &str) -> Option<Prop> {
    // The value starts at the first colon outside quotes.
    let mut quoted = false;
    let colon = line.char_indices().find(|&(_, c)| {
        if c == '"' {
            quoted = !quoted;
        }
        c == ':' && !quoted
    })?.0;
    let (head, value) = (&line[..colon], &line[colon + 1..]);
    let mut parts = head.split(';');
    let name = parts.next()?.trim().to_ascii_uppercase();
    let params = parts.filter_map(|p| p.split_once('=')).map(|(k, v)| (k.trim().to_string(), v.trim().trim_matches('"').to_string())).collect();
    Some(Prop { name, params, value: value.to_string() })
}

fn unescape(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    let mut chars = v.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') | Some('N') => out.push(' '),
                Some(o) => out.push(o),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Read a calendar file. What can't be read is skipped, never fatal.
pub fn parse(text: &str) -> Calendar {
    let mut cal = Calendar::default();
    let mut event: Option<Event> = None;
    let mut zone: Option<(String, Zone)> = None;
    let mut rule: Option<ZoneRule> = None;
    for line in unfold(text) {
        let Some(p) = parse_prop(&line) else { continue };
        let value = p.value.trim();
        match (p.name.as_str(), value.to_ascii_uppercase().as_str()) {
            ("BEGIN", "VEVENT") => event = Some(Event::default()),
            ("END", "VEVENT") => cal.events.extend(event.take()),
            ("BEGIN", "VTIMEZONE") => zone = Some((String::new(), Zone::default())),
            ("END", "VTIMEZONE") => {
                if let Some((name, z)) = zone.take() {
                    cal.zones.insert(name, z);
                }
            }
            ("BEGIN", "STANDARD") | ("BEGIN", "DAYLIGHT") if zone.is_some() => rule = Some(ZoneRule { start: 0, offset_to: 0, rrule: None }),
            ("END", "STANDARD") | ("END", "DAYLIGHT") => {
                if let (Some(r), Some((_, z))) = (rule.take(), zone.as_mut()) {
                    z.rules.push(r);
                }
            }
            _ => {
                if let Some(r) = rule.as_mut() {
                    match p.name.as_str() {
                        "DTSTART" => r.start = p.time().map_or(0, |t| t.wall()),
                        "TZOFFSETTO" => r.offset_to = parse_offset(value).unwrap_or(0),
                        "RRULE" => r.rrule = parse_rrule(value),
                        _ => {}
                    }
                } else if let Some((name, _)) = zone.as_mut() {
                    if p.name == "TZID" {
                        *name = value.to_string();
                    }
                } else if let Some(e) = event.as_mut() {
                    match p.name.as_str() {
                        "UID" => e.uid = value.to_string(),
                        "SUMMARY" => e.summary = unescape(&p.value),
                        "LOCATION" => e.location = unescape(&p.value),
                        "DTSTART" => e.start = p.time(),
                        "DTEND" => e.end = p.time(),
                        "DURATION" => e.duration = parse_duration(value),
                        "RRULE" => e.rrule = parse_rrule(value),
                        "EXDATE" => {
                            for v in value.split(',') {
                                let date_only = p.param("VALUE").is_some_and(|x| x.eq_ignore_ascii_case("DATE"));
                                e.exdates.extend(parse_time(v, p.param("TZID"), date_only));
                            }
                        }
                        "RECURRENCE-ID" => e.recurrence_id = p.time(),
                        "STATUS" => e.cancelled |= value.eq_ignore_ascii_case("CANCELLED"),
                        _ => {}
                    }
                }
            }
        }
    }
    cal
}

/// "PT1H30M", "P1D", "-PT15M" in seconds.
fn parse_duration(v: &str) -> Option<i64> {
    let (sign, v) = match v.strip_prefix('-') {
        Some(r) => (-1, r),
        None => (1, v.strip_prefix('+').unwrap_or(v)),
    };
    let v = v.strip_prefix('P')?;
    let (mut total, mut num, mut in_time) = (0i64, String::new(), false);
    for c in v.chars() {
        match c {
            'T' => in_time = true,
            '0'..='9' => num.push(c),
            _ => {
                let n: i64 = num.parse().ok()?;
                num.clear();
                total += n * match (c, in_time) {
                    ('W', _) => 7 * 86_400,
                    ('D', _) => 86_400,
                    ('H', true) => 3600,
                    ('M', true) => 60,
                    ('S', true) => 1,
                    _ => return None,
                };
            }
        }
    }
    Some(sign * total)
}

impl Calendar {
    /// Where a time of this calendar is.
    fn instant(&self, t: &Time) -> Instant {
        match t {
            Time::Utc(s) => Instant::Utc(*s),
            Time::Date(d) => Instant::LocalWall(d * 86_400),
            Time::Wall { wall, tz } => match tz.as_deref().and_then(|z| self.zones.get(z)).and_then(|z| z.offset_at(*wall)) {
                Some(off) => Instant::Utc(wall - off),
                None => Instant::LocalWall(*wall),
            },
        }
    }

    /// The same wall time as `like`'s, in its zone.
    fn at_wall(like: &Time, wall: i64) -> Time {
        match like {
            Time::Utc(_) => Time::Utc(wall),
            Time::Date(_) => Time::Date(wall.div_euclid(86_400)),
            Time::Wall { tz, .. } => Time::Wall { wall, tz: tz.clone() },
        }
    }

    /// Every meeting overlapping `from..to` (UTC seconds; times on the PC's
    /// wall clock are compared as if UTC, so leave a day's margin).
    pub fn occurrences(&self, from: i64, to: i64) -> Vec<Occurrence> {
        // Changed occurrences of repeating events, by (uid, original start).
        let mut changed: HashMap<(String, Instant), &Event> = HashMap::new();
        for e in &self.events {
            if let Some(rid) = &e.recurrence_id {
                changed.insert((e.uid.clone(), self.instant(rid)), e);
            }
        }
        let mut out = Vec::new();
        let push = |e: &Event, start: &Time, out: &mut Vec<Occurrence>| {
            if e.cancelled {
                return;
            }
            let all_day = matches!(start, Time::Date(_));
            let len = match (&e.end, e.duration) {
                (Some(end), _) => end.wall() - e.start.as_ref().map_or(0, Time::wall),
                (None, Some(d)) => d,
                (None, None) if all_day => 86_400,
                _ => 0,
            }
            .max(0);
            let end = Calendar::at_wall(start, start.wall() + len);
            let (s, en) = (self.instant(start), self.instant(&end));
            if s.rough() < to && en.rough() > from {
                out.push(Occurrence { summary: e.summary.clone(), location: e.location.clone(), start: s, end: en, all_day });
            }
        };
        for e in self.events.iter().filter(|e| e.recurrence_id.is_none()) {
            let Some(start) = &e.start else { continue };
            let Some(rule) = &e.rrule else {
                push(e, start, &mut out);
                continue;
            };
            // Expand on the event's own wall clock, a little past the window.
            let until = rule.until.as_ref().map(|u| match (u, start.tz()) {
                // A UTC end moved to the event's clock, near enough for a day's margin.
                (Time::Utc(s), _) => s + 86_400,
                (t, _) => t.wall() + 86_399,
            });
            let excluded: Vec<Instant> = e.exdates.iter().map(|x| self.instant(x)).collect();
            for wall in expand(rule, start.wall(), to + 2 * 86_400, until) {
                let t = Calendar::at_wall(start, wall);
                let at = self.instant(&t);
                if excluded.contains(&at) || excluded.iter().any(|x| matches!((x, at), (Instant::LocalWall(a), Instant::LocalWall(b)) if a.div_euclid(86_400) == b.div_euclid(86_400) && matches!(start, Time::Date(_)))) {
                    continue;
                }
                match changed.remove(&(e.uid.clone(), at)) {
                    Some(c) => {
                        if let Some(cs) = &c.start {
                            push(c, cs, &mut out);
                        }
                    }
                    None => push(e, &t, &mut out),
                }
            }
        }
        // Changed occurrences whose repeating event isn't in the file, or fell
        // outside what was expanded.
        for c in changed.into_values() {
            if let Some(cs) = &c.start {
                push(c, cs, &mut out);
            }
        }
        out.sort_by_key(|o| o.start.rough());
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(y: i32, m: u8, d: u8, h: i64, min: i64) -> i64 {
        days_from_civil(y, m, d) * 86_400 + h * 3600 + min * 60
    }

    const OUTLOOK: &str = "BEGIN:VCALENDAR\r\n\
METHOD:PUBLISH\r\n\
PRODID:Microsoft Exchange Server 2010\r\n\
VERSION:2.0\r\n\
BEGIN:VTIMEZONE\r\n\
TZID:E. South America Standard Time\r\n\
BEGIN:STANDARD\r\n\
DTSTART:16010101T000000\r\n\
TZOFFSETFROM:-0300\r\n\
TZOFFSETTO:-0300\r\n\
END:STANDARD\r\n\
END:VTIMEZONE\r\n\
BEGIN:VTIMEZONE\r\n\
TZID:Central Europe Standard Time\r\n\
BEGIN:STANDARD\r\n\
DTSTART:16010101T030000\r\n\
TZOFFSETFROM:+0200\r\n\
TZOFFSETTO:+0100\r\n\
RRULE:FREQ=YEARLY;INTERVAL=1;BYDAY=-1SU;BYMONTH=10\r\n\
END:STANDARD\r\n\
BEGIN:DAYLIGHT\r\n\
DTSTART:16010101T020000\r\n\
TZOFFSETFROM:+0100\r\n\
TZOFFSETTO:+0200\r\n\
RRULE:FREQ=YEARLY;INTERVAL=1;BYDAY=-1SU;BYMONTH=3\r\n\
END:DAYLIGHT\r\n\
END:VTIMEZONE\r\n\
BEGIN:VEVENT\r\n\
UID:weekly-1\r\n\
SUMMARY;LANGUAGE=pt-BR:Weekly sell-out\\, review\r\n\
LOCATION:Microsoft Teams\r\n\
DTSTART;TZID=E. South America Standard Time:20260105T093000\r\n\
DTEND;TZID=E. South America Standard Time:20260105T100000\r\n\
RRULE:FREQ=WEEKLY;UNTIL=20261231T120000Z;INTERVAL=1;BYDAY=MO,WE;WKST=SU\r\n\
EXDATE;TZID=E. South America Standard Time:20261028T093000\r\n\
END:VEVENT\r\n\
BEGIN:VEVENT\r\n\
UID:weekly-1\r\n\
RECURRENCE-ID;TZID=E. South America Standard Time:20261026T093000\r\n\
SUMMARY:Weekly (moved)\r\n\
DTSTART;TZID=E. South America Standard Time:20261026T110000\r\n\
DTEND;TZID=E. South America Standard Time:20261026T113000\r\n\
END:VEVENT\r\n\
BEGIN:VEVENT\r\n\
UID:berlin\r\n\
SUMMARY:Call with Berlin,\r\n  long title\r\n\
DTSTART;TZID=Central Europe Standard Time:20261025T150000\r\n\
DURATION:PT45M\r\n\
END:VEVENT\r\n\
BEGIN:VEVENT\r\n\
UID:holiday\r\n\
SUMMARY:Feriado\r\n\
DTSTART;VALUE=DATE:20261102\r\n\
DTEND;VALUE=DATE:20261103\r\n\
END:VEVENT\r\n\
BEGIN:VEVENT\r\n\
UID:gone\r\n\
SUMMARY:Cancelled one\r\n\
STATUS:CANCELLED\r\n\
DTSTART:20261026T130000Z\r\n\
DTEND:20261026T140000Z\r\n\
END:VEVENT\r\n\
BEGIN:VEVENT\r\n\
UID:monthly\r\n\
SUMMARY:Fechamento\r\n\
DTSTART:20260130T180000Z\r\n\
DTEND:20260130T190000Z\r\n\
RRULE:FREQ=MONTHLY;BYDAY=-1FR;COUNT=24\r\n\
END:VEVENT\r\n\
END:VCALENDAR\r\n";

    #[test]
    fn links_are_outlooks_https_ics_only() {
        let ok = "https://outlook.live.com/owa/calendar/00000000-0000/abc/cid-1/calendar.ics";
        assert_eq!(check_link(&format!("  \"{ok}\" ")), Ok(ok.to_string()));
        assert_eq!(check_link("webcal://outlook.office365.com/owa/calendar/x/y/calendar.ics"), Ok("https://outlook.office365.com/owa/calendar/x/y/calendar.ics".into()));
        assert_eq!(check_link("http://outlook.live.com/a/calendar.ics"), Err(LinkError::NotHttps));
        assert_eq!(check_link("https://evil.example/outlook.live.com/calendar.ics"), Err(LinkError::NotOutlook));
        assert_eq!(check_link("https://outlook.live.com.evil.example/calendar.ics"), Err(LinkError::NotOutlook));
        assert_eq!(check_link("https://user@outlook.live.com/calendar.ics"), Err(LinkError::NotOutlook));
        assert_eq!(check_link("https://outlook.live.com/owa/calendar/x/calendar.html"), Err(LinkError::NotIcs));
        assert_eq!(check_link(""), Err(LinkError::Empty));
        assert_eq!(host_and_path(ok), Some(("outlook.live.com".into(), "/owa/calendar/00000000-0000/abc/cid-1/calendar.ics".into())));
    }

    #[test]
    fn a_week_of_an_outlook_calendar() {
        let cal = parse(OUTLOOK);
        // Sun 25 Oct 2026 to Mon 2 Nov 2026 (UTC).
        let got = cal.occurrences(utc(2026, 10, 25, 0, 0), utc(2026, 11, 3, 0, 0));
        let names: Vec<(&str, Instant)> = got.iter().map(|o| (o.summary.as_str(), o.start)).collect();
        assert_eq!(
            names,
            [
                // 15:00 Berlin: still summer time on 25 Oct? No: it ends that morning, so +01:00.
                ("Call with Berlin, long title", Instant::Utc(utc(2026, 10, 25, 14, 0))),
                // Monday's moved to 11:00 São Paulo (-03:00).
                ("Weekly (moved)", Instant::Utc(utc(2026, 10, 26, 14, 0))),
                // Wednesday 28th is excluded; the last Friday of October.
                ("Fechamento", Instant::Utc(utc(2026, 10, 30, 18, 0))),
                ("Feriado", Instant::LocalWall(utc(2026, 11, 2, 0, 0))),
                ("Weekly sell-out, review", Instant::Utc(utc(2026, 11, 2, 12, 30))),
            ]
        );
        assert_eq!(got[0].end, Instant::Utc(utc(2026, 10, 25, 14, 45)));
        assert!(got[3].all_day && got[3].end == Instant::LocalWall(utc(2026, 11, 3, 0, 0)));
        assert_eq!(got[1].location, "");
        assert_eq!(got[4].location, "Microsoft Teams");
    }

    #[test]
    fn summer_time_follows_the_files_own_rules() {
        let cal = parse(OUTLOOK);
        let z = &cal.zones["Central Europe Standard Time"];
        assert_eq!(z.offset_at(utc(2026, 7, 1, 12, 0)), Some(7200));
        assert_eq!(z.offset_at(utc(2026, 12, 1, 12, 0)), Some(3600));
        // The last Sunday of March 2026 is the 29th, at 02:00.
        assert_eq!(z.offset_at(utc(2026, 3, 29, 1, 59)), Some(3600));
        assert_eq!(z.offset_at(utc(2026, 3, 29, 2, 0)), Some(7200));
    }

    #[test]
    fn rules_count_and_stop() {
        let r = parse_rrule("FREQ=DAILY;INTERVAL=2;COUNT=3").unwrap();
        let start = utc(2026, 10, 1, 9, 0);
        assert_eq!(expand(&r, start, utc(2027, 1, 1, 0, 0), None), [start, start + 2 * 86_400, start + 4 * 86_400]);
        let r = parse_rrule("FREQ=MONTHLY;BYMONTHDAY=-1").unwrap();
        let ends = expand(&r, utc(2026, 1, 31, 8, 0), utc(2026, 4, 1, 0, 0), None);
        assert_eq!(ends, [utc(2026, 1, 31, 8, 0), utc(2026, 2, 28, 8, 0), utc(2026, 3, 31, 8, 0)]);
        let r = parse_rrule("FREQ=MONTHLY;BYDAY=MO,TU,WE,TH,FR;BYSETPOS=-1").unwrap();
        assert_eq!(expand(&r, utc(2026, 10, 1, 8, 0), utc(2026, 11, 30, 0, 0), None), [utc(2026, 10, 30, 8, 0)]);
        assert_eq!(parse_duration("PT1H30M"), Some(5400));
        assert_eq!(parse_duration("P1W"), Some(7 * 86_400));
    }
}
