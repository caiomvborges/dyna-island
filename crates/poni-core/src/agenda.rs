//! The Outlook agenda add-on: today's and tomorrow's meetings as Outlook
//! (classic, desktop) has them on this PC, and the next one counting down on
//! the closed island.
//!
//! Times are kept as **local wall-clock seconds**: the local date and time
//! read as if they were UTC (what Outlook's automation dates are, and what
//! [`local_secs`] makes of the island's clock), so no time zone math is
//! needed to compare them.

use crate::datetime::{civil_from_days, days_from_civil, LocalDateTime};

/// The island counts down from this long before a meeting...
pub const LEAD_S: i64 = 15 * 60;
/// ...and says "now" for this long after it starts.
pub const NOW_S: i64 = 5 * 60;

#[derive(Clone, Debug, PartialEq)]
pub struct Meeting {
    pub subject: String,
    pub location: String,
    /// Local wall-clock seconds (see the module docs).
    pub start: i64,
    pub end: i64,
    pub all_day: bool,
}

/// The island's local time as local wall-clock seconds.
pub fn local_secs(t: &LocalDateTime) -> i64 {
    days_from_civil(t.year, t.month, t.day) * 86_400 + i64::from(t.hour) * 3600 + i64::from(t.minute) * 60 + i64::from(t.second)
}

/// An OLE automation date (days since 30 Dec 1899, local) as local wall-clock seconds.
pub fn from_ole_date(d: f64) -> i64 {
    ((d - 25_569.0) * 86_400.0).round() as i64
}

/// Local wall-clock seconds back to (year, month, day, hour, minute).
pub fn civil(secs: i64) -> (i32, u8, u8, u8, u8) {
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    let s = secs.rem_euclid(86_400);
    (y, m, d, (s / 3600) as u8, (s % 3600 / 60) as u8)
}

/// Sort and drop what the island never shows: cancelled meetings are left
/// out by the reader; here, duplicates and meetings that end before they start.
pub fn tidy(mut list: Vec<Meeting>) -> Vec<Meeting> {
    list.retain(|m| m.end >= m.start);
    list.sort_by(|a, b| (a.all_day, a.start, &a.subject).cmp(&(b.all_day, b.start, &b.subject)));
    list.dedup_by(|a, b| a.start == b.start && a.subject == b.subject);
    list
}

/// The next timed meeting that hasn't ended, and seconds until it starts
/// (negative once it has).
pub fn next(list: &[Meeting], now: i64) -> Option<(&Meeting, i64)> {
    list.iter().filter(|m| !m.all_day && m.end > now).min_by_key(|m| m.start).map(|m| (m, m.start - now))
}

/// What the closed island shows: the next meeting from [`LEAD_S`] before it
/// until [`NOW_S`] after it starts.
pub fn countdown(list: &[Meeting], now: i64) -> Option<(&Meeting, i64)> {
    next(list, now).filter(|(_, until)| (-NOW_S..=LEAD_S).contains(until))
}

/// "in 8 min", "in 1 min", "now".
pub fn fmt_until(until: i64) -> String {
    if until <= 0 {
        "now".to_string()
    } else {
        format!("in {} min", (until + 59) / 60)
    }
}

/// "14:30" (or "2:30 PM").
pub fn fmt_time(secs: i64, use_24h: bool) -> String {
    let (_, _, _, h, m) = civil(secs);
    if use_24h {
        format!("{h:02}:{m:02}")
    } else {
        let h12 = if h % 12 == 0 { 12 } else { h % 12 };
        format!("{h12}:{m:02} {}", if h < 12 { "AM" } else { "PM" })
    }
}

/// The meetings on the day of `now`, and those on the next day.
pub fn by_day(list: &[Meeting], now: i64) -> (Vec<&Meeting>, Vec<&Meeting>) {
    let today = now.div_euclid(86_400);
    let day = |m: &&Meeting| m.start.div_euclid(86_400);
    let a = list.iter().filter(|m| day(m) == today || (m.start < now && m.end > now)).collect();
    let b = list.iter().filter(|m| day(m) == today + 1).collect();
    (a, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(h: i64, m: i64) -> i64 {
        days_from_civil(2026, 10, 25) * 86_400 + h * 3600 + m * 60
    }

    fn meeting(subject: &str, start: i64, mins: i64) -> Meeting {
        Meeting { subject: subject.into(), location: String::new(), start, end: start + mins * 60, all_day: false }
    }

    #[test]
    fn ole_dates_and_the_island_clock_agree() {
        // 25 Oct 2026 14:30 local is OLE date 46320.604166...
        let ole = 46_320.0 + (14.5 / 24.0);
        let t = LocalDateTime { year: 2026, month: 10, day: 25, hour: 14, minute: 30, second: 0 };
        assert_eq!(from_ole_date(ole), local_secs(&t));
        assert_eq!(civil(local_secs(&t)), (2026, 10, 25, 14, 30));
        assert_eq!(fmt_time(local_secs(&t), true), "14:30");
        assert_eq!(fmt_time(local_secs(&t), false), "2:30 PM");
    }

    #[test]
    fn the_island_counts_down_from_fifteen_minutes_to_five_after() {
        let list = tidy(vec![meeting("Later", at(16, 0), 30), meeting("Weekly", at(14, 30), 60)]);
        assert_eq!(countdown(&list, at(14, 0)), None, "30 min away");
        let (m, until) = countdown(&list, at(14, 22)).unwrap();
        assert_eq!((m.subject.as_str(), fmt_until(until).as_str()), ("Weekly", "in 8 min"));
        assert_eq!(fmt_until(countdown(&list, at(14, 31)).unwrap().1), "now");
        assert_eq!(countdown(&list, at(14, 40)), None, "under way for 10 min");
        assert_eq!(next(&list, at(14, 40)).unwrap().0.subject, "Weekly", "still the next one until it ends");
        assert_eq!(next(&list, at(15, 40)).unwrap().0.subject, "Later");
    }

    #[test]
    fn all_day_events_never_count_down_and_days_split() {
        let mut all_day = meeting("Holiday", at(0, 0), 24 * 60);
        all_day.all_day = true;
        let tomorrow = meeting("Tomorrow", at(9, 0) + 86_400, 30);
        let list = tidy(vec![tomorrow, all_day, meeting("Today", at(10, 0), 30), meeting("Today", at(10, 0), 30)]);
        assert_eq!(list.len(), 3, "the copy went");
        assert_eq!(next(&list, at(8, 0)).unwrap().0.subject, "Today");
        let (today, next_day) = by_day(&list, at(8, 0));
        assert_eq!(today.iter().map(|m| m.subject.as_str()).collect::<Vec<_>>(), ["Today", "Holiday"]);
        assert_eq!(next_day.iter().map(|m| m.subject.as_str()).collect::<Vec<_>>(), ["Tomorrow"]);
    }
}
