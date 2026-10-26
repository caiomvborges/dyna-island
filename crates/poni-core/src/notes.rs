//! The Notes page's quick notes: short lines typed during a call or between
//! tasks, newest first, each with the moment it was written. They live in
//! `data.json` and never leave the PC.

use serde::{Deserialize, Serialize};

/// Longest note, in characters (a line or two, not a document).
pub const MAX_NOTE_LEN: usize = 500;
/// The oldest go once there are this many.
pub const MAX_NOTES: usize = 200;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    pub id: u64,
    pub text: String,
    /// The day it was written (`LocalDateTime::day_key`, "2026-10-25").
    pub day: String,
    /// The time it was written, as the island's clock showed it ("14:32").
    pub time: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Notes {
    /// Newest first.
    pub items: Vec<Note>,
    pub next_id: u64,
}

impl Notes {
    /// Add `text` (trimmed, cut to [`MAX_NOTE_LEN`]) at the top. `None` when
    /// there is nothing to keep.
    pub fn add(&mut self, text: &str, day: &str, time: &str) -> Option<u64> {
        let text: String = text.trim().chars().take(MAX_NOTE_LEN).collect();
        if text.is_empty() {
            return None;
        }
        self.next_id += 1;
        let id = self.next_id;
        self.items.insert(0, Note { id, text, day: day.to_string(), time: time.to_string() });
        self.items.truncate(MAX_NOTES);
        Some(id)
    }

    /// Take a note out; returns it and where it was (for Undo).
    pub fn remove(&mut self, id: u64) -> Option<(usize, Note)> {
        let i = self.items.iter().position(|n| n.id == id)?;
        Some((i, self.items.remove(i)))
    }

    /// Put a removed note back where it was.
    pub fn restore(&mut self, at: usize, note: Note) {
        if self.items.iter().any(|n| n.id == note.id) {
            return;
        }
        self.items.insert(at.min(self.items.len()), note);
        self.items.truncate(MAX_NOTES);
    }
}

/// When a note was written, as its row says it: the time on the day it was
/// written, else the day ("24 Oct").
pub fn when(note: &Note, today: &str) -> String {
    if note.day == today {
        return note.time.clone();
    }
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let mut parts = note.day.split('-').skip(1).map(|p| p.parse::<usize>().unwrap_or(0));
    match (parts.next(), parts.next()) {
        (Some(m @ 1..=12), Some(d @ 1..=31)) => format!("{d} {}", MONTHS[m - 1]),
        _ => note.day.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_go_on_top_trimmed_and_capped() {
        let mut n = Notes::default();
        assert_eq!(n.add("   ", "2026-10-25", "14:00"), None);
        let a = n.add("  ligar para o cliente  ", "2026-10-25", "14:00").unwrap();
        let b = n.add(&"x".repeat(MAX_NOTE_LEN + 20), "2026-10-25", "14:05").unwrap();
        assert_eq!(n.items[0].id, b);
        assert_eq!(n.items[0].text.chars().count(), MAX_NOTE_LEN);
        assert_eq!(n.items[1].text, "ligar para o cliente");
        for i in 0..MAX_NOTES {
            n.add(&format!("{i}"), "2026-10-25", "15:00");
        }
        assert_eq!(n.items.len(), MAX_NOTES);
        assert!(n.items.iter().all(|x| x.id != a), "the oldest went");
    }

    #[test]
    fn a_removed_note_comes_back_where_it_was() {
        let mut n = Notes::default();
        for t in ["c", "b", "a"] {
            n.add(t, "2026-10-25", "10:00");
        }
        let id = n.items[1].id;
        let (at, note) = n.remove(id).unwrap();
        assert_eq!((at, note.text.as_str()), (1, "b"));
        assert!(n.remove(id).is_none());
        n.restore(at, note.clone());
        n.restore(at, note);
        assert_eq!(n.items.iter().map(|x| x.text.as_str()).collect::<Vec<_>>(), ["a", "b", "c"]);
    }

    #[test]
    fn rows_say_the_time_today_and_the_day_before() {
        let note = Note { id: 1, text: "x".into(), day: "2026-10-24".into(), time: "09:12".into() };
        assert_eq!(when(&note, "2026-10-24"), "09:12");
        assert_eq!(when(&note, "2026-10-25"), "24 Oct");
    }
}
