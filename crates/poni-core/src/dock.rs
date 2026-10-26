//! The Dock page's apps: programs and shortcuts the person picked, each
//! opened with a click. Only the paths are kept (in `data.json`); the names
//! come from the files and the icons from Windows when the page draws.

use serde::{Deserialize, Serialize};

/// How many apps the Dock holds: two rows of tiles at the narrowest island,
/// with the Add tile in the last place while there is room.
pub const MAX_APPS: usize = 11;

/// One app on the Dock.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockApp {
    /// What opens: an `.exe`, a shortcut (`.lnk`) or anything Windows opens
    /// with a double-click.
    pub path: String,
    /// The name under its icon (the file's name without its extension).
    pub name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Dock {
    pub apps: Vec<DockApp>,
}

/// Why an app wasn't added.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddError {
    Empty,
    AlreadyThere,
    Full,
}

impl Dock {
    /// Add `path` at the end. Paths compare without case, as Windows does.
    pub fn add(&mut self, path: &str) -> Result<&DockApp, AddError> {
        let path = path.trim();
        if path.is_empty() {
            return Err(AddError::Empty);
        }
        if self.apps.iter().any(|a| a.path.eq_ignore_ascii_case(path)) {
            return Err(AddError::AlreadyThere);
        }
        if self.is_full() {
            return Err(AddError::Full);
        }
        self.apps.push(DockApp { path: path.to_string(), name: display_name(path) });
        Ok(self.apps.last().expect("just pushed"))
    }

    /// Take the app at `i` off the Dock (the file itself is untouched).
    pub fn remove(&mut self, i: usize) -> Option<DockApp> {
        (i < self.apps.len()).then(|| self.apps.remove(i))
    }

    pub fn is_full(&self) -> bool {
        self.apps.len() >= MAX_APPS
    }
}

/// The name a path shows: its file name without the extension
/// (`C:\…\CBHub.lnk` → "CBHub"), or the whole path when it has none.
pub fn display_name(path: &str) -> String {
    let file = path.trim_end_matches(['\\', '/']).rsplit(['\\', '/']).next().unwrap_or(path);
    let stem = match file.rfind('.') {
        Some(i) if i > 0 => &file[..i],
        _ => file,
    };
    if stem.trim().is_empty() { path.to_string() } else { stem.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_come_from_the_file() {
        assert_eq!(display_name(r"C:\Users\me\Desktop\CBHub.lnk"), "CBHub");
        assert_eq!(display_name(r"C:\Program Files\App\app.v2.exe"), "app.v2");
        assert_eq!(display_name(r"C:\Tools\runner"), "runner");
        assert_eq!(display_name(r"C:\Tools\.hidden"), ".hidden");
    }

    #[test]
    fn adds_once_and_stops_when_full() {
        let mut d = Dock::default();
        assert_eq!(d.add(r"C:\a\CBHub.lnk").map(|a| a.name.clone()), Ok("CBHub".to_string()));
        assert_eq!(d.add(r"c:\A\cbhub.LNK").err(), Some(AddError::AlreadyThere));
        assert_eq!(d.add("  ").err(), Some(AddError::Empty));
        for i in 1..MAX_APPS {
            assert!(d.add(&format!(r"C:\a\app{i}.exe")).is_ok());
        }
        assert!(d.is_full());
        assert_eq!(d.add(r"C:\a\one-more.exe").err(), Some(AddError::Full));
        assert_eq!(d.remove(0).map(|a| a.name), Some("CBHub".to_string()));
        assert_eq!(d.remove(99), None);
        assert!(!d.is_full());
    }

    #[test]
    fn an_old_data_file_has_an_empty_dock() {
        let d: Dock = serde_json::from_str("{}").unwrap();
        assert!(d.apps.is_empty());
    }
}
